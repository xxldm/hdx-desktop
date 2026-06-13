use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    time::Duration,
};

pub const LOCAL_HOST: &str = "127.0.0.1";

pub struct HttpResponse {
    pub status_code: u16,
    pub body: String,
}

pub struct HttpRequest<'a> {
    pub method: &'a str,
    pub path: &'a str,
    pub headers: Vec<(&'a str, &'a str)>,
    pub body: Option<&'a str>,
}

pub fn reserve_local_port() -> Result<u16, String> {
    let listener =
        TcpListener::bind((LOCAL_HOST, 0)).map_err(|error| format!("分配本机端口失败：{error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("读取本机端口失败：{error}"))?
        .port();
    drop(listener);
    Ok(port)
}

pub fn http_get(port: u16, path: &str) -> Result<HttpResponse, String> {
    http_request(
        port,
        HttpRequest {
            method: "GET",
            path,
            headers: Vec::new(),
            body: None,
        },
    )
}

pub fn http_request(port: u16, request: HttpRequest<'_>) -> Result<HttpResponse, String> {
    validate_method(request.method)?;
    validate_path(request.path)?;

    let mut stream = TcpStream::connect((LOCAL_HOST, port))
        .map_err(|error| format!("连接本机服务失败：{error}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| format!("设置本机服务读取超时失败：{error}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| format!("设置本机服务写入超时失败：{error}"))?;

    let body = request.body.unwrap_or("");
    let mut header_lines = String::new();
    for (name, value) in request.headers {
        validate_header(name, value)?;
        header_lines.push_str(name);
        header_lines.push_str(": ");
        header_lines.push_str(value);
        header_lines.push_str("\r\n");
    }

    let content_headers = if body.is_empty() {
        String::new()
    } else {
        format!(
            "Content-Type: application/json\r\nContent-Length: {}\r\n",
            body.len()
        )
    };
    let request_text = format!(
        "{} {} HTTP/1.1\r\nHost: {LOCAL_HOST}:{port}\r\nConnection: close\r\nAccept: application/json\r\n{header_lines}{content_headers}\r\n{body}",
        request.method, request.path
    );
    stream
        .write_all(request_text.as_bytes())
        .map_err(|error| format!("发送本机服务请求失败：{error}"))?;

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|error| format!("读取本机服务响应失败：{error}"))?;
    parse_http_response(&response)
}

fn validate_method(method: &str) -> Result<(), String> {
    if matches!(method, "GET" | "POST") {
        return Ok(());
    }

    Err(format!("不支持的本机 HTTP 方法：{method}"))
}

fn validate_path(path: &str) -> Result<(), String> {
    if path.starts_with('/') && !path.contains('\r') && !path.contains('\n') {
        return Ok(());
    }

    Err(format!("本机 HTTP path 无效：{path}"))
}

fn validate_header(name: &str, value: &str) -> Result<(), String> {
    let valid_name = !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'));
    let valid_value = !value.contains('\r') && !value.contains('\n');

    if valid_name && valid_value {
        return Ok(());
    }

    Err(format!("本机 HTTP header 无效：{name}"))
}

fn parse_http_response(response: &[u8]) -> Result<HttpResponse, String> {
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| "本机服务响应缺少 HTTP header。".to_string())?;
    let headers = String::from_utf8_lossy(&response[..header_end]);
    let status_line = headers
        .lines()
        .next()
        .ok_or_else(|| "本机服务响应缺少状态行。".to_string())?;
    let status_code = status_line
        .split_whitespace()
        .nth(1)
        .ok_or_else(|| "本机服务响应状态行无效。".to_string())?
        .parse::<u16>()
        .map_err(|error| format!("解析本机服务 HTTP 状态失败：{error}"))?;
    let mut body = response[header_end + 4..].to_vec();

    if headers
        .to_ascii_lowercase()
        .contains("transfer-encoding: chunked")
    {
        body = decode_chunked_body(&body)?;
    }

    let body =
        String::from_utf8(body).map_err(|error| format!("本机服务响应不是 UTF-8：{error}"))?;
    Ok(HttpResponse { status_code, body })
}

fn decode_chunked_body(body: &[u8]) -> Result<Vec<u8>, String> {
    let mut cursor = 0;
    let mut decoded = Vec::new();

    loop {
        let line_end =
            find_crlf(body, cursor).ok_or_else(|| "chunked 响应缺少长度行。".to_string())?;
        let size_line = String::from_utf8_lossy(&body[cursor..line_end]);
        let size_hex = size_line.split(';').next().unwrap_or("").trim();
        let size = usize::from_str_radix(size_hex, 16)
            .map_err(|error| format!("解析 chunked 响应长度失败：{error}"))?;
        cursor = line_end + 2;

        if size == 0 {
            break;
        }

        let chunk_end = cursor + size;
        if chunk_end > body.len() {
            return Err("chunked 响应正文长度不足。".to_string());
        }
        decoded.extend_from_slice(&body[cursor..chunk_end]);
        cursor = chunk_end + 2;
    }

    Ok(decoded)
}

fn find_crlf(bytes: &[u8], start: usize) -> Option<usize> {
    bytes
        .get(start..)?
        .windows(2)
        .position(|window| window == b"\r\n")
        .map(|position| start + position)
}
