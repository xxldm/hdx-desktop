use serde::Deserialize;
use std::{
    fs,
    path::{Component, Path, PathBuf},
};

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

pub(super) struct PreparedBackend {
    pub(super) version: String,
    pub(super) executable_path: PathBuf,
    pub(super) install_dir: PathBuf,
    pub(super) data_dir: PathBuf,
}

pub(super) fn prepare_backend_runtime(
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

    copy_dir_contents(resource_backend_dir, &install_dir)?;
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

pub(super) fn safe_relative_path(raw: &str) -> Result<PathBuf, String> {
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

pub(super) fn display_path(path: &Path) -> String {
    path.display().to_string()
}
