//! Process-global lock for tests that mutate `PIXIEKIT_CONFIG_DIR`.
//!
//! `preset` and `recent` both write under the config dir, so their tests must
//! serialize on a single mutex — otherwise parallel runs clobber each other's
//! tempdir env var.

use std::sync::{Mutex, MutexGuard, OnceLock};

pub(crate) fn config_dir_lock() -> MutexGuard<'static, ()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
        .lock()
        .unwrap_or_else(|p| p.into_inner())
}
