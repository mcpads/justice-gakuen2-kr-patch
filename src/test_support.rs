use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_TEMPORARY_DIRECTORY: AtomicU64 = AtomicU64::new(0);

pub(crate) fn temporary_directory(scope: &str) -> PathBuf {
    let sequence = NEXT_TEMPORARY_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "justice-gakuen2-{scope}-{}-{sequence}",
        std::process::id()
    ))
}
