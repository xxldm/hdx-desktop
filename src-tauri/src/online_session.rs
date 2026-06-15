use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};

use crate::online_config::OnlineConfig;

mod http;
mod time;

pub(crate) use http::fetch_remote_business;
use http::{post_auth, post_auth_raw};
use time::{parse_iso_timestamp, should_refresh};

/// access token 临近过期时提前刷新的时间窗口（秒）。
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
    access_token_expires_iso: String,
    refresh_token: String,
    #[allow(dead_code)]
    refresh_token_expires_at: u64,
    refresh_token_expires_iso: String,
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
    pub access_token_expires_at: Option<String>,
    pub refresh_token_expires_at: Option<String>,
    pub sid: Option<String>,
    pub user: Option<RemoteAuthUser>,
    pub roles: Vec<String>,
    pub permissions: Vec<String>,
}

impl OnlinePublicSession {
    fn anonymous() -> Self {
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

        Ok(self.clear_local())
    }

    pub fn clear_local(&self) -> OnlinePublicSession {
        let mut inner = self.lock();
        inner.session = None;
        OnlinePublicSession::anonymous()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, OnlineSessionInner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
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
            access_token_expires_iso: response.access_token_expires_at,
            refresh_token: response.refresh_token,
            refresh_token_expires_at,
            refresh_token_expires_iso: response.refresh_token_expires_at,
            sid: response.sid,
            user: response.user,
            roles: response.roles,
            permissions: response.permissions,
        }
    }

    fn to_public(&self) -> OnlinePublicSession {
        OnlinePublicSession {
            authenticated: true,
            access_token_expires_at: Some(self.access_token_expires_iso.clone()),
            refresh_token_expires_at: Some(self.refresh_token_expires_iso.clone()),
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
    use crate::online_session::time::now_epoch_seconds;

    fn fake_token_session() -> RemoteTokenSession {
        RemoteTokenSession {
            access_token: "access-secret".to_string(),
            access_token_expires_at: now_epoch_seconds() + 3600,
            access_token_expires_iso: "2026-06-13T12:00:00Z".to_string(),
            refresh_token: "refresh-secret".to_string(),
            refresh_token_expires_at: now_epoch_seconds() + 86400,
            refresh_token_expires_iso: "2026-06-20T12:00:00Z".to_string(),
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
    fn clear_local_removes_tokens_from_holder() {
        let holder = OnlineSessionHolder::default();
        {
            let mut inner = holder.lock();
            inner.session = Some(fake_token_session());
        }

        let public = holder.clear_local();

        assert!(!public.authenticated);
        assert!(!holder.snapshot().authenticated);
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
            http::build_url("https://api.example.com", "/api/v1/tools"),
            "https://api.example.com/api/v1/tools"
        );
        assert_eq!(
            http::build_url("https://api.example.com/gateway/", "/api/v1/tools"),
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
