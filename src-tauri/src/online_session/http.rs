use std::time::Duration;

use serde::{de::DeserializeOwned, Serialize};
use ureq::{Agent, AgentBuilder};

use crate::online_config::OnlineConfig;

use super::RemoteError;

fn build_agent(config: &OnlineConfig) -> Agent {
    AgentBuilder::new()
        .timeout(Duration::from_secs(config.request_timeout_seconds))
        .build()
}

pub(super) fn build_url(base_url: &str, path: &str) -> String {
    format!("{}{path}", base_url.trim_end_matches('/'))
}

pub(super) fn post_auth<T: DeserializeOwned>(
    config: &OnlineConfig,
    path: &str,
    body: &impl Serialize,
) -> Result<T, RemoteError> {
    let value = post_auth_raw(config, path, body)?;
    serde_json::from_value::<T>(value).map_err(|error| RemoteError {
        status_code: None,
        message: format!("解析认证中心响应失败：{error}"),
    })
}

pub(super) fn post_auth_raw(
    config: &OnlineConfig,
    path: &str,
    body: &impl Serialize,
) -> Result<serde_json::Value, RemoteError> {
    let url = build_url(&config.auth_base_url, path);
    let agent = build_agent(config);

    let payload = serde_json::to_value(body).unwrap_or(serde_json::Value::Null);
    match agent
        .post(&url)
        .set("Accept", "application/json")
        .set("Content-Type", "application/json")
        .send_json(payload)
    {
        Ok(response) => {
            if response.status() == 204 {
                return Ok(serde_json::Value::Null);
            }
            response
                .into_json::<serde_json::Value>()
                .map_err(|error| RemoteError {
                    status_code: None,
                    message: format!("读取认证中心响应失败：{error}"),
                })
        }
        Err(ureq::Error::Status(status, response)) => Err(RemoteError {
            status_code: Some(status),
            message: extract_remote_message(status, response),
        }),
        Err(error) => Err(RemoteError {
            status_code: None,
            message: format!("连接认证中心失败：{error}"),
        }),
    }
}

/// 向 gateway 发送带 Bearer token 的业务请求。
pub(crate) fn fetch_remote_business<T: DeserializeOwned>(
    config: &OnlineConfig,
    access_token: &str,
    path: &str,
    method: &str,
    body: Option<&serde_json::Value>,
) -> Result<T, String> {
    fetch_remote_business_inner(config, access_token, path, method, body)
        .map_err(translate_business_error)
}

fn fetch_remote_business_inner<T: DeserializeOwned>(
    config: &OnlineConfig,
    access_token: &str,
    path: &str,
    method: &str,
    body: Option<&serde_json::Value>,
) -> Result<T, RemoteError> {
    let url = build_url(&config.gateway_base_url, path);
    let agent = build_agent(config);

    let request = match method {
        "GET" => agent.get(&url),
        "POST" => agent.post(&url),
        "PUT" => agent.put(&url),
        other => {
            return Err(RemoteError {
                status_code: None,
                message: format!("不支持的远端请求方法：{other}"),
            })
        }
    };

    let request = request
        .set("Accept", "application/json")
        .set("Authorization", &format!("Bearer {access_token}"));

    let response = if matches!(method, "POST" | "PUT") {
        let payload = body
            .cloned()
            .unwrap_or(serde_json::Value::Object(Default::default()));
        request
            .set("Content-Type", "application/json")
            .send_json(payload)
    } else {
        request.call()
    };

    match response {
        Ok(response) => parse_business_response::<T>(response),
        Err(ureq::Error::Status(status, response)) => Err(RemoteError {
            status_code: Some(status),
            message: extract_remote_message(status, response),
        }),
        Err(error) => Err(RemoteError {
            status_code: None,
            message: format!("连接业务网关失败：{error}"),
        }),
    }
}

fn parse_business_response<T: DeserializeOwned>(
    response: ureq::Response,
) -> Result<T, RemoteError> {
    let value = response
        .into_json::<serde_json::Value>()
        .map_err(|error| RemoteError {
            status_code: None,
            message: format!("解析业务网关响应失败：{error}"),
        })?;

    serde_json::from_value::<T>(value).map_err(|error| RemoteError {
        status_code: None,
        message: format!("解析业务网关响应失败：{error}"),
    })
}

fn translate_business_error(error: RemoteError) -> String {
    match error.status() {
        Some(401) => "登录已过期，请重新登录。".to_string(),
        Some(403) => "当前账号无权执行该操作。".to_string(),
        Some(status) => format!("业务网关返回 HTTP {status}。"),
        None => error.to_message(),
    }
}

fn extract_remote_message(status: u16, response: ureq::Response) -> String {
    match response.into_json::<serde_json::Value>() {
        Ok(value) => {
            if let Some(message) = value.get("message").and_then(|v| v.as_str()) {
                return message.to_string();
            }
            format!("远端服务返回 HTTP {status}。")
        }
        Err(_) => format!("远端服务返回 HTTP {status}。"),
    }
}
