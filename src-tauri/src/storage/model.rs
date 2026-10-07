use super::StorageError;
use crate::application::Endpoint;
use serde::{Deserialize, Serialize};

pub const MAX_PROFILES: usize = 100;
pub const MAX_FILE_BYTES: usize = 128 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Appearance {
    pub font_size: u8,
    pub foreground: String,
    pub background: String,
}
impl Default for Appearance {
    fn default() -> Self {
        Self {
            font_size: 14,
            foreground: "#e6e8eb".into(),
            background: "#111318".into(),
        }
    }
}
impl Appearance {
    pub fn validate(&self) -> Result<(), StorageError> {
        let color = |value: &str| {
            value.len() == 7
                && value.starts_with('#')
                && value.as_bytes()[1..].iter().all(u8::is_ascii_hexdigit)
        };
        if !(10..=24).contains(&self.font_size)
            || !color(&self.foreground)
            || !color(&self.background)
        {
            return Err(StorageError::InvalidAppearance);
        }
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Profile {
    pub id: u64,
    pub name: String,
    pub host: String,
    pub port: u16,
    pub appearance: Appearance,
}
#[derive(Clone)]
pub struct ProfileInput {
    pub name: String,
    pub host: String,
    pub port: u32,
    pub appearance: Appearance,
}
impl ProfileInput {
    pub(super) fn record(self, id: u64) -> Result<Profile, StorageError> {
        let name = self.name.trim();
        validate_name(name)?;
        let name = name.to_owned();
        Endpoint::parse(&self.host, self.port).map_err(|_| StorageError::InvalidEndpoint)?;
        self.appearance.validate()?;
        Ok(Profile {
            id,
            name,
            host: self.host,
            port: self.port as u16,
            appearance: self.appearance,
        })
    }
}
fn validate_name(name: &str) -> Result<(), StorageError> {
    if name.is_empty()
        || name.len() > 128
        || name.chars().any(char::is_control)
        || name != name.trim()
    {
        return Err(StorageError::InvalidName);
    }
    Ok(())
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Document {
    pub version: u8,
    pub next_id: u64,
    pub selected: Option<u64>,
    pub profiles: Vec<Profile>,
}
impl Default for Document {
    fn default() -> Self {
        Self {
            version: 1,
            next_id: 1,
            selected: None,
            profiles: Vec::new(),
        }
    }
}
impl Document {
    pub fn validate(&self) -> Result<(), StorageError> {
        if self.version != 1 {
            return Err(StorageError::UnsupportedVersion);
        }
        if self.profiles.len() > MAX_PROFILES || self.next_id == 0 {
            return Err(StorageError::Limit);
        }
        let mut ids = std::collections::HashSet::new();
        let mut names = std::collections::HashSet::new();
        for p in &self.profiles {
            validate_name(&p.name)?;
            Endpoint::parse(&p.host, u32::from(p.port))
                .map_err(|_| StorageError::InvalidEndpoint)?;
            p.appearance.validate()?;
            if p.id == 0 || p.id >= self.next_id || !ids.insert(p.id) {
                return Err(StorageError::InvalidFile);
            }
            if !names.insert(&p.name) {
                return Err(StorageError::DuplicateName);
            }
        }
        if self.selected.is_some_and(|id| !ids.contains(&id)) {
            return Err(StorageError::InvalidFile);
        }
        Ok(())
    }
}
