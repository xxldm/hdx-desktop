use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackendAuthUser {
    pub(super) id: u64,
    pub(super) display_name: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebAuthPublicSession {
    pub(super) authenticated: bool,
    pub(super) csrf_token: String,
    pub(super) access_token_expires_at: Option<String>,
    pub(super) refresh_token_expires_at: Option<String>,
    pub(super) sid: Option<String>,
    pub(super) actor_type: Option<String>,
    pub(super) subject: Option<String>,
    pub(super) user: Option<BackendAuthUser>,
    pub(super) roles: Vec<String>,
    pub(super) permissions: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WebAuthLoginRequest {
    pub(super) identifier: String,
    pub(super) password: String,
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
    pub(super) tool_key: String,
    pub(super) display_name: String,
    #[serde(default)]
    pub(super) description: Option<String>,
}

impl WebAuthLoginRequest {
    pub(super) fn validate(&self) -> Result<(), String> {
        validate_trimmed_text("登录账号", &self.identifier, 1, 320)?;
        validate_text("登录密码", &self.password, 1, 200)
    }
}

impl RuntimeInfo {
    pub(super) fn validate(&self) -> Result<(), String> {
        validate_trimmed_text("runtime.application", &self.application, 1, 200)?;
        validate_trimmed_text("runtime.topology", &self.topology, 1, 200)?;
        validate_trimmed_text("runtime.javaVersion", &self.java_version, 1, 80)?;
        Ok(())
    }
}

impl ToolRecord {
    pub(super) fn validate(&self) -> Result<(), String> {
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
    pub(super) fn to_backend_body(&self) -> Result<Value, String> {
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
