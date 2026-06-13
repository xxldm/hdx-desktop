use std::{
    fs,
    path::PathBuf,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use url::Url;

use crate::flavor;

const CONFIG_FILE_NAME: &str = "online-config.json";
const HEALTH_PATH: &str = "/actuator/health";
const DEFAULT_TIMEOUT_SECONDS: u64 = 10;
const MIN_TIMEOUT_SECONDS: u64 = 1;
const MAX_TIMEOUT_SECONDS: u64 = 60;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineConfig {
    auth_base_url: String,
    gateway_base_url: String,
    request_timeout_seconds: u64,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineConfigInput {
    auth_base_url: String,
    gateway_base_url: String,
    #[serde(default)]
    request_timeout_seconds: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineConfigState {
    available: bool,
    configured: bool,
    config: Option<OnlineConfig>,
    message: Option<String>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineConnectionCheckResult {
    ok: bool,
    auth: OnlineEndpointCheck,
    gateway: OnlineEndpointCheck,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlineEndpointCheck {
    ok: bool,
    url: String,
    status_code: Option<u16>,
    elapsed_ms: u128,
    message: String,
}

#[tauri::command]
pub fn hdx_online_config_get(app: AppHandle) -> Result<OnlineConfigState, String> {
    if !flavor::active_flavor().remote_endpoint_required() {
        return Ok(OnlineConfigState {
            available: false,
            configured: false,
            config: None,
            message: Some("当前 Desktop flavor 不需要远端配置。".to_string()),
        });
    }

    let config = read_config(&config_path(&app)?)?;
    Ok(OnlineConfigState {
        available: true,
        configured: config.is_some(),
        config,
        message: None,
    })
}

#[tauri::command]
pub fn hdx_online_config_save(
    app: AppHandle,
    input: OnlineConfigInput,
) -> Result<OnlineConfigState, String> {
    ensure_online_flavor()?;

    let config = input.normalize()?;
    let path = config_path(&app)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| format!("创建 Desktop Online 配置目录失败：{error}"))?;
    }

    let json = serde_json::to_string_pretty(&config)
        .map_err(|error| format!("序列化 Desktop Online 配置失败：{error}"))?;
    fs::write(&path, json).map_err(|error| format!("写入 Desktop Online 配置失败：{error}"))?;

    Ok(OnlineConfigState {
        available: true,
        configured: true,
        config: Some(config),
        message: Some("Desktop Online 配置已保存。".to_string()),
    })
}

#[tauri::command]
pub fn hdx_online_connection_check(
    input: OnlineConfigInput,
) -> Result<OnlineConnectionCheckResult, String> {
    ensure_online_flavor()?;

    let config = input.normalize()?;
    let auth = check_endpoint(
        "认证中心",
        &config.auth_base_url,
        config.request_timeout_seconds,
    );
    let gateway = check_endpoint(
        "业务网关",
        &config.gateway_base_url,
        config.request_timeout_seconds,
    );

    Ok(OnlineConnectionCheckResult {
        ok: auth.ok && gateway.ok,
        auth,
        gateway,
    })
}

fn ensure_online_flavor() -> Result<(), String> {
    if flavor::active_flavor().remote_endpoint_required() {
        return Ok(());
    }

    Err("当前 Desktop flavor 不需要远端配置。".to_string())
}

fn config_path(app: &AppHandle) -> Result<PathBuf, String> {
    let config_dir = app
        .path()
        .app_config_dir()
        .map_err(|error| format!("定位 Desktop 配置目录失败：{error}"))?;
    Ok(config_dir.join(CONFIG_FILE_NAME))
}

fn read_config(path: &PathBuf) -> Result<Option<OnlineConfig>, String> {
    if !path.exists() {
        return Ok(None);
    }

    let content = fs::read_to_string(path)
        .map_err(|error| format!("读取 Desktop Online 配置失败：{error}"))?;
    let input: OnlineConfigInput = serde_json::from_str(&content)
        .map_err(|error| format!("解析 Desktop Online 配置失败：{error}"))?;
    input.normalize().map(Some)
}

fn check_endpoint(name: &str, base_url: &str, timeout_seconds: u64) -> OnlineEndpointCheck {
    let started = Instant::now();
    let url = match health_url(base_url) {
        Ok(value) => value,
        Err(message) => {
            return OnlineEndpointCheck {
                ok: false,
                url: base_url.to_string(),
                status_code: None,
                elapsed_ms: started.elapsed().as_millis(),
                message,
            }
        }
    };

    match fetch_status(&url, timeout_seconds) {
        Ok(status_code) if (200..300).contains(&status_code) => OnlineEndpointCheck {
            ok: true,
            url,
            status_code: Some(status_code),
            elapsed_ms: started.elapsed().as_millis(),
            message: format!("{name} 连接正常。"),
        },
        Ok(status_code) => OnlineEndpointCheck {
            ok: false,
            url,
            status_code: Some(status_code),
            elapsed_ms: started.elapsed().as_millis(),
            message: format!("{name} 健康检查返回 HTTP {status_code}。"),
        },
        Err(message) => OnlineEndpointCheck {
            ok: false,
            url,
            status_code: None,
            elapsed_ms: started.elapsed().as_millis(),
            message: format!("{name} 连接失败：{message}"),
        },
    }
}

fn fetch_status(url: &str, timeout_seconds: u64) -> Result<u16, String> {
    let agent = ureq::AgentBuilder::new()
        .timeout(Duration::from_secs(timeout_seconds))
        .build();

    match agent.get(url).call() {
        Ok(response) => Ok(response.status()),
        Err(ureq::Error::Status(status, _response)) => Ok(status),
        Err(error) => Err(error.to_string()),
    }
}

fn health_url(base_url: &str) -> Result<String, String> {
    let mut url = parse_base_url("远端地址", base_url)?;
    let base_path = url.path().trim_end_matches('/');
    url.set_path(&format!("{base_path}{HEALTH_PATH}"));
    Ok(url.to_string())
}

fn normalize_base_url(name: &str, value: &str) -> Result<String, String> {
    let url = parse_base_url(name, value)?;
    Ok(url.as_str().trim_end_matches('/').to_string())
}

fn parse_base_url(name: &str, value: &str) -> Result<Url, String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(format!("{name} 不能为空。"));
    }

    let mut url = Url::parse(trimmed).map_err(|error| format!("{name} 不是有效 URL：{error}"))?;
    if !matches!(url.scheme(), "http" | "https") {
        return Err(format!("{name} 只允许 http 或 https。"));
    }
    if url.cannot_be_a_base() || url.host_str().is_none() {
        return Err(format!("{name} 必须包含主机名。"));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(format!("{name} 不能包含用户名或密码。"));
    }
    if url.query().is_some() || url.fragment().is_some() {
        return Err(format!("{name} 不能包含 query 或 fragment。"));
    }

    url.set_query(None);
    url.set_fragment(None);
    Ok(url)
}

fn validate_timeout_seconds(value: Option<u64>) -> Result<u64, String> {
    let timeout = value.unwrap_or(DEFAULT_TIMEOUT_SECONDS);
    if (MIN_TIMEOUT_SECONDS..=MAX_TIMEOUT_SECONDS).contains(&timeout) {
        return Ok(timeout);
    }

    Err(format!(
        "连接超时必须在 {MIN_TIMEOUT_SECONDS} 到 {MAX_TIMEOUT_SECONDS} 秒之间。"
    ))
}

impl OnlineConfigInput {
    fn normalize(&self) -> Result<OnlineConfig, String> {
        Ok(OnlineConfig {
            auth_base_url: normalize_base_url("认证中心地址", &self.auth_base_url)?,
            gateway_base_url: normalize_base_url("业务网关地址", &self.gateway_base_url)?,
            request_timeout_seconds: validate_timeout_seconds(self.request_timeout_seconds)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_online_config_boundary_input() {
        let input = OnlineConfigInput {
            auth_base_url: " https://auth.example.com/ ".to_string(),
            gateway_base_url: "https://api.example.com/gateway/".to_string(),
            request_timeout_seconds: None,
        };
        let config = input.normalize().unwrap();

        assert_eq!(config.auth_base_url, "https://auth.example.com");
        assert_eq!(config.gateway_base_url, "https://api.example.com/gateway");
        assert_eq!(config.request_timeout_seconds, DEFAULT_TIMEOUT_SECONDS);
    }

    #[test]
    fn rejects_url_with_credentials() {
        let input = OnlineConfigInput {
            auth_base_url: "https://user:secret@example.com".to_string(),
            gateway_base_url: "https://api.example.com".to_string(),
            request_timeout_seconds: Some(10),
        };

        assert!(input.normalize().is_err());
    }

    #[test]
    fn builds_health_url_below_base_path() {
        let url = health_url("https://api.example.com/root").unwrap();

        assert_eq!(url, "https://api.example.com/root/actuator/health");
    }

    #[test]
    fn rejects_timeout_outside_boundary() {
        assert!(validate_timeout_seconds(Some(0)).is_err());
        assert!(validate_timeout_seconds(Some(MAX_TIMEOUT_SECONDS + 1)).is_err());
    }
}
