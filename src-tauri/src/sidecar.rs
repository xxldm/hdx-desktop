use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File},
    path::{Component, Path, PathBuf},
    process::{Child, Command, Stdio},
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

use crate::local_http::{http_get, reserve_local_port, LOCAL_HOST};

const HEALTH_PATH: &str = "/actuator/health";
const LOCAL_SESSION_PATH: &str = "/local/session";
const STARTUP_TIMEOUT: Duration = Duration::from_secs(30);
const POLL_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackendSidecarState {
    NotApplicable,
    MissingResource,
    Starting,
    Running,
    Failed,
    Stopped,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendSidecarStatus {
    pub state: BackendSidecarState,
    pub base_url: Option<String>,
    pub health_check_url: Option<String>,
    pub local_session_ready: bool,
    pub local_token_exposed_to_webview: bool,
    pub executable_path: Option<String>,
    pub data_dir: Option<String>,
    pub message: Option<String>,
}

impl BackendSidecarStatus {
    pub fn not_applicable() -> Self {
        Self {
            state: BackendSidecarState::NotApplicable,
            base_url: None,
            health_check_url: None,
            local_session_ready: false,
            local_token_exposed_to_webview: false,
            executable_path: None,
            data_dir: None,
            message: Some("Online flavor 不启动本机后端。".to_string()),
        }
    }
}

#[derive(Clone, Default)]
pub struct BackendSidecar {
    inner: Arc<Mutex<BackendSidecarInner>>,
}

#[derive(Default)]
struct BackendSidecarInner {
    status: Option<BackendSidecarStatus>,
    child: Option<Child>,
    local_session: Option<LocalSession>,
}

struct LocalSession {
    header_name: String,
    token: String,
}

#[derive(Clone)]
pub struct LocalBackendSession {
    pub base_url: String,
    pub header_name: String,
    pub token: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackendBuild {
    schema_version: String,
    manifest_kind: String,
    version: String,
    backend: BackendBuildBackend,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct BackendBuildBackend {
    executable_path: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LocalSessionResponse {
    header_name: String,
    token: String,
}

struct PreparedBackend {
    version: String,
    executable_path: PathBuf,
    install_dir: PathBuf,
    data_dir: PathBuf,
}

impl BackendSidecar {
    pub fn start_in_background(&self, resource_backend_dir: PathBuf, app_data_dir: PathBuf) {
        self.set_status(BackendSidecarStatus {
            state: BackendSidecarState::Starting,
            base_url: None,
            health_check_url: None,
            local_session_ready: false,
            local_token_exposed_to_webview: false,
            executable_path: None,
            data_dir: None,
            message: Some("正在准备本机后端。".to_string()),
        });

        let manager = self.clone();
        thread::spawn(move || {
            if let Err(error) = manager.start_blocking(&resource_backend_dir, &app_data_dir) {
                manager.stop_child();
                let state = if error.contains("缺少 Desktop Full 后端资源") {
                    BackendSidecarState::MissingResource
                } else {
                    BackendSidecarState::Failed
                };
                manager.set_status(BackendSidecarStatus {
                    state,
                    base_url: None,
                    health_check_url: None,
                    local_session_ready: false,
                    local_token_exposed_to_webview: false,
                    executable_path: None,
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
            Some(BackendSidecarState::NotApplicable | BackendSidecarState::MissingResource)
        ) {
            inner.status = Some(BackendSidecarStatus {
                state: BackendSidecarState::Stopped,
                base_url: None,
                health_check_url: None,
                local_session_ready: false,
                local_token_exposed_to_webview: false,
                executable_path: None,
                data_dir: None,
                message: Some("本机后端已随 Desktop 退出清理。".to_string()),
            });
        }
    }

    pub fn snapshot(&self) -> BackendSidecarStatus {
        let mut inner = self.lock_inner();
        let local_session_ready = inner
            .local_session
            .as_ref()
            .map(|session| !session.header_name.is_empty() && !session.token.is_empty())
            .unwrap_or(false);
        if let Some(status) = inner.status.as_mut() {
            if matches!(status.state, BackendSidecarState::Running) {
                status.local_session_ready = local_session_ready;
            }
        }

        let exited = match inner.child.as_mut() {
            Some(child) => match child.try_wait() {
                Ok(Some(status)) => Some(format!("本机后端进程已退出：{status}")),
                Ok(None) => None,
                Err(error) => Some(format!("检查本机后端进程状态失败：{error}")),
            },
            None => None,
        };

        if let Some(message) = exited {
            inner.child = None;
            inner.local_session = None;
            inner.status = Some(BackendSidecarStatus {
                state: BackendSidecarState::Failed,
                base_url: None,
                health_check_url: None,
                local_session_ready: false,
                local_token_exposed_to_webview: false,
                executable_path: None,
                data_dir: None,
                message: Some(message),
            });
        }

        inner
            .status
            .clone()
            .unwrap_or_else(BackendSidecarStatus::not_applicable)
    }

    pub fn local_backend_session(&self) -> Option<LocalBackendSession> {
        let inner = self.lock_inner();
        let status = inner.status.as_ref()?;

        if !matches!(status.state, BackendSidecarState::Running) {
            return None;
        }

        let session = inner.local_session.as_ref()?;
        Some(LocalBackendSession {
            base_url: status.base_url.clone()?,
            header_name: session.header_name.clone(),
            token: session.token.clone(),
        })
    }

    fn start_blocking(
        &self,
        resource_backend_dir: &Path,
        app_data_dir: &Path,
    ) -> Result<(), String> {
        let prepared = prepare_backend_runtime(resource_backend_dir, app_data_dir)?;
        let port = reserve_local_port()?;
        let base_url = format!("http://{LOCAL_HOST}:{port}");
        let health_check_url = format!("{base_url}{HEALTH_PATH}");
        let stdout_log = create_log_file(&prepared.data_dir, "backend-full.stdout.log")?;
        let stderr_log = create_log_file(&prepared.data_dir, "backend-full.stderr.log")?;
        let jdbc_path = prepared
            .data_dir
            .join("hdx-all-in-one")
            .to_string_lossy()
            .replace('\\', "/");

        let child = Command::new(&prepared.executable_path)
            .current_dir(&prepared.install_dir)
            .env("HDX_ALL_IN_ONE_PORT", port.to_string())
            .env(
                "HDX_LOCAL_JDBC_URL",
                format!("jdbc:h2:file:{jdbc_path};MODE=PostgreSQL;AUTO_SERVER=TRUE"),
            )
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout_log))
            .stderr(Stdio::from(stderr_log))
            .spawn()
            .map_err(|error| format!("启动本机后端失败：{error}"))?;

        {
            let mut inner = self.lock_inner();
            inner.child = Some(child);
            inner.status = Some(BackendSidecarStatus {
                state: BackendSidecarState::Starting,
                base_url: Some(base_url.clone()),
                health_check_url: Some(health_check_url.clone()),
                local_session_ready: false,
                local_token_exposed_to_webview: false,
                executable_path: Some(display_path(&prepared.executable_path)),
                data_dir: Some(display_path(&prepared.data_dir)),
                message: Some(format!("本机后端 {} 正在启动。", prepared.version)),
            });
        }

        wait_for_health(port)?;
        let session = fetch_local_session(port)?;

        let mut inner = self.lock_inner();
        inner.local_session = Some(LocalSession {
            header_name: session.header_name,
            token: session.token,
        });
        inner.status = Some(BackendSidecarStatus {
            state: BackendSidecarState::Running,
            base_url: Some(base_url),
            health_check_url: Some(health_check_url),
            local_session_ready: true,
            local_token_exposed_to_webview: false,
            executable_path: Some(display_path(&prepared.executable_path)),
            data_dir: Some(display_path(&prepared.data_dir)),
            message: Some("本机后端已启动，本机 token 保留在 Rust 主进程边界内。".to_string()),
        });
        Ok(())
    }

    fn stop_child(&self) {
        let child = {
            let mut inner = self.lock_inner();
            inner.local_session = None;
            inner.child.take()
        };

        if let Some(mut child) = child {
            if child.try_wait().ok().flatten().is_none() {
                let _ = child.kill();
            }
            let _ = child.wait();
        }
    }

    fn set_status(&self, status: BackendSidecarStatus) {
        let mut inner = self.lock_inner();
        inner.status = Some(status);
    }

    fn lock_inner(&self) -> std::sync::MutexGuard<'_, BackendSidecarInner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

fn prepare_backend_runtime(
    resource_backend_dir: &Path,
    app_data_dir: &Path,
) -> Result<PreparedBackend, String> {
    if !resource_backend_dir.is_dir() {
        return Err(format!(
            "缺少 Desktop Full 后端资源目录：{}",
            display_path(resource_backend_dir)
        ));
    }

    let backend_build_path = resource_backend_dir.join("backend-build.json");
    let backend_build = read_backend_build(&backend_build_path)?;
    let executable_relative = safe_relative_path(&backend_build.backend.executable_path)?;
    let resource_executable = resource_backend_dir.join(&executable_relative);
    if !resource_executable.is_file() {
        return Err(format!(
            "缺少 Desktop Full 后端可执行文件：{}",
            display_path(&resource_executable)
        ));
    }

    let install_dir = app_data_dir
        .join("backend")
        .join("runtime")
        .join(sanitize_path_segment(&backend_build.version));
    let data_dir = app_data_dir.join("backend").join("data");
    fs::create_dir_all(&install_dir)
        .map_err(|error| format!("创建本机后端运行目录失败：{error}"))?;
    fs::create_dir_all(&data_dir).map_err(|error| format!("创建本机后端数据目录失败：{error}"))?;

    copy_dir_contents(&resource_backend_dir, &install_dir)?;
    let executable_path = install_dir.join(executable_relative);
    if !executable_path.is_file() {
        return Err(format!(
            "复制后的本机后端可执行文件不存在：{}",
            display_path(&executable_path)
        ));
    }
    ensure_executable_permission(&executable_path)?;

    Ok(PreparedBackend {
        version: backend_build.version,
        executable_path,
        install_dir,
        data_dir,
    })
}

#[cfg(unix)]
fn ensure_executable_permission(path: &Path) -> Result<(), String> {
    use std::os::unix::fs::PermissionsExt;

    let metadata =
        fs::metadata(path).map_err(|error| format!("读取可执行文件权限失败：{error}"))?;
    let mut permissions = metadata.permissions();
    permissions.set_mode(permissions.mode() | 0o700);
    fs::set_permissions(path, permissions)
        .map_err(|error| format!("设置可执行文件权限失败：{error}"))
}

#[cfg(not(unix))]
fn ensure_executable_permission(_path: &Path) -> Result<(), String> {
    Ok(())
}

fn read_backend_build(path: &Path) -> Result<BackendBuild, String> {
    let text = fs::read_to_string(path)
        .map_err(|error| format!("读取 backend-build.json 失败：{error}"))?;
    let build: BackendBuild = serde_json::from_str(&text)
        .map_err(|error| format!("解析 backend-build.json 失败：{error}"))?;

    if build.schema_version != "1.0" || build.manifest_kind != "backend-build" {
        return Err("backend-build.json 类型无效。".to_string());
    }

    Ok(build)
}

fn safe_relative_path(raw: &str) -> Result<PathBuf, String> {
    let path = Path::new(raw);
    if path.is_absolute() {
        return Err(format!("后端 entrypoint 不能是绝对路径：{raw}"));
    }

    let mut output = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Normal(value) => output.push(value),
            Component::CurDir => {}
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(format!("后端 entrypoint 不能跳出资源目录：{raw}"));
            }
        }
    }

    if output.as_os_str().is_empty() {
        return Err("后端 entrypoint 不能为空。".to_string());
    }

    Ok(output)
}

fn copy_dir_contents(source: &Path, destination: &Path) -> Result<(), String> {
    for entry in fs::read_dir(source).map_err(|error| format!("读取资源目录失败：{error}"))?
    {
        let entry = entry.map_err(|error| format!("读取资源目录项失败：{error}"))?;
        let file_type = entry
            .file_type()
            .map_err(|error| format!("读取资源文件类型失败：{error}"))?;
        let target = destination.join(entry.file_name());

        if file_type.is_symlink() {
            return Err(format!(
                "本机后端资源不允许包含符号链接：{}",
                display_path(&entry.path())
            ));
        }

        if file_type.is_dir() {
            fs::create_dir_all(&target).map_err(|error| format!("创建资源子目录失败：{error}"))?;
            copy_dir_contents(&entry.path(), &target)?;
        } else if file_type.is_file() {
            fs::copy(entry.path(), &target)
                .map_err(|error| format!("复制资源文件失败：{error}"))?;
        }
    }

    Ok(())
}

fn create_log_file(data_dir: &Path, name: &str) -> Result<File, String> {
    let log_dir = data_dir
        .parent()
        .ok_or_else(|| "无法定位本机后端日志目录。".to_string())?
        .join("logs");
    fs::create_dir_all(&log_dir).map_err(|error| format!("创建本机后端日志目录失败：{error}"))?;
    File::create(log_dir.join(name)).map_err(|error| format!("创建本机后端日志文件失败：{error}"))
}

fn wait_for_health(port: u16) -> Result<(), String> {
    let deadline = Instant::now() + STARTUP_TIMEOUT;
    let mut last_error = String::new();

    while Instant::now() < deadline {
        match http_get(port, HEALTH_PATH) {
            Ok(response) if response.status_code == 200 && response.body.contains("\"UP\"") => {
                return Ok(());
            }
            Ok(response) => {
                last_error = format!("健康检查返回 HTTP {}。", response.status_code);
            }
            Err(error) => {
                last_error = error;
            }
        }

        thread::sleep(POLL_INTERVAL);
    }

    Err(format!("本机后端健康检查超时：{last_error}"))
}

fn fetch_local_session(port: u16) -> Result<LocalSessionResponse, String> {
    let response = http_get(port, LOCAL_SESSION_PATH)?;
    if response.status_code != 200 {
        return Err(format!(
            "读取本机会话失败，HTTP 状态：{}",
            response.status_code
        ));
    }

    let session: LocalSessionResponse = serde_json::from_str(&response.body)
        .map_err(|error| format!("解析本机会话失败：{error}"))?;
    if session.header_name.trim().is_empty() || session.token.trim().len() < 32 {
        return Err("本机会话返回的 token 信息无效。".to_string());
    }
    Ok(session)
}

fn sanitize_path_segment(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn display_path(path: &Path) -> String {
    path.display().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_relative_path_rejects_parent_segments() {
        assert!(safe_relative_path("../bin/hdx-backend-full.exe").is_err());
    }

    #[test]
    fn safe_relative_path_accepts_backend_entrypoint() {
        assert_eq!(
            safe_relative_path("bin/hdx-backend-full.exe").unwrap(),
            PathBuf::from("bin").join("hdx-backend-full.exe")
        );
    }

    #[test]
    fn sidecar_status_does_not_serialize_local_token() {
        let status = BackendSidecarStatus {
            state: BackendSidecarState::Running,
            base_url: Some("http://127.0.0.1:18082".to_string()),
            health_check_url: Some("http://127.0.0.1:18082/actuator/health".to_string()),
            local_session_ready: true,
            local_token_exposed_to_webview: false,
            executable_path: None,
            data_dir: None,
            message: None,
        };

        let json = serde_json::to_string(&status).unwrap();

        assert!(!json.contains("token"));
        assert!(json.contains("localSessionReady"));
    }
}
