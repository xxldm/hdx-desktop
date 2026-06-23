use std::collections::HashSet;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const USER_PREFERENCE_PRIMARY_COLORS: &[&str] = &[
    "black", "red", "orange", "amber", "yellow", "lime", "green", "emerald", "teal", "cyan", "sky",
    "blue", "indigo", "violet", "purple", "fuchsia", "pink", "rose",
];
const USER_PREFERENCE_NEUTRAL_COLORS: &[&str] = &[
    "slate", "gray", "zinc", "neutral", "stone", "taupe", "mauve", "mist", "olive",
];
const USER_PREFERENCE_RADII: &[&str] = &["0", "0.125", "0.25", "0.375", "0.5"];

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
    schema_version: u8,
    version: u32,
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

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimerPreference {
    schema_version: u8,
    version: u32,
    presets: Vec<TimerPreferencePreset>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimerPreferencePreset {
    id: String,
    order: u8,
    duration_seconds: u32,
    created_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimerPreferenceSaveRequest {
    schema_version: u8,
    version: u32,
    presets: Vec<TimerPreferencePresetRequest>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TimerPreferencePresetRequest {
    id: String,
    order: u8,
    duration_seconds: u32,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPreference {
    schema_version: u8,
    version: u32,
    locale: String,
    color_mode: String,
    theme: UserPreferenceTheme,
    navigation: UserPreferenceNavigation,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPreferenceSaveRequest {
    schema_version: u8,
    version: u32,
    locale: String,
    color_mode: String,
    theme: UserPreferenceTheme,
    navigation: UserPreferenceNavigation,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPreferenceTheme {
    primary_mode: String,
    primary: String,
    custom_primary: String,
    neutral_mode: String,
    neutral: String,
    custom_neutral: String,
    radius: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UserPreferenceNavigation {
    pinned_item_ids: Vec<String>,
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
        if self.schema_version != 1 {
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

impl TimerPreference {
    pub(super) fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err("计时器预设版本不支持。".to_string());
        }

        if self.presets.is_empty() || self.presets.len() > 24 {
            return Err("计时器预设数量必须在 1 到 24 个之间。".to_string());
        }

        let mut preset_ids = HashSet::new();
        let mut durations = HashSet::new();

        for preset in &self.presets {
            preset.validate()?;

            if !preset_ids.insert(preset.id.as_str()) {
                return Err("计时器预设包含重复 ID。".to_string());
            }

            if !durations.insert(preset.duration_seconds) {
                return Err("计时器预设包含重复时长。".to_string());
            }
        }

        Ok(())
    }
}

impl TimerPreferencePreset {
    fn validate(&self) -> Result<(), String> {
        validate_timer_preset_id("timer.preset.id", &self.id)?;
        validate_u8_range("timer.preset.order", self.order, 0, 23)?;
        validate_u32_range(
            "timer.preset.durationSeconds",
            self.duration_seconds,
            1,
            86_400,
        )?;
        validate_trimmed_text("timer.preset.createdAt", &self.created_at, 1, 80)?;
        Ok(())
    }
}

impl TimerPreferenceSaveRequest {
    pub(super) fn to_backend_body(&self) -> Result<Value, String> {
        self.validate()?;
        serde_json::to_value(self).map_err(|error| format!("序列化计时器预设失败：{error}"))
    }

    fn validate(&self) -> Result<(), String> {
        if self.schema_version != 1 {
            return Err("计时器预设版本不支持。".to_string());
        }

        if self.presets.is_empty() || self.presets.len() > 24 {
            return Err("计时器预设数量必须在 1 到 24 个之间。".to_string());
        }

        let mut preset_ids = HashSet::new();
        let mut durations = HashSet::new();

        for preset in &self.presets {
            preset.validate()?;

            if !preset_ids.insert(preset.id.as_str()) {
                return Err("计时器预设包含重复 ID。".to_string());
            }

            if !durations.insert(preset.duration_seconds) {
                return Err("计时器预设包含重复时长。".to_string());
            }
        }

        Ok(())
    }
}

impl TimerPreferencePresetRequest {
    fn validate(&self) -> Result<(), String> {
        validate_timer_preset_id("timer.preset.id", &self.id)?;
        validate_u8_range("timer.preset.order", self.order, 0, 23)?;
        validate_u32_range(
            "timer.preset.durationSeconds",
            self.duration_seconds,
            1,
            86_400,
        )?;
        Ok(())
    }
}

impl UserPreference {
    pub(super) fn validate(&self) -> Result<(), String> {
        validate_user_preference(
            self.schema_version,
            &self.locale,
            &self.color_mode,
            &self.theme,
            &self.navigation,
        )
    }
}

impl UserPreferenceSaveRequest {
    pub(super) fn to_backend_body(&self) -> Result<Value, String> {
        self.validate()?;
        serde_json::to_value(self).map_err(|error| format!("序列化用户偏好失败：{error}"))
    }

    fn validate(&self) -> Result<(), String> {
        validate_user_preference(
            self.schema_version,
            &self.locale,
            &self.color_mode,
            &self.theme,
            &self.navigation,
        )
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

fn validate_u32_range(name: &str, value: u32, min: u32, max: u32) -> Result<(), String> {
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

fn validate_timer_preset_id(name: &str, value: &str) -> Result<(), String> {
    validate_text(name, value, 1, 120)?;

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

fn validate_user_preference(
    schema_version: u8,
    locale: &str,
    color_mode: &str,
    theme: &UserPreferenceTheme,
    navigation: &UserPreferenceNavigation,
) -> Result<(), String> {
    if schema_version != 1 {
        return Err("用户偏好版本不支持。".to_string());
    }

    validate_one_of("userPreference.locale", locale, &["zh-CN", "en-US"])?;
    validate_one_of(
        "userPreference.colorMode",
        color_mode,
        &["system", "light", "dark"],
    )?;
    validate_user_preference_theme(theme)?;
    validate_user_preference_navigation(navigation)
}

fn validate_user_preference_theme(theme: &UserPreferenceTheme) -> Result<(), String> {
    validate_one_of(
        "userPreference.theme.primaryMode",
        &theme.primary_mode,
        &["preset", "custom"],
    )?;
    validate_one_of(
        "userPreference.theme.primary",
        &theme.primary,
        USER_PREFERENCE_PRIMARY_COLORS,
    )?;
    validate_hex_color("userPreference.theme.customPrimary", &theme.custom_primary)?;
    validate_one_of(
        "userPreference.theme.neutralMode",
        &theme.neutral_mode,
        &["preset", "custom"],
    )?;
    validate_one_of(
        "userPreference.theme.neutral",
        &theme.neutral,
        USER_PREFERENCE_NEUTRAL_COLORS,
    )?;
    validate_hex_color("userPreference.theme.customNeutral", &theme.custom_neutral)?;
    validate_one_of(
        "userPreference.theme.radius",
        &theme.radius,
        USER_PREFERENCE_RADII,
    )
}

fn validate_user_preference_navigation(
    navigation: &UserPreferenceNavigation,
) -> Result<(), String> {
    if navigation.pinned_item_ids.len() > 6 {
        return Err("顶栏固定菜单数量不能超过 6 个。".to_string());
    }

    let mut item_ids = HashSet::new();

    for item_id in &navigation.pinned_item_ids {
        validate_navigation_item_id("userPreference.navigation.pinnedItemId", item_id)?;

        if !item_ids.insert(item_id.as_str()) {
            return Err("顶栏固定菜单项重复。".to_string());
        }
    }

    Ok(())
}

fn validate_navigation_item_id(name: &str, value: &str) -> Result<(), String> {
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

fn validate_hex_color(name: &str, value: &str) -> Result<(), String> {
    if value.len() == 7
        && value.starts_with('#')
        && value
            .chars()
            .skip(1)
            .all(|character| character.is_ascii_hexdigit())
    {
        return Ok(());
    }

    Err(format!("{name} 必须是 #RRGGBB 色号。"))
}

fn validate_one_of(name: &str, value: &str, allowed_values: &[&str]) -> Result<(), String> {
    if allowed_values.contains(&value) {
        return Ok(());
    }

    Err(format!("{name} 不在允许范围内。"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workbench_layout_rejects_overlapping_widgets() {
        let layout = WorkbenchLayout {
            schema_version: 1,
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
            schema_version: 1,
            version: 1,
            rows: 2,
            columns: 2,
            gap: 12,
            widgets: vec![widget("first", "quick-links", 0, 0, 0, 1, 1)],
        };
        let body = layout.to_backend_body().unwrap();

        assert_eq!(body["schemaVersion"], 1);
        assert_eq!(body["version"], 1);
        assert_eq!(body["widgets"][0]["id"], "first");
        assert_eq!(body["widgets"][0]["header"]["title"], true);
    }

    #[test]
    fn timer_preference_save_request_rejects_duplicate_durations() {
        let request = TimerPreferenceSaveRequest {
            schema_version: 1,
            version: 1,
            presets: vec![
                timer_preset("timer-60", 0, 60),
                timer_preset("timer-60-copy", 1, 60),
            ],
        };

        assert_eq!(
            request.to_backend_body().unwrap_err(),
            "计时器预设包含重复时长。"
        );
    }

    #[test]
    fn user_preference_save_request_rejects_duplicate_navigation_items() {
        let request = user_preference_request(vec!["timer", "timer"]);

        assert_eq!(
            request.to_backend_body().unwrap_err(),
            "顶栏固定菜单项重复。"
        );
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

    fn timer_preset(id: &str, order: u8, duration_seconds: u32) -> TimerPreferencePresetRequest {
        TimerPreferencePresetRequest {
            id: id.to_string(),
            order,
            duration_seconds,
        }
    }

    fn user_preference_request(pinned_item_ids: Vec<&str>) -> UserPreferenceSaveRequest {
        UserPreferenceSaveRequest {
            schema_version: 1,
            version: 1,
            locale: "zh-CN".to_string(),
            color_mode: "dark".to_string(),
            theme: UserPreferenceTheme {
                primary_mode: "custom".to_string(),
                primary: "green".to_string(),
                custom_primary: "#3366ff".to_string(),
                neutral_mode: "preset".to_string(),
                neutral: "slate".to_string(),
                custom_neutral: "#64748b".to_string(),
                radius: "0.375".to_string(),
            },
            navigation: UserPreferenceNavigation {
                pinned_item_ids: pinned_item_ids.into_iter().map(String::from).collect(),
            },
        }
    }
}
