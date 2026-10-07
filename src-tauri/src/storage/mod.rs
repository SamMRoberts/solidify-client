//! Bounded profile persistence, enabled independently of native UI by `profiles`.
//! A single admitted blocking operation owns its permit through completion, even
//! when the IPC caller goes away. Shutdown closes admission and awaits that work.
mod model;
mod store;
pub use model::{Appearance, MAX_FILE_BYTES, MAX_PROFILES, Profile, ProfileInput};
use std::{
    fmt,
    path::PathBuf,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::Semaphore;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StorageError {
    Unavailable,
    InvalidFile,
    UnsupportedVersion,
    Limit,
    InvalidName,
    InvalidEndpoint,
    InvalidAppearance,
    DuplicateName,
    MissingProfile,
    ReadOnly,
    Busy,
    ShuttingDown,
}
impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Unavailable => "Saved connections are unavailable. Check storage access and retry loading.",
            Self::InvalidFile => "Saved connections contain invalid data. The file was left untouched.",
            Self::UnsupportedVersion => "Saved connections use an unsupported format. The file was left untouched.",
            Self::Limit => "Saved connections exceed the 100-profile or 128 KiB limit.",
            Self::InvalidName => "Use a nonempty name of at most 128 UTF-8 bytes without control characters.",
            Self::InvalidEndpoint => "Enter a valid IP address or ASCII hostname and port 1–65535.",
            Self::InvalidAppearance => "Use font size 10–24 and six-digit hex colors.",
            Self::DuplicateName => "A saved connection already has that name.",
            Self::MissingProfile => "This saved connection no longer exists. Retry loading.",
            Self::ReadOnly => "Saved connections are read-only. Another instance may be using them; retry loading when it closes.",
            Self::Busy => "A saved-connection operation is still finishing. Try again shortly.",
            Self::ShuttingDown => "The application is shutting down.",
        })
    }
}
impl std::error::Error for StorageError {}
#[derive(Clone, Debug)]
pub struct ProfileSnapshot {
    pub profiles: Vec<Profile>,
    pub selected: Option<u64>,
    pub writable: bool,
    pub warning: Option<StorageError>,
}
pub enum ProfileOperation {
    Load,
    Create(ProfileInput),
    Update { id: u64, input: ProfileInput },
    Appearance { id: u64, appearance: Appearance },
    Delete(u64),
    Select(Option<u64>),
}
pub struct ProfileService {
    store: Arc<Mutex<store::Store>>,
    admission: Arc<Semaphore>,
    closing: AtomicBool,
}
impl ProfileService {
    /// The native owner resolves this directory; IPC never supplies a path.
    /// None leaves manual connections available when path resolution failed.
    pub fn new(directory: Option<PathBuf>) -> Self {
        Self {
            store: Arc::new(Mutex::new(store::Store::new(directory))),
            admission: Arc::new(Semaphore::new(1)),
            closing: AtomicBool::new(false),
        }
    }
    pub async fn execute(
        &self,
        operation: ProfileOperation,
    ) -> Result<ProfileSnapshot, StorageError> {
        if self.closing.load(Ordering::SeqCst) {
            return Err(StorageError::ShuttingDown);
        }
        let permit = self
            .admission
            .clone()
            .try_acquire_owned()
            .map_err(|_| StorageError::Busy)?;
        if self.closing.load(Ordering::SeqCst) {
            return Err(StorageError::ShuttingDown);
        }
        let store = self.store.clone();
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            store
                .lock()
                .map_err(|_| StorageError::Unavailable)?
                .execute(operation)
        })
        .await
        .map_err(|_| StorageError::Unavailable)?
    }
    pub async fn shutdown(&self) {
        self.closing.store(true, Ordering::SeqCst);
        let _permit = self.admission.acquire().await;
        // No filesystem work remains; dropping the handle releases the OS lock.
        if let Ok(mut store) = self.store.lock() {
            store.release();
        }
    }
}
#[cfg(test)]
mod tests;
