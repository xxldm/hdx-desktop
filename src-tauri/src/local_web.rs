use serde::Serialize;
use std::{
    fs::{self, File},
    path::{Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use crate::{
    local_http::{http_get, reserve_local_port, LOCAL_HOST},
    sidecar::{BackendSidecar, BackendSidecarState, LocalBackendSession},
};

const WEB_SESSION_PATH: &str = "/api/hdx/v1/auth/session";
const BACKEND_WAIT_TIMEOUT: Duration = Duration::from_secs(45);
const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum LocalWebServerState {
    NotApplicable,
    MissingResource,
    WaitingForBackend,
    Starting,
    Running,
    Failed,
    Stopped,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalWebServerStatus {
    pub state: LocalWebServerState,
    pub base_url: Option<String>,
    pub session_probe_url: Option<String>,
    pub local_backend_ready: bool,
    pub local_token_exposed_to_webview: bool,
    pub web_root: Option<String>,
    pub data_dir: Option<String>,
    pub message: Option<String>,
}

impl LocalWebServerStatus {
    pub fn not_applicable() -> Self {
        Self {
            state: LocalWebServerState::NotApplicable,
            base_url: None,
            session_probe_url: None,
            local_backend_ready: false,
            local_token_exposed_to_webview: false,
            web_root: None,
            data_dir: None,
            message: Some("Online flavor 不启动本机 Web/Nuxt server。".to_string()),
        }
    }
}

#[derive(Clone, Default)]
pub struct LocalWebServer {
    inner: Arc<Mutex<LocalWebServerInner>>,
}

#[derive(Default)]
struct LocalWebServerInner {
    status: Option<LocalWebServerStatus>,
    child: Option<Child>,
}

struct PreparedWebServer {
    root_dir: PathBuf,
    data_dir: PathBuf,
}

impl LocalWebServer {
    pub fn start_after_backend(
        &self,
        backend_sidecar: BackendSidecar,
        resource_web_dir: PathBuf,
        app_data_dir: PathBuf,
    ) {
        if !resource_web_dir.is_dir() {
            self.set_status(LocalWebServerStatus {
                state: LocalWebServerState::MissingResource,
                base_url: None,
                session_probe_url: None,
                local_backend_ready: false,
                local_token_exposed_to_webview: false,
                web_root: Some(display_path(&resource_web_dir)),
                data_dir: None,
                message: Some("缺少 Desktop Full Web/Nuxt server 资源目录。".to_string()),
            });
            return;
        }

        self.set_status(LocalWebServerStatus {
            state: LocalWebServerState::WaitingForBackend,
            base_url: None,
            session_probe_url: None,
            local_backend_ready: false,
            local_token_exposed_to_webview: false,
            web_root: Some(display_path(&resource_web_dir)),
            data_dir: None,
            message: Some("正在等待本机后端会话。".to_string()),
        });

        let manager = self.clone();
        thread::spawn(move || {
            let session = match wait_for_backend_session(&backend_sidecar) {
                Ok(session) => session,
                Err(error) => {
                    manager.set_status(LocalWebServerStatus {
                        state: LocalWebServerState::Failed,
                        base_url: None,
                        session_probe_url: None,
                        local_backend_ready: false,
                        local_token_exposed_to_webview: false,
                        web_root: Some(display_path(&resource_web_dir)),
                        data_dir: None,
                        message: Some(error),
                    });
                    return;
                }
            };

            if let Err(error) = manager.start_blocking(&resource_web_dir, &app_data_dir, session) {
                manager.stop_child();
                let state = if error.contains("缺少 Desktop Full Web") {
                    LocalWebServerState::MissingResource
                } else {
                    LocalWebServerState::Failed
                };
                manager.set_status(LocalWebServerStatus {
                    state,
                    base_url: None,
                    session_probe_url: None,
                    local_backend_ready: false,
                    local_token_exposed_to_webview: false,
                    web_root: Some(display_path(&resource_web_dir)),
                    data_dir: None,
                    message: Some(error),
                });
            }
        });
    }

    pub fn stop(&self) {
        self.stop_child();
        let mut inner = self.lock_inner();
        if !matches!(
            inner.status.as_ref().map(|status| &status.state),
            Some(LocalWebServerState::NotApplicable | LocalWebServerState::MissingResource)
        ) {
            inner.status = Some(LocalWebServerStatus {
                state: LocalWebServerState::Stopped,
                base_url: None,
                session_probe_url: None,
                local_backend_ready: false,
                local_token_exposed_to_webview: false,
                web_root: None,
                data_dir: None,
                message: Some("本机 Web/Nuxt server 已随 Desktop 退出清理。".to_string()),
            });
        }
    }

    pub fn snapshot(&self) -> LocalWebServerStatus {
        let mut inner = self.lock_inner();
        let exited = match inner.child.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(Some(status)) => Some(format!("本机 Web/Nuxt server 已退出：{status}")),
                Ok(None) => None,
                Err(error) => Some(format!("检查本机 Web/Nuxt server 状态失败：{error}")),
            },
            None => None,
        };

        if let Some(message) = exited {
            inner.child = None;
            inner.status = Some(LocalWebServerStatus {
                state: LocalWebServerState::Failed,
                base_url: None,
                session_probe_url: None,
                local_backend_ready: false,
                local_token_exposed_to_webview: false,
                web_root: None,
                data_dir: None,
                message: Some(message),
            });
        }

        inner
            .status
            .clone()
            .unwrap_or_else(LocalWebServerStatus::not_applicable)
    }

    fn start_blocking(
        &self,
        resource_web_dir: &Path,
        app_data_dir: &Path,
        session: LocalBackendSession,
    ) -> Result<(), String> {
        let prepared = prepare_web_server(resource_web_dir, app_data_dir)?;
        let port = reserve_local_port()?;
        let base_url = format!("http://{LOCAL_HOST}:{port}");
        let session_probe_url = format!("{base_url}{WEB_SESSION_PATH}");
        let stdout_log = create_log_file(&prepared.data_dir, "web-node-server.stdout.log")?;
        let stderr_log = create_log_file(&prepared.data_dir, "web-node-server.stderr.log")?;

        let mut command = Command::new("node");
        command
            .current_dir(&prepared.root_dir)
            .arg(prepared.root_dir.join("start-web.mjs"))
            .env("NODE_ENV", "production")
            .env("NITRO_HOST", LOCAL_HOST)
            .env("NITRO_PORT", port.to_string())
            .env("NUXT_BACKEND_BASE_URL", &session.base_url)
            .env("NUXT_AUTH_BASE_URL", &session.base_url)
            .env("NUXT_BACKEND_LOCAL_TOKEN_HEADER", &session.header_name)
            .env("NUXT_BACKEND_LOCAL_TOKEN", &session.token)
            .env("NUXT_AUTH_COOKIE_SECURE", "false")
            .env("NUXT_AUTH_SESSION_COOKIE_NAME", "hdx_desktop_full_session")
            .env("NUXT_AUTH_CSRF_COOKIE_NAME", "hdx_desktop_full_csrf")
            .env(
                "NUXT_AUTH_SESSION_SECRET",
                format!("desktop-full-session-{}", session.token),
            )
            .env("HDX_WEB_CONFIG_FILE", "")
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout_log))
            .stderr(Stdio::from(stderr_log));

        let child = command
            .spawn()
            .map_err(|error| format!("启动本机 Web/Nuxt server 失败：{error}"))?;

        {
            let mut inner = self.lock_inner();
            inner.child = Some(child);
            inner.status = Some(LocalWebServerStatus {
                state: LocalWebServerState::Starting,
                base_url: Some(base_url.clone()),
                session_probe_url: Some(session_probe_url.clone()),
                local_backend_ready: true,
                local_token_exposed_to_webview: false,
                web_root: Some(display_path(&prepared.root_dir)),
                data_dir: Some(display_path(&prepared.data_dir)),
                message: Some("本机 Web/Nuxt server 正在启动。".to_string()),
            });
        }

        wait_for_web_session(port)?;

        let mut inner = self.lock_inner();
        inner.status = Some(LocalWebServerStatus {
            state: LocalWebServerState::Running,
            base_url: Some(base_url),
            session_probe_url: Some(session_probe_url),
            local_backend_ready: true,
            local_token_exposed_to_webview: false,
            web_root: Some(display_path(&prepared.root_dir)),
            data_dir: Some(display_path(&prepared.data_dir)),
            message: Some(
                "本机 Web/Nuxt server 已启动，本机 token 仅注入 server 环境。".to_string(),
            ),
        });
        Ok(())
    }

    fn stop_child(&self) {
        let child = {
            let mut inner = self.lock_inner();
            inner.child.take()
        };

        if let Some(mut child) = child {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
            }
            let _ = child.wait();
        }
    }

    fn set_status(&self, status: LocalWebServerStatus) {
        let mut inner = self.lock_inner();
        inner.status = Some(status);
    }

    fn lock_inner(&self) -> std::sync::MutexGuard<'_, LocalWebServerInner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn wait_for_backend_session(
    backend_sidecar: &BackendSidecar,
) -> Result<LocalBackendSession, String> {
    let deadline = Instant::now() + BACKEND_WAIT_TIMEOUT;

    while Instant::now() < deadline {
        if let Some(session) = backend_sidecar.local_backend_session() {
            return Ok(session);
        }

        let backend_status = backend_sidecar.snapshot();
        if matches!(
            backend_status.state,
            BackendSidecarState::MissingResource
                | BackendSidecarState::Failed
                | BackendSidecarState::Stopped
        ) {
            return Err(format!(
                "本机后端未就绪，无法启动 Web/Nuxt server：{}",
                backend_status
                    .message
                    .unwrap_or_else(|| "未知状态".to_string())
            ));
        }

        thread::sleep(POLL_INTERVAL);
    }

    Err("等待本机后端会话超时，无法启动 Web/Nuxt server。".to_string())
}

