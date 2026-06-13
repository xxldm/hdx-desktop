use std::{
    sync::{Arc, Mutex},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use serde::{de::DeserializeOwned, Deserialize, Serialize};
use ureq::{Agent, AgentBuilder};

use crate::online_config::OnlineConfig;

/// access token 临近过期时提前刷新的时间窗口（秒）。
const REFRESH_SKEW_SECONDS: u64 = 60;
const CLIENT_TYPE_DESKTOP: &str = "DESKTOP";

/// Rust 主进程持有的远端登录态。access token / refresh token 只存在这里，
/// 不出现在任何 Tauri command 的返回值中。
#[derive(Default)]
struct OnlineSessionInner {
    session: Option<RemoteTokenSession>,
}

#[derive(Clone)]
struct RemoteTokenSession {
    access_token: String,
    access_token_expires_at: u64,
    refresh_token: String,
    refresh_token_expires_at: u64,
    sid: String,
    user: RemoteAuthUser,
    roles: Vec<String>,
    permissions: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RemoteAuthUser {
    pub id: u64,
    pub display_name: String,
}

/// WebView 可见的 public session 投影——不包含任何 token。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OnlinePublicSession {
    pub authenticated: bool,
    pub access_token_expires_at: Option<u64>,
    pub refresh_token_expires_at: Option<u64>,
    pub sid: Option<String>,
    pub user: Option<RemoteAuthUser>,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
}

impl OnlinePublicSession {
    fn anonymous() -> Self {
        Self::anonymous_pub()
    }

    pub fn anonymous_pub() -> Self {
        Self {
            authenticated: false,
            access_token_expires_at: None,
            refresh_token_expires_at: None,
            sid: None,
            user: None,
            roles: Vec::new(),
            permissions: Vec::new(),
        }
    }
}

#[derive(Clone, Default)]
pub struct OnlineSessionHolder {
    inner: Arc<Mutex<OnlineSessionInner>>,
}

/// 远端 HTTP 响应中 AuthTokenResponse 的最小反序列化结构。
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AuthTokenResponse {
    #[allow(dead_code)]
    token_type: String,
    access_token: String,
    access_token_expires_at: String,
    refresh_token: String,
    refresh_token_expires_at: String,
    sid: String,
    user: RemoteAuthUser,
    roles: Vec<String>,
    permissions: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LoginBody<'a> {
    identifier: &'a str,
    password: &'a str,
    client_type: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RefreshBody<'a> {
    refresh_token: &'a str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LogoutBody<'a> {
    refresh_token: &'a str,
}

/// 远端请求失败时携带 HTTP 状态码的错误。
struct RemoteError {
    status_code: Option<u16>,
    message: String,
}

impl RemoteError {
    fn is_unauthorized(&self) -> bool {
        matches!(self.status_code, Some(401))
    }

    fn to_message(&self) -> String {
        self.message.clone()
    }

    fn status(&self) -> Option<u16> {
        self.status_code
    }
}

impl OnlineSessionHolder {
    pub fn snapshot(&self) -> OnlinePublicSession {
        let inner = self.lock();
        match &inner.session {
            Some(session) => session.to_public(),
            None => OnlinePublicSession::anonymous(),
        }
    }

    pub fn login(
        &self,
        config: &OnlineConfig,
        identifier: &str,
        password: &str,
    ) -> Result<OnlinePublicSession, String> {
        let body = LoginBody {
            identifier,
            password,
            client_type: CLIENT_TYPE_DESKTOP,
        };

        let token_response = post_auth::<AuthTokenResponse>(config, "/api/auth/login", &body)
            .map_err(|error| error.to_message())?;
        let session = RemoteTokenSession::from_token_response(token_response);
        let public = session.to_public();

        let mut inner = self.lock();
        inner.session = Some(session);
        Ok(public)
    }

    /// 如果 access token 临近过期则自动刷新；返回当前可用的 access token。
    pub fn ensure_access_token(&self, config: &OnlineConfig) -> Result<String, String> {
        let session = {
            let inner = self.lock();
            inner
                .session
                .clone()
                .ok_or_else(|| "请先登录。".to_string())?
        };

        if !should_refresh(session.access_token_expires_at) {
            return Ok(session.access_token);
        }

        self.refresh(config)
    }

    pub fn refresh(&self, config: &OnlineConfig) -> Result<String, String> {
        let refresh_token = {
            let inner = self.lock();
            match &inner.session {
                Some(session) => session.refresh_token.clone(),
                None => return Err("登录已过期，请重新登录。".to_string()),
            }
        };

        let body = RefreshBody {
            refresh_token: &refresh_token,
        };

        match post_auth::<AuthTokenResponse>(config, "/api/auth/refresh", &body) {
            Ok(token_response) => {
                let new_session = RemoteTokenSession::from_token_response(token_response);
                let access_token = new_session.access_token.clone();
                let mut inner = self.lock();
                inner.session = Some(new_session);
                Ok(access_token)
            }
            Err(error) if error.is_unauthorized() => {
                let mut inner = self.lock();
                inner.session = None;
                Err("登录已过期，请重新登录。".to_string())
            }
            Err(error) => Err(error.to_message()),
        }
    }

    pub fn logout(&self, config: &OnlineConfig) -> Result<OnlinePublicSession, String> {
        let refresh_token = {
            let inner = self.lock();
            inner
                .session
                .as_ref()
                .map(|session| session.refresh_token.clone())
        };

        if let Some(refresh_token) = refresh_token {
            let body = LogoutBody {
                refresh_token: &refresh_token,
            };
            // 后端 logout 失败时本地仍然清理 session，只影响远端 sid 是否已撤销。
            let _ = post_auth_raw(config, "/api/auth/logout", &body);
        }

        let mut inner = self.lock();
        inner.session = None;
        Ok(OnlinePublicSession::anonymous())
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, OnlineSessionInner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn build_agent(config: &OnlineConfig) -> Agent {
    AgentBuilder::new()
        .timeout(Duration::from_secs(config.request_timeout_seconds))
        .build()
}

fn build_url(base_url: &str, path: &str) -> String {
    format!("{}{path}", base_url.trim_end_matches('/'))
}

fn post_auth<T: DeserializeOwned>(
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

fn post_auth_raw(
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
            response.into_json::<serde_json::Value>().map_err(|error| {
                RemoteError {
                    status_code: None,
                    message: format!("读取认证中心响应失败：{error}"),
                }
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
pub fn fetch_remote_business<T: DeserializeOwned>(
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

    let response = if method == "POST" {
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

fn now_epoch_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn should_refresh(access_token_expires_at: u64) -> bool {
    access_token_expires_at.saturating_sub(now_epoch_seconds()) <= REFRESH_SKEW_SECONDS
}

fn parse_iso_timestamp(value: &str) -> Result<u64, String> {
    let trimmed = value.trim();
    // 后端返回 Instant.toString() 格式，固定以 Z 结尾（UTC）。
    if !trimmed.ends_with('Z') {
        return Err(format!("认证中心返回的时间格式无效：{value}"));
    }

    let datetime_part = &trimmed[..trimmed.len() - 1];
    let (date_part, time_part) = datetime_part
        .split_once('T')
        .ok_or_else(|| format!("认证中心返回的时间格式无效：{value}"))?;

    let (year, month, day) = parse_date(date_part)?;
    let (hour, minute, second) = parse_time(time_part)?;

    Ok(epoch_seconds(year, month, day, hour, minute, second))
}

fn parse_date(part: &str) -> Result<(u32, u32, u32), String> {
    let segments: Vec<&str> = part.split('-').collect();
    if segments.len() != 3 {
        return Err(format!("日期格式无效：{part}"));
    }
    let year = segments[0]
        .parse::<u32>()
        .map_err(|_| format!("年份无效：{part}"))?;
    let month = segments[1]
        .parse::<u32>()
        .map_err(|_| format!("月份无效：{part}"))?;
    let day = segments[2]
        .parse::<u32>()
        .map_err(|_| format!("日期无效：{part}"))?;
    Ok((year, month, day))
}

fn parse_time(part: &str) -> Result<(u32, u32, u32), String> {
    // 去掉可能的毫秒部分（如 12:00:00.123）。
    let main = part.split('.').next().unwrap_or(part);
    let segments: Vec<&str> = main.split(':').collect();
    if segments.len() < 2 || segments.len() > 3 {
        return Err(format!("时间格式无效：{part}"));
    }
    let hour = segments[0]
        .parse::<u32>()
        .map_err(|_| format!("小时无效：{part}"))?;
    let minute = segments[1]
        .parse::<u32>()
        .map_err(|_| format!("分钟无效：{part}"))?;
    let second = if segments.len() == 3 {
        segments[2]
            .parse::<u32>()
            .map_err(|_| format!("秒无效：{part}"))?
    } else {
        0
    };
    Ok((hour, minute, second))
}

/// 公历 UTC epoch 秒（Howard Hinnant 算法，不处理闰秒，精度对 token 过期判断足够）。
fn epoch_seconds(year: u32, month: u32, day: u32, hour: u32, minute: u32, second: u32) -> u64 {
    let y = if month <= 2 { year - 1 } else { year } as i64;
    let m = month as i64;
    let d = day as i64;
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = (y - era * 400) as u64;
    let doy = (153 * (if m > 2 { m - 3 } else { m + 9 }) + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy as u64;
    (era * 146097 + doe as i64 - 719468) as u64 * 86400
        + hour as u64 * 3600
        + minute as u64 * 60
        + second as u64
}

impl RemoteTokenSession {
    fn from_token_response(response: AuthTokenResponse) -> Self {
        let access_token_expires_at =
            parse_iso_timestamp(&response.access_token_expires_at).unwrap_or(0);
        let refresh_token_expires_at =
            parse_iso_timestamp(&response.refresh_token_expires_at).unwrap_or(0);

        Self {
            access_token: response.access_token,
            access_token_expires_at,
            refresh_token: response.refresh_token,
            refresh_token_expires_at,
            sid: response.sid,
            user: response.user,
            roles: response.roles,
            permissions: response.permissions,
        }
    }

    fn to_public(&self) -> OnlinePublicSession {
        OnlinePublicSession {
            authenticated: true,
            access_token_expires_at: Some(self.access_token_expires_at),
            refresh_token_expires_at: Some(self.refresh_token_expires_at),
            sid: Some(self.sid.clone()),
            user: Some(self.user.clone()),
            roles: self.roles.clone(),
            permissions: self.permissions.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fake_token_session() -> RemoteTokenSession {
        RemoteTokenSession {
            access_token: "access-secret".to_string(),
            access_token_expires_at: now_epoch_seconds() + 3600,
            refresh_token: "refresh-secret".to_string(),
            refresh_token_expires_at: now_epoch_seconds() + 86400,
            sid: "sid-123".to_string(),
            user: RemoteAuthUser {
                id: 1,
                display_name: "测试用户".to_string(),
            },
            roles: vec!["ADMIN".to_string()],
            permissions: vec!["*".to_string()],
        }
    }

    #[test]
    fn public_session_does_not_serialize_tokens() {
        let holder = OnlineSessionHolder::default();
        {
            let mut inner = holder.lock();
            inner.session = Some(fake_token_session());
        }

        let json = serde_json::to_string(&holder.snapshot()).unwrap();

        assert!(json.contains("测试用户"));
        assert!(json.contains("sid-123"));
        assert!(!json.contains("access-secret"));
        assert!(!json.contains("refresh-secret"));
    }

    #[test]
    fn anonymous_session_has_no_user() {
        let holder = OnlineSessionHolder::default();
        let session = holder.snapshot();

        assert!(!session.authenticated);
        assert!(session.user.is_none());
        assert!(session.sid.is_none());
    }

    #[test]
    fn should_refresh_when_close_to_expiry() {
        let now = now_epoch_seconds();
        assert!(should_refresh(now + 30));
        assert!(should_refresh(now));
        assert!(!should_refresh(now + 3600));
    }

    #[test]
    fn build_url_joins_base_and_path() {
        assert_eq!(
            build_url("https://api.example.com", "/api/v1/tools"),
            "https://api.example.com/api/v1/tools"
        );
        assert_eq!(
            build_url("https://api.example.com/gateway/", "/api/v1/tools"),
            "https://api.example.com/gateway/api/v1/tools"
        );
    }

    #[test]
    fn parses_iso_timestamp() {
        assert_eq!(parse_iso_timestamp("1970-01-01T00:00:00Z").unwrap(), 0);
        assert_eq!(parse_iso_timestamp("1970-01-01T00:00:01Z").unwrap(), 1);
        assert_eq!(
            parse_iso_timestamp("2026-06-13T12:30:45Z").unwrap(),
            1781353845
        );
    }

    #[test]
    fn rejects_non_utc_timestamp() {
        assert!(parse_iso_timestamp("2026-06-13T12:30:45+08:00").is_err());
    }

    #[test]
    fn token_response_parsing_assigns_expiry() {
        let response = AuthTokenResponse {
            token_type: "Bearer".to_string(),
            access_token: "a".to_string(),
            access_token_expires_at: "2026-06-13T12:00:00Z".to_string(),
            refresh_token: "r".to_string(),
            refresh_token_expires_at: "2026-06-20T12:00:00Z".to_string(),
            sid: "sid".to_string(),
            user: RemoteAuthUser {
                id: 42,
                display_name: "用户".to_string(),
            },
            roles: vec!["ADMIN".to_string()],
            permissions: vec!["*".to_string()],
        };

        let session = RemoteTokenSession::from_token_response(response);

        assert_eq!(session.access_token_expires_at, 1781352000);
        assert_eq!(session.refresh_token_expires_at, 1781956800);
        assert_eq!(session.sid, "sid");
    }
}
