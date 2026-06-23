use crate::{
    flavor, online_config,
    online_session::{self, OnlineSessionHolder},
    sidecar::BackendSidecar,
};
use tauri::AppHandle;

mod dto;
mod local_backend;

pub use dto::{
    BackendAuthUser, CreateToolRequest, HolidayAdminRecord, HolidayCreateRequest, HolidayRecord,
    HolidayUpdateRequest, RuntimeInfo, TimerPreference, TimerPreferenceSaveRequest, ToolRecord,
    UserPreference, UserPreferenceSaveRequest, WebAuthLoginRequest, WebAuthPublicSession,
    WorkbenchLayout,
};
use local_backend::fetch_local_json;

const DESKTOP_CSRF_TOKEN: &str = "desktop-csrf-token-000000000000000000000000000000000000";

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
            let public = online_session
                .logout(&config)
                .unwrap_or_else(|_| online_session.clear_local());
            online_public_to_web_session(&public)
        }
        Ok(None) | Err(_) => online_public_to_web_session(&online_session.clear_local()),
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

#[tauri::command]
pub fn hdx_holidays_list(
    app: AppHandle,
    online_session: tauri::State<'_, OnlineSessionHolder>,
    backend_sidecar: tauri::State<'_, BackendSidecar>,
) -> Result<Vec<HolidayRecord>, String> {
    if flavor::active_flavor().includes_full_backend() {
        let session = require_local_backend_session(&backend_sidecar)?;
        let holidays =
            fetch_local_json::<Vec<HolidayRecord>>(&session, "/api/v1/holidays", "GET", None)?;
        for holiday in &holidays {
            holiday.validate()?;
        }
        return Ok(holidays);
    }

    let config = require_online_config(&app)?;
    let access_token = online_session.ensure_access_token(&config)?;
    let holidays = online_session::fetch_remote_business::<Vec<HolidayRecord>>(
        &config,
        &access_token,
        "/api/v1/holidays",
        "GET",
        None,
    )?;

    for holiday in &holidays {
        holiday.validate()?;
    }

    Ok(holidays)
}

#[tauri::command]
pub fn hdx_admin_holidays_list(
    app: AppHandle,
    online_session: tauri::State<'_, OnlineSessionHolder>,
    backend_sidecar: tauri::State<'_, BackendSidecar>,
) -> Result<Vec<HolidayAdminRecord>, String> {
    if flavor::active_flavor().includes_full_backend() {
        let session = require_local_backend_session(&backend_sidecar)?;
        let holidays = fetch_local_json::<Vec<HolidayAdminRecord>>(
            &session,
            "/api/v1/admin/holidays",
            "GET",
            None,
        )?;
        for holiday in &holidays {
            holiday.validate()?;
        }
        return Ok(holidays);
    }

    let config = require_online_config(&app)?;
    let access_token = online_session.ensure_access_token(&config)?;
    let holidays = online_session::fetch_remote_business::<Vec<HolidayAdminRecord>>(
        &config,
        &access_token,
        "/api/v1/admin/holidays",
        "GET",
        None,
    )?;

    for holiday in &holidays {
        holiday.validate()?;
    }

    Ok(holidays)
}

#[tauri::command]
pub fn hdx_admin_holidays_create(
    app: AppHandle,
    online_session: tauri::State<'_, OnlineSessionHolder>,
    backend_sidecar: tauri::State<'_, BackendSidecar>,
    input: HolidayCreateRequest,
) -> Result<HolidayAdminRecord, String> {
    let body = input.to_backend_body()?;

    if flavor::active_flavor().includes_full_backend() {
        let session = require_local_backend_session(&backend_sidecar)?;
        let holiday = fetch_local_json::<HolidayAdminRecord>(
            &session,
            "/api/v1/admin/holidays",
            "POST",
            Some(body),
        )?;
        holiday.validate()?;
        return Ok(holiday);
    }

    let config = require_online_config(&app)?;
    let access_token = online_session.ensure_access_token(&config)?;
    let holiday = online_session::fetch_remote_business::<HolidayAdminRecord>(
        &config,
        &access_token,
        "/api/v1/admin/holidays",
        "POST",
        Some(&body),
    )?;
    holiday.validate()?;
    Ok(holiday)
}

