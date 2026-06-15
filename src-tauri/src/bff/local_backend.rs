use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::{
    local_http::{http_request, HttpRequest, LOCAL_HOST},
    sidecar::LocalBackendSession,
};

pub(super) fn fetch_local_json<T>(
    session: &LocalBackendSession,
    path: &str,
    method: &str,
    body: Option<Value>,
) -> Result<T, String>
where
    T: DeserializeOwned,
{
    let port = parse_local_backend_port(&session.base_url)?;
    let body_text = body
        .map(|value| serde_json::to_string(&value))
        .transpose()
        .map_err(|error| format!("序列化 Desktop BFF 请求失败：{error}"))?;
    let response = http_request(
        port,
        HttpRequest {
            method,
            path,
            headers: vec![(&session.header_name, &session.token)],
            body: body_text.as_deref(),
        },
    )?;

    if !(200..300).contains(&response.status_code) {
        return Err(format!(
            "本机后端请求失败，HTTP 状态：{}",
            response.status_code
        ));
    }

    serde_json::from_str(&response.body).map_err(|error| format!("解析本机后端响应失败：{error}"))
}

pub(super) fn parse_local_backend_port(base_url: &str) -> Result<u16, String> {
    let prefix = format!("http://{LOCAL_HOST}:");
    let port_text = base_url
        .strip_prefix(&prefix)
        .ok_or_else(|| format!("本机后端地址必须绑定 {LOCAL_HOST}：{base_url}"))?;

    port_text
        .parse::<u16>()
        .map_err(|error| format!("解析本机后端端口失败：{error}"))
}