fn prepare_web_server(
    resource_web_dir: &Path,
    app_data_dir: &Path,
) -> Result<PreparedWebServer, String> {
    if !resource_web_dir.is_dir() {
        return Err(format!(
            "缺少 Desktop Full Web/Nuxt server 资源目录：{}",
            display_path(resource_web_dir)
        ));
    }

    for relative_path in [
        "start-web.mjs",
        "server/index.mjs",
        "scripts/web-config-loader.mjs",
        "node_modules/yaml/package.json",
    ] {
        let target = resource_web_dir.join(relative_path);
        if !target.is_file() {
            return Err(format!(
                "缺少 Desktop Full Web/Nuxt server 资源文件：{}",
                display_path(&target)
            ));
        }
    }

    let data_dir = app_data_dir.join("web");
    fs::create_dir_all(&data_dir).map_err(|error| format!("创建本机 Web 数据目录失败：{error}"))?;

    Ok(PreparedWebServer {
        root_dir: resource_web_dir.to_path_buf(),
        data_dir,
    })
}

fn create_log_file(data_dir: &Path, name: &str) -> Result<File, String> {
    let log_dir = data_dir.join("logs");
    fs::create_dir_all(&log_dir).map_err(|error| format!("创建本机 Web 日志目录失败：{error}"))?;
    File::create(log_dir.join(name)).map_err(|error| format!("创建本机 Web 日志文件失败：{error}"))
}

