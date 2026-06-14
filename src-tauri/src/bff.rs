use serde::{de::DeserializeOwned, Deserialize, Serialize};
use serde_json::{Map, Value};
use tauri::AppHandle;
use crate::{
    flavor,
    local_http::{http_request, HttpRequest, LOCAL_HOST},
    online_config,
    online_session::{self, OnlineSessionHolder},
    sidecar::{BackendSidecar, LocalBackendSession},
};

const DESKTOP_CSRF_TOKEN: &str = "desktop-csrf-token-000000000000000000000000000000000000";

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendAuthUser {
    id: u64,
    display_name: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebAuthPublicSession {
    authenticated: bool,
    csrf_token: String,
    access_token_expires_at: Option<String>,
    refresh_token_expires_at: Option<String>,
    sid: Option<String>,
    actor_type: Option<String>,
    subject: Option<String>,
    user: Option<BackendAuthUser>,
    roles: Vec<String>,
    permissions: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebAuthLoginRequest {
    identifier: String,
    password: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeInfo {
    application: String,
    topology: String,
    java_version: String,
    native_image: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolRecord {
    id: u64,
    tool_key: String,
    display_name: String,
    #[serde(default)]
    description: Option<String>,
    enabled: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateToolRequest {
    tool_key: String,
    display_name: String,
    #[serde(default)]
    description: Option<String>,
}

#[tauri::command]
pub fn hdx_auth_session(
    _app: AppHandle,
    online_session: tauri::State<'_, OnlineSessionHolder>,
) -> WebAuthPublicSession {
    if flavor::active_flavor().includes_full_backend() {
        return local_admin_session();
    }

    online_remote_session(&online_session)
}

#[tauri::command]
pub fn hdx_auth_login(
    app: AppHandle,
    online_session: tauri::State<'_, OnlineSessionHolder>,
    input: WebAuthLoginRequest,
) -> Result<WebAuthPublicSession, String> {
    input.validate()?;

    if flavor::active_flavor().includes_full_backend() {
        return Ok(local_admin_session());
    }

    let config = require_online_config(&app)?;
    let public = online_session.login(&config, &input.identifier, &input.password)?;
    Ok(online_public_to_web_session(&public))
}

#[tauri::command]
pub fn hdx_auth_logout(
    app: AppHandle,
    online_session: tauri::State<'_, OnlineSessionHolder>,
) -> WebAuthPublicSession {
    if flavor::active_flavor().includes_full_backend() {
        return local_admin_session();
    }

    match online_config::read_app_config(&app) {
        Ok(Some(config)) => {
            let public = online_session.logout(&config).unwrap_or_else(|_| {
                online_session::OnlinePublicSession::anonymous_pub()
            });
            online_public_to_web_session(&public)
        }
        Ok(None) => anonymous_session(),
        Err(_) => anonymous_session(),
    }
}

#[tauri::command]
pub fn hdx_runtime_info(
    app: AppHandle,
    online_session: tauri::State<'_, OnlineSessionHolder>,
    backend_sidecar: tauri::State<'_, BackendSidecar>,
) -> Result<RuntimeInfo, String> {
    if flavor::active_flavor().includes_full_backend() {
        let session = require_local_backend_session(&backend_sidecar)?;
        let runtime = fetch_local_json::<RuntimeInfo>(&session, "/api/v1/runtime", "GET", None)?;
        runtime.validate()?;
        return Ok(runtime);
    }

    let config = require_online_config(&app)?;
    let access_token = online_session.ensure_access_token(&config)?;
    let runtime = online_session::fetch_remote_business::<RuntimeInfo>(
        &config,
        &access_token,
        "/api/v1/runtime",
        "GET",
        None,
    )?;
    runtime.validate()?;
    Ok(runtime)
}

#[tauri::command]
pub fn hdx_tools_list(
    app: AppHandle,
    online_session: tauri::State<'_, OnlineSessionHolder>,
    backend_sidecar: tauri::State<'_, BackendSidecar>,
) -> Result<Vec<ToolRecord>, String> {
    if flavor::active_flavor().includes_full_backend() {
        let session = require_local_backend_session(&backend_sidecar)?;
        let tools = fetch_local_json::<Vec<ToolRecord>>(&session, "/api/v1/tools", "GET", None)?;
        for tool in &tools {
            tool.validate()?;
        }
        return Ok(tools);
    }

    let config = require_online_config(&app)?;
    let access_token = online_session.ensure_access_token(&config)?;
    let tools = online_session::fetch_remote_business::<Vec<ToolRecord>>(
        &config,
        &access_token,
        "/api/v1/tools",
        "GET",
        None,
    )?;

    for tool in &tools {
        tool.validate()?;
    }

    Ok(tools)
}

#[tauri::command]
pub fn hdx_tools_create(
    app: AppHandle,
    online_session: tauri::State<'_, OnlineSessionHolder>,
    backend_sidecar: tauri::State<'_, BackendSidecar>,
    input: CreateToolRequest,
) -> Result<ToolRecord, String> {
    let body = input.to_backend_body()?;

    if flavor::active_flavor().includes_full_backend() {
        let session = require_local_backend_session(&backend_sidecar)?;
        let tool = fetch_local_json::<ToolRecord>(&session, "/api/v1/tools", "POST", Some(body))?;
        tool.validate()?;
        return Ok(tool);
    }

    let config = require_online_config(&app)?;
    let access_token = online_session.ensure_access_token(&config)?;
    let tool = online_session::fetch_remote_business::<ToolRecord>(
        &config,
        &access_token,
        "/api/v1/tools",
        "POST",
        Some(&body),
    )?;
    tool.validate()?;
    Ok(tool)
}

fn anonymous_session() -> WebAuthPublicSession {
    WebAuthPublicSession {
        authenticated: false,
        csrf_token: DESKTOP_CSRF_TOKEN.to_string(),
        access_token_expires_at: None,
        refresh_token_expires_at: None,
        sid: None,
        actor_type: None,
        subject: None,
        user: None,
        roles: Vec::new(),
        permissions: Vec::new(),
    }
}

fn local_admin_session() -> WebAuthPublicSession {
    WebAuthPublicSession {
        authenticated: true,
        csrf_token: DESKTOP_CSRF_TOKEN.to_string(),
        access_token_expires_at: None,
        refresh_token_expires_at: None,
        sid: Some("local-admin".to_string()),
        actor_type: Some("LOCAL_ADMIN".to_string()),
        subject: Some("local-admin".to_string()),
        user: Some(BackendAuthUser {
            id: 0,
            display_name: "用户".to_string(),
        }),
        roles: vec!["ADMIN".to_string()],
        permissions: vec!["*".to_string()],
    }
}

fn require_local_backend_session(sidecar: &BackendSidecar) -> Result<LocalBackendSession, String> {
    sidecar
        .local_backend_session()
        .ok_or_else(|| "本机后端尚未就绪，请稍后重试。".to_string())
}

fn require_online_config(app: &AppHandle) -> Result<crate::online_config::OnlineConfig, String> {
    online_config::read_app_config(app)?
        .ok_or_else(|| "请先配置 Desktop Online 远端服务地址。".to_string())
}

fn online_remote_session(
    online_session: &OnlineSessionHolder,
) -> WebAuthPublicSession {
    let public = online_session.snapshot();
    online_public_to_web_session(&public)
}

fn online_public_to_web_session(
    public: &online_session::OnlinePublicSession,
) -> WebAuthPublicSession {
    if public.authenticated {
        let user = public.user.as_ref().map(|u| BackendAuthUser {
            id: u.id,
            display_name: u.display_name.clone(),
        });
        WebAuthPublicSession {
            authenticated: true,
            csrf_token: DESKTOP_CSRF_TOKEN.to_string(),
            access_token_expires_at: public.access_token_expires_at.clone(),
            refresh_token_expires_at: public.refresh_token_expires_at.clone(),
            sid: public.sid.clone(),
            actor_type: Some("USER".to_string()),
            subject: public.user.as_ref().map(|u| format!("USER:{}", u.id)),
            user,
            roles: public.roles.clone(),
            permissions: public.permissions.clone(),
        }
    } else {
        anonymous_session()
    }
}

fn fetch_local_json<T>(
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

fn parse_local_backend_port(base_url: &str) -> Result<u16, String> {
    let prefix = format!("http://{LOCAL_HOST}:");
    let port_text = base_url
        .strip_prefix(&prefix)
        .ok_or_else(|| format!("本机后端地址必须绑定 {LOCAL_HOST}：{base_url}"))?;

    port_text
        .parse::<u16>()
        .map_err(|error| format!("解析本机后端端口失败：{error}"))
}

impl WebAuthLoginRequest {
    fn validate(&self) -> Result<(), String> {
        validate_trimmed_text("登录账号", &self.identifier, 1, 320)?;
        validate_text("登录密码", &self.password, 1, 200)
    }
}

impl RuntimeInfo {
    fn validate(&self) -> Result<(), String> {
        validate_trimmed_text("runtime.application", &self.application, 1, 200)?;
        validate_trimmed_text("runtime.topology", &self.topology, 1, 200)?;
        validate_trimmed_text("runtime.javaVersion", &self.java_version, 1, 80)?;
        Ok(())
    }
}

impl ToolRecord {
    fn validate(&self) -> Result<(), String> {
        validate_trimmed_text("tool.toolKey", &self.tool_key, 1, 80)?;
        validate_trimmed_text("tool.displayName", &self.display_name, 1, 120)?;
        if let Some(description) = &self.description {
            validate_text("tool.description", description, 0, 500)?;
        }
        validate_trimmed_text("tool.createdAt", &self.created_at, 1, 80)?;
        validate_trimmed_text("tool.updatedAt", &self.updated_at, 1, 80)?;
        Ok(())
    }
}

impl CreateToolRequest {
    fn to_backend_body(&self) -> Result<Value, String> {
        let tool_key = validate_trimmed_text("工具标识", &self.tool_key, 1, 80)?;
        let display_name = validate_trimmed_text("工具名称", &self.display_name, 1, 120)?;
        let mut body = Map::new();
        body.insert("toolKey".to_string(), Value::String(tool_key));
        body.insert("displayName".to_string(), Value::String(display_name));

        if let Some(value) = &self.description {
            body.insert(
                "description".to_string(),
                Value::String(validate_trimmed_text("工具说明", value, 0, 500)?),
            );
        }

        Ok(Value::Object(body))
    }
}

fn validate_trimmed_text(
    name: &str,
    value: &str,
    min: usize,
    max: usize,
) -> Result<String, String> {
    let trimmed = value.trim();
    validate_text(name, trimmed, min, max)?;
    Ok(trimmed.to_string())
}

fn validate_text(name: &str, value: &str, min: usize, max: usize) -> Result<(), String> {
    let length = value.chars().count();

    if length < min || length > max {
        return Err(format!("{name} 长度必须在 {min} 到 {max} 个字符之间。"));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_admin_session_does_not_serialize_backend_token() {
        let json = serde_json::to_string(&local_admin_session()).unwrap();

        assert!(json.contains("LOCAL_ADMIN"));
        assert!(!json.contains("headerName"));
        assert!(!json.contains("localToken"));
    }

    #[test]
    fn parse_local_backend_port_rejects_remote_url() {
        assert!(parse_local_backend_port("https://example.com").is_err());
    }

    #[test]
    fn create_tool_request_trims_desktop_boundary_input() {
        let input = CreateToolRequest {
            tool_key: " demo ".to_string(),
            display_name: " Demo ".to_string(),
            description: Some(" description ".to_string()),
        };
        let body = input.to_backend_body().unwrap();

        assert_eq!(body["toolKey"], "demo");
        assert_eq!(body["displayName"], "Demo");
        assert_eq!(body["description"], "description");
    }
}
