//! Profile IPC has no path, terminal identity, or connection-opening capability.
use super::authorize;
use serde::{Deserialize, Serialize};
use solidify_client::storage::{
    Appearance, ProfileInput, ProfileOperation, ProfileService, ProfileSnapshot,
};
use std::sync::Arc;
use tauri::{State, WebviewWindow};

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AppearanceDto {
    font_size: u8,
    foreground: String,
    background: String,
}
impl From<AppearanceDto> for Appearance {
    fn from(a: AppearanceDto) -> Self {
        Self {
            font_size: a.font_size,
            foreground: a.foreground,
            background: a.background,
        }
    }
}
impl From<Appearance> for AppearanceDto {
    fn from(a: Appearance) -> Self {
        Self {
            font_size: a.font_size,
            foreground: a.foreground,
            background: a.background,
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Draft {
    name: String,
    host: String,
    port: u32,
    appearance: AppearanceDto,
}
impl From<Draft> for ProfileInput {
    fn from(d: Draft) -> Self {
        Self {
            name: d.name,
            host: d.host,
            port: d.port,
            appearance: d.appearance.into(),
        }
    }
}
#[derive(Serialize)]
pub struct ProfileDto {
    id: String,
    name: String,
    host: String,
    port: u16,
    appearance: AppearanceDto,
}
#[derive(Serialize)]
pub struct Profiles {
    profiles: Vec<ProfileDto>,
    selected: Option<String>,
    writable: bool,
    warning: Option<String>,
}
impl From<ProfileSnapshot> for Profiles {
    fn from(s: ProfileSnapshot) -> Self {
        Self {
            profiles: s
                .profiles
                .into_iter()
                .map(|p| ProfileDto {
                    id: p.id.to_string(),
                    name: p.name,
                    host: p.host,
                    port: p.port,
                    appearance: p.appearance.into(),
                })
                .collect(),
            selected: s.selected.map(|id| id.to_string()),
            writable: s.writable,
            warning: s.warning.map(|e| e.to_string()),
        }
    }
}
fn profile_id(value: &str) -> Result<u64, String> {
    value
        .parse::<u64>()
        .ok()
        .filter(|id| *id > 0 && id.to_string() == value)
        .ok_or_else(|| "Invalid saved-connection identity.".into())
}
async fn execute(
    service: &ProfileService,
    operation: ProfileOperation,
) -> Result<Profiles, String> {
    service
        .execute(operation)
        .await
        .map(Into::into)
        .map_err(|e| e.to_string())
}
#[tauri::command]
pub async fn load_profiles(
    window: WebviewWindow,
    profiles: State<'_, Arc<ProfileService>>,
) -> Result<Profiles, String> {
    authorize(&window)?;
    execute(&profiles, ProfileOperation::Load).await
}
#[tauri::command]
pub async fn create_profile(
    window: WebviewWindow,
    profiles: State<'_, Arc<ProfileService>>,
    draft: Draft,
) -> Result<Profiles, String> {
    authorize(&window)?;
    execute(&profiles, ProfileOperation::Create(draft.into())).await
}
#[tauri::command]
pub async fn update_profile(
    window: WebviewWindow,
    profiles: State<'_, Arc<ProfileService>>,
    profile_id_value: String,
    draft: Draft,
) -> Result<Profiles, String> {
    authorize(&window)?;
    execute(
        &profiles,
        ProfileOperation::Update {
            id: profile_id(&profile_id_value)?,
            input: draft.into(),
        },
    )
    .await
}
#[tauri::command]
pub async fn update_profile_appearance(
    window: WebviewWindow,
    profiles: State<'_, Arc<ProfileService>>,
    profile_id_value: String,
    appearance: AppearanceDto,
) -> Result<Profiles, String> {
    authorize(&window)?;
    execute(
        &profiles,
        ProfileOperation::Appearance {
            id: profile_id(&profile_id_value)?,
            appearance: appearance.into(),
        },
    )
    .await
}
#[tauri::command]
pub async fn delete_profile(
    window: WebviewWindow,
    profiles: State<'_, Arc<ProfileService>>,
    profile_id_value: String,
) -> Result<Profiles, String> {
    authorize(&window)?;
    execute(
        &profiles,
        ProfileOperation::Delete(profile_id(&profile_id_value)?),
    )
    .await
}
#[tauri::command]
pub async fn select_profile(
    window: WebviewWindow,
    profiles: State<'_, Arc<ProfileService>>,
    selected: Option<String>,
) -> Result<Profiles, String> {
    authorize(&window)?;
    execute(
        &profiles,
        ProfileOperation::Select(selected.map(|id| profile_id(&id)).transpose()?),
    )
    .await
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn boundary_ids_and_drafts_are_strict() {
        for value in ["0", "01", "-1", "18446744073709551616", "path/file"] {
            assert!(profile_id(value).is_err());
        }
        assert_eq!(profile_id("18446744073709551615").unwrap(), u64::MAX);
        assert!(serde_json::from_str::<Draft>(r##"{"name":"Demo","host":"localhost","port":4000,"appearance":{"fontSize":14,"foreground":"#ffffff","background":"#000000"},"path":"/tmp"}"##).is_err());
    }
}