#[tauri::command]
pub fn hdx_admin_holidays_update(
    app: AppHandle,
    online_session: tauri::State<'_, OnlineSessionHolder>,
    backend_sidecar: tauri::State<'_, BackendSidecar>,
    id: u64,
    input: HolidayUpdateRequest,
) -> Result<HolidayAdminRecord, String> {
    validate_holiday_id(id)?;
    let body = input.to_backend_body()?;
    let path = format!("/api/v1/admin/holidays/{id}");

    if flavor::active_flavor().includes_full_backend() {
        let session = require_local_backend_session(&backend_sidecar)?;
        let holiday = fetch_local_json::<HolidayAdminRecord>(&session, &path, "PUT", Some(body))?;
        holiday.validate()?;
        return Ok(holiday);
    }

    let config = require_online_config(&app)?;
    let access_token = online_session.ensure_access_token(&config)?;
    let holiday = online_session::fetch_remote_business::<HolidayAdminRecord>(
        &config,
        &access_token,
        &path,
        "PUT",
        Some(&body),
    )?;
    holiday.validate()?;
    Ok(holiday)
}

#[tauri::command]
pub fn hdx_admin_holidays_delete(
    app: AppHandle,
    online_session: tauri::State<'_, OnlineSessionHolder>,
    backend_sidecar: tauri::State<'_, BackendSidecar>,
    id: u64,
    version: u32,
) -> Result<HolidayAdminRecord, String> {
    validate_holiday_id(id)?;
    validate_holiday_version(version)?;
    let path = format!("/api/v1/admin/holidays/{id}?version={version}");

    if flavor::active_flavor().includes_full_backend() {
        let session = require_local_backend_session(&backend_sidecar)?;
        let holiday = fetch_local_json::<HolidayAdminRecord>(&session, &path, "DELETE", None)?;
        holiday.validate()?;
        return Ok(holiday);
    }

    let config = require_online_config(&app)?;
    let access_token = online_session.ensure_access_token(&config)?;
    let holiday = online_session::fetch_remote_business::<HolidayAdminRecord>(
        &config,
        &access_token,
        &path,
        "DELETE",
        None,
    )?;
    holiday.validate()?;
    Ok(holiday)
}

#[tauri::command]
pub fn hdx_workbench_layout_get(
    app: AppHandle,
    online_session: tauri::State<'_, OnlineSessionHolder>,
    backend_sidecar: tauri::State<'_, BackendSidecar>,
) -> Result<WorkbenchLayout, String> {
    if flavor::active_flavor().includes_full_backend() {
        let session = require_local_backend_session(&backend_sidecar)?;
        let layout =
            fetch_local_json::<WorkbenchLayout>(&session, "/api/v1/workbench/layout", "GET", None)?;
        layout.validate()?;
        return Ok(layout);
    }

    let config = require_online_config(&app)?;
    let access_token = online_session.ensure_access_token(&config)?;
    let layout = online_session::fetch_remote_business::<WorkbenchLayout>(
        &config,
        &access_token,
        "/api/v1/workbench/layout",
        "GET",
        None,
    )?;
    layout.validate()?;
    Ok(layout)
}

#[tauri::command]
pub fn hdx_workbench_layout_save(
    app: AppHandle,
    online_session: tauri::State<'_, OnlineSessionHolder>,
    backend_sidecar: tauri::State<'_, BackendSidecar>,
    input: WorkbenchLayout,
) -> Result<WorkbenchLayout, String> {
    let body = input.to_backend_body()?;

    if flavor::active_flavor().includes_full_backend() {
        let session = require_local_backend_session(&backend_sidecar)?;
        let layout = fetch_local_json::<WorkbenchLayout>(
            &session,
            "/api/v1/workbench/layout",
            "PUT",
            Some(body),
        )?;
        layout.validate()?;
        return Ok(layout);
    }

    let config = require_online_config(&app)?;
    let access_token = online_session.ensure_access_token(&config)?;
    let layout = online_session::fetch_remote_business::<WorkbenchLayout>(
        &config,
        &access_token,
        "/api/v1/workbench/layout",
        "PUT",
        Some(&body),
    )?;
    layout.validate()?;
    Ok(layout)
}

#[tauri::command]
pub fn hdx_timer_preferences_get(
    app: AppHandle,
    online_session: tauri::State<'_, OnlineSessionHolder>,
    backend_sidecar: tauri::State<'_, BackendSidecar>,
) -> Result<TimerPreference, String> {
    if flavor::active_flavor().includes_full_backend() {
        let session = require_local_backend_session(&backend_sidecar)?;
        let preference = fetch_local_json::<TimerPreference>(
            &session,
            "/api/v1/timer/preferences",
            "GET",
            None,
        )?;
        preference.validate()?;
        return Ok(preference);
    }

    let config = require_online_config(&app)?;
    let access_token = online_session.ensure_access_token(&config)?;
    let preference = online_session::fetch_remote_business::<TimerPreference>(
        &config,
        &access_token,
        "/api/v1/timer/preferences",
        "GET",
        None,
    )?;
    preference.validate()?;
    Ok(preference)
}