fn wait_for_web_session(port: u16) -> Result<(), String> {
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    let mut last_error = String::new();

    while Instant::now() < deadline {
        match http_get(port, WEB_SESSION_PATH) {
            Ok(response)
                if response.status_code == 200
                    && response.body.contains("\"authenticated\":true")
                    && response.body.contains("\"actorType\":\"LOCAL_ADMIN\"") =>
            {
                return Ok(());
            }
            Ok(response) => {
                last_error = format!("本机 Web 会话探测返回 HTTP {}。", response.status_code);
            }
            Err(error) => {
                last_error = error;
            }
        }

        thread::sleep(POLL_INTERVAL);
    }

    Err(format!("本机 Web/Nuxt server 会话探测超时：{last_error}"))
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn web_status_does_not_serialize_local_token() {
        let status = LocalWebServerStatus {
            state: LocalWebServerState::Running,
            base_url: Some("http://127.0.0.1:3000".to_string()),
            session_probe_url: Some("http://127.0.0.1:3000/api/hdx/v1/auth/session".to_string()),
            local_backend_ready: true,
            local_token_exposed_to_webview: false,
            web_root: None,
            data_dir: None,
            message: Some("本机 Web/Nuxt server 已启动。".to_string()),
        };

        let json = serde_json::to_string(&status).unwrap();

        assert!(!json.contains("secret-local-token"));
        assert!(!json.contains("NUXT_BACKEND_LOCAL_TOKEN"));
        assert!(json.contains("localTokenExposedToWebview"));
    }

    #[test]
    fn prepare_web_server_rejects_missing_layout() {
        let missing = PathBuf::from("target/desktop-web-missing-fixture");
        let result = prepare_web_server(&missing, Path::new("target"));

        assert!(result.is_err());
    }
}
