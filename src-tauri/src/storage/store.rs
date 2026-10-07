use super::{model::Document, *};
use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::Path,
    sync::atomic::AtomicU64,
};
#[cfg(test)]
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum WriteStage {
    Create,
    Write,
    Flush,
    Sync,
    Replace,
}

static TEMP_ID: AtomicU64 = AtomicU64::new(1);

pub(super) struct Store {
    directory: Option<PathBuf>,
    lock: Option<File>,
    document: Document,
    warning: Option<StorageError>,
    #[cfg(test)]
    pub fail_at: Option<WriteStage>,
}
impl Store {
    pub fn new(directory: Option<PathBuf>) -> Self {
        Self {
            directory,
            lock: None,
            document: Document::default(),
            warning: Some(StorageError::Unavailable),
            #[cfg(test)]
            fail_at: None,
        }
    }
    pub fn release(&mut self) {
        self.lock = None;
        self.warning = Some(StorageError::ReadOnly);
    }
    fn snapshot(&self) -> ProfileSnapshot {
        ProfileSnapshot {
            profiles: self.document.profiles.clone(),
            selected: self.document.selected,
            writable: self.lock.is_some() && self.warning.is_none(),
            warning: self.warning,
        }
    }
    fn load(&mut self) -> Result<(), StorageError> {
        let directory = self.directory.as_ref().ok_or(StorageError::Unavailable)?;
        fs::create_dir_all(directory).map_err(|_| StorageError::Unavailable)?;
        let mut warning = None;
        if self.lock.is_none() {
            let lock = OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .truncate(false)
                .open(directory.join("profiles.lock"))
                .map_err(|_| StorageError::Unavailable)?;
            match lock.try_lock() {
                Ok(()) => self.lock = Some(lock),
                Err(std::fs::TryLockError::WouldBlock) => warning = Some(StorageError::ReadOnly),
                Err(_) => return Err(StorageError::Unavailable),
            }
        }
        let path = directory.join("profiles.json");
        let document = match File::open(path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Document::default(),
            Err(_) => return Err(StorageError::Unavailable),
            Ok(file) => {
                if !file
                    .metadata()
                    .map_err(|_| StorageError::Unavailable)?
                    .is_file()
                {
                    return Err(StorageError::InvalidFile);
                }
                let mut bytes = Vec::new();
                file.take((MAX_FILE_BYTES + 1) as u64)
                    .read_to_end(&mut bytes)
                    .map_err(|_| StorageError::Unavailable)?;
                if bytes.len() > MAX_FILE_BYTES {
                    return Err(StorageError::Limit);
                }
                let value: serde_json::Value =
                    serde_json::from_slice(&bytes).map_err(|_| StorageError::InvalidFile)?;
                match value.get("version").and_then(serde_json::Value::as_u64) {
                    Some(1) => {}
                    Some(_) => return Err(StorageError::UnsupportedVersion),
                    None => return Err(StorageError::InvalidFile),
                }
                let document: Document =
                    serde_json::from_slice(&bytes).map_err(|_| StorageError::InvalidFile)?;
                document.validate()?;
                document
            }
        };
        self.document = document;
        self.warning = warning;
        Ok(())
    }
    pub fn execute(
        &mut self,
        operation: ProfileOperation,
    ) -> Result<ProfileSnapshot, StorageError> {
        if matches!(operation, ProfileOperation::Load) {
            if let Err(error) = self.load() {
                self.warning = Some(error);
            }
            return Ok(self.snapshot());
        }
        if !self.snapshot().writable {
            return Err(StorageError::ReadOnly);
        }
        let mut next = self.document.clone();
        match operation {
            ProfileOperation::Load => unreachable!(),
            ProfileOperation::Create(input) => {
                if next.profiles.len() >= MAX_PROFILES {
                    return Err(StorageError::Limit);
                }
                let id = next.next_id;
                next.next_id = id.checked_add(1).ok_or(StorageError::Limit)?;
                next.profiles.push(input.record(id)?);
                next.selected = Some(id);
            }
            ProfileOperation::Update { id, input } => {
                let record = next
                    .profiles
                    .iter_mut()
                    .find(|p| p.id == id)
                    .ok_or(StorageError::MissingProfile)?;
                *record = input.record(id)?;
            }
            ProfileOperation::Appearance { id, appearance } => {
                appearance.validate()?;
                next.profiles
                    .iter_mut()
                    .find(|p| p.id == id)
                    .ok_or(StorageError::MissingProfile)?
                    .appearance = appearance;
            }
            ProfileOperation::Delete(id) => {
                let index = next
                    .profiles
                    .iter()
                    .position(|p| p.id == id)
                    .ok_or(StorageError::MissingProfile)?;
                next.profiles.remove(index);
                if next.selected == Some(id) {
                    next.selected = None;
                }
            }
            ProfileOperation::Select(selected) => {
                if selected.is_some_and(|id| !next.profiles.iter().any(|p| p.id == id)) {
                    return Err(StorageError::MissingProfile);
                }
                next.selected = selected;
            }
        }
        next.validate()?;
        let bytes = serde_json::to_vec(&next).map_err(|_| StorageError::InvalidFile)?;
        if bytes.len() > MAX_FILE_BYTES {
            return Err(StorageError::Limit);
        }
        let directory = self.directory.as_ref().ok_or(StorageError::Unavailable)?;
        self.replace(directory, &bytes)?;
        self.document = next;
        Ok(self.snapshot())
    }
    #[cfg(test)]
    fn inject(&self, stage: WriteStage) -> std::io::Result<()> {
        if self.fail_at == Some(stage) {
            Err(std::io::Error::other("injected failure"))
        } else {
            Ok(())
        }
    }
    fn replace(&self, directory: &Path, bytes: &[u8]) -> Result<(), StorageError> {
        #[cfg(test)]
        self.inject(WriteStage::Create)
            .map_err(|_| StorageError::Unavailable)?;
        // create_new prevents truncating a stale temporary file or following a link.
        let mut temporary = None;
        for _ in 0..16 {
            let serial = TEMP_ID.fetch_add(1, Ordering::Relaxed);
            let path = directory.join(format!(".profiles-{}-{serial}.tmp", std::process::id()));
            match OpenOptions::new().write(true).create_new(true).open(&path) {
                Ok(file) => {
                    temporary = Some((path, file));
                    break;
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(_) => return Err(StorageError::Unavailable),
            }
        }
        let (path, mut file) = temporary.ok_or(StorageError::Unavailable)?;
        let result = (|| {
            #[cfg(test)]
            self.inject(WriteStage::Write)?;
            file.write_all(bytes)?;
            #[cfg(test)]
            self.inject(WriteStage::Flush)?;
            file.flush()?;
            #[cfg(test)]
            self.inject(WriteStage::Sync)?;
            file.sync_all()?;
            drop(file);
            #[cfg(test)]
            self.inject(WriteStage::Replace)?;
            fs::rename(&path, directory.join("profiles.json"))
        })();
        if result.is_err() {
            let _ = fs::remove_file(path);
        }
        result.map_err(|_| StorageError::Unavailable)
    }
}