#[tauri::command]
pub fn hdx_timer_preferences_save(
    app: AppHandle,
    online_session: tauri::State<'_, OnlineSessionHolder>,
    backend_sidecar: tauri::State<'_, BackendSidecar>,
    input: TimerPreferenceSaveRequest,
) -> Result<TimerPreference, String> {
    let body = input.to_backend_body()?;

    if flavor::active_flavor().includes_full_backend() {
        let session = require_local_backend_session(&backend_sidecar)?;
        let preference = fetch_local_json::<TimerPreference>(
            &session,
            "/api/v1/timer/preferences",
            "PUT",
            Some(body),
        )?;
        preference.validate()?;
        return Ok(preference);
    }

    let config = require_online_config(&app)?;
    let access_token = online_session.ensure_access_token(&config)?;
    let preference = online_session::fetch_remote_business::<TimerPreference>(
        &config,
        &access_token,
        "/api/v1/timer/preferences",
        "PUT",
        Some(&body),
    )?;
    preference.validate()?;
    Ok(preference)
}

#[tauri::command]
pub fn hdx_user_preferences_get(
    app: AppHandle,
    online_session: tauri::State<'_, OnlineSessionHolder>,
    backend_sidecar: tauri::State<'_, BackendSidecar>,
) -> Result<UserPreference, String> {
    if flavor::active_flavor().includes_full_backend() {
        let session = require_local_backend_session(&backend_sidecar)?;
        let preference =
            fetch_local_json::<UserPreference>(&session, "/api/v1/user/preferences", "GET", None)?;
        preference.validate()?;
        return Ok(preference);
    }

    let config = require_online_config(&app)?;
    let access_token = online_session.ensure_access_token(&config)?;
    let preference = online_session::fetch_remote_business::<UserPreference>(
        &config,
        &access_token,
        "/api/v1/user/preferences",
        "GET",
        None,
    )?;
    preference.validate()?;
    Ok(preference)
}

#[tauri::command]
pub fn hdx_user_preferences_save(
    app: AppHandle,
    online_session: tauri::State<'_, OnlineSessionHolder>,
    backend_sidecar: tauri::State<'_, BackendSidecar>,
    input: UserPreferenceSaveRequest,
) -> Result<UserPreference, String> {
    let body = input.to_backend_body()?;

    if flavor::active_flavor().includes_full_backend() {
        let session = require_local_backend_session(&backend_sidecar)?;
        let preference = fetch_local_json::<UserPreference>(
            &session,
            "/api/v1/user/preferences",
            "PUT",
            Some(body),
        )?;
        preference.validate()?;
        return Ok(preference);
    }

    let config = require_online_config(&app)?;
    let access_token = online_session.ensure_access_token(&config)?;
    let preference = online_session::fetch_remote_business::<UserPreference>(
        &config,
        &access_token,
        "/api/v1/user/preferences",
        "PUT",
        Some(&body),
    )?;
    preference.validate()?;
    Ok(preference)
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

fn require_local_backend_session(
    sidecar: &BackendSidecar,
) -> Result<crate::sidecar::LocalBackendSession, String> {
    sidecar
        .local_backend_session()
        .ok_or_else(|| "本机后端尚未就绪，请稍后重试。".to_string())
}

fn require_online_config(app: &AppHandle) -> Result<crate::online_config::OnlineConfig, String> {
    online_config::read_app_config(app)?
        .ok_or_else(|| "请先配置 Desktop Online 远端服务地址。".to_string())
}

fn validate_holiday_id(id: u64) -> Result<(), String> {
    if id == 0 {
        return Err("节日 ID 无效。".to_string());
    }

    Ok(())
}

fn validate_holiday_version(version: u32) -> Result<(), String> {
    if version == 0 {
        return Err("节日版本无效。".to_string());
    }

    Ok(())
}

fn online_remote_session(online_session: &OnlineSessionHolder) -> WebAuthPublicSession {
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
        assert!(local_backend::parse_local_backend_port("https://example.com").is_err());
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
