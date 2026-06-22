use std::collections::HashSet;

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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchLayout {
    version: u8,
    rows: u8,
    columns: u8,
    gap: u8,
    widgets: Vec<WorkbenchLayoutWidget>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchLayoutWidget {
    id: String,
    key: String,
    order: u8,
    column: u8,
    row: u8,
    col_span: u8,
    row_span: u8,
    chrome: String,
    orientation: String,
    header: WorkbenchLayoutHeader,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkbenchLayoutHeader {
    visible: bool,
    icon: bool,
    title: bool,
    description: bool,
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

impl WorkbenchLayout {
    pub(super) fn validate(&self) -> Result<(), String> {
        if self.version != 1 {
            return Err("工作台布局版本不支持。".to_string());
        }

        validate_u8_range("workbench.rows", self.rows, 1, 8)?;
        validate_u8_range("workbench.columns", self.columns, 1, 8)?;
        validate_u8_range("workbench.gap", self.gap, 2, 24)?;

        if self.widgets.len() > 24 {
            return Err("工作台组件数量不能超过 24 个。".to_string());
        }

        let mut widget_ids = HashSet::new();
        let mut occupied = vec![vec![false; self.columns as usize]; self.rows as usize];

        for widget in &self.widgets {
            widget.validate(self.rows, self.columns)?;

            if !widget_ids.insert(widget.id.as_str()) {
                return Err("工作台布局包含重复组件。".to_string());
            }

            for row in widget.row..widget.row + widget.row_span {
                for column in widget.column..widget.column + widget.col_span {
                    let cell = &mut occupied[row as usize][column as usize];

                    if *cell {
                        return Err("工作台组件位置发生重叠。".to_string());
                    }

                    *cell = true;
                }
            }
        }

        Ok(())
    }

    pub(super) fn to_backend_body(&self) -> Result<Value, String> {
        self.validate()?;
        serde_json::to_value(self).map_err(|error| format!("序列化工作台布局失败：{error}"))
    }
}

impl WorkbenchLayoutWidget {
    fn validate(&self, rows: u8, columns: u8) -> Result<(), String> {
        validate_trimmed_text("workbench.widget.id", &self.id, 1, 120)?;
        validate_workbench_key("workbench.widget.key", &self.key)?;
        validate_u8_range("workbench.widget.order", self.order, 0, 23)?;
        validate_u8_range("workbench.widget.column", self.column, 0, 7)?;
        validate_u8_range("workbench.widget.row", self.row, 0, 7)?;
        validate_u8_range("workbench.widget.colSpan", self.col_span, 1, 8)?;
        validate_u8_range("workbench.widget.rowSpan", self.row_span, 1, 8)?;

        if !matches!(self.chrome.as_str(), "card" | "bare") {
            return Err("workbench.widget.chrome 只能是 card 或 bare。".to_string());
        }

        if !matches!(
            self.orientation.as_str(),
            "auto" | "horizontal" | "vertical"
        ) {
            return Err(
                "workbench.widget.orientation 只能是 auto、horizontal 或 vertical。".to_string(),
            );
        }

        if self.column + self.col_span > columns || self.row + self.row_span > rows {
            return Err("工作台组件超出布局范围。".to_string());
        }

        Ok(())
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

fn validate_u8_range(name: &str, value: u8, min: u8, max: u8) -> Result<(), String> {
    if value < min || value > max {
        return Err(format!("{name} 必须在 {min} 到 {max} 之间。"));
    }

    Ok(())
}

fn validate_workbench_key(name: &str, value: &str) -> Result<(), String> {
    validate_text(name, value, 1, 80)?;

    let mut chars = value.chars();
    let first = chars.next().ok_or_else(|| format!("{name} 格式无效。"))?;

    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return Err(format!("{name} 格式无效。"));
    }

    if chars.all(|character| {
        character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
    }) {
        return Ok(());
    }

    Err(format!("{name} 格式无效。"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workbench_layout_rejects_overlapping_widgets() {
        let layout = WorkbenchLayout {
            version: 1,
            rows: 2,
            columns: 2,
            gap: 12,
            widgets: vec![
                widget("first", "quick-links", 0, 0, 0, 1, 1),
                widget("second", "runtime", 1, 0, 0, 1, 1),
            ],
        };

        assert_eq!(layout.validate().unwrap_err(), "工作台组件位置发生重叠。");
    }

    #[test]
    fn workbench_layout_serializes_valid_backend_body() {
        let layout = WorkbenchLayout {
            version: 1,
            rows: 2,
            columns: 2,
            gap: 12,
            widgets: vec![widget("first", "quick-links", 0, 0, 0, 1, 1)],
        };
        let body = layout.to_backend_body().unwrap();

        assert_eq!(body["version"], 1);
        assert_eq!(body["widgets"][0]["id"], "first");
        assert_eq!(body["widgets"][0]["header"]["title"], true);
    }

    fn widget(
        id: &str,
        key: &str,
        order: u8,
        column: u8,
        row: u8,
        col_span: u8,
        row_span: u8,
    ) -> WorkbenchLayoutWidget {
        WorkbenchLayoutWidget {
            id: id.to_string(),
            key: key.to_string(),
            order,
            column,
            row,
            col_span,
            row_span,
            chrome: "card".to_string(),
            orientation: "auto".to_string(),
            header: WorkbenchLayoutHeader {
                visible: true,
                icon: true,
                title: true,
                description: true,
            },
        }
    }
}
