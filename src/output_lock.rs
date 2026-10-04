//! Advisory process locks for tool-owned output directories. The lock file is
//! deliberately retained: unlinking it would let a second inode evade the lock.
use std::fs::File;
use std::path::Path;

use anyhow::{Context, Result};

pub(crate) struct OutputLock {
    _file: File,
}

impl OutputLock {
    pub(crate) fn writer(directory: &Path) -> Result<Self> {
        std::fs::create_dir_all(directory)?;
        Self::acquire(directory, false)
    }

    pub(crate) fn reader(directory: &Path) -> Result<Self> {
        Self::acquire(directory, true)
    }

    fn acquire(directory: &Path, shared: bool) -> Result<Self> {
        let path = directory.join(".output.lock");
        let file = File::options()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)
            .with_context(|| format!("cannot open output lock {}", path.display()))?;
        let result = if shared {
            file.try_lock_shared()
        } else {
            file.try_lock()
        };
        result.with_context(|| {
            format!(
                "output directory is busy or cannot be locked: {}",
                directory.display()
            )
        })?;
        Ok(Self { _file: file })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writers_exclude_readers_and_other_writers_and_release_on_drop() {
        let root = crate::test_support::temporary_directory("output-lock");
        let writer = OutputLock::writer(&root).unwrap();
        assert!(OutputLock::writer(&root).is_err());
        assert!(OutputLock::reader(&root).is_err());
        drop(writer);
        let first = OutputLock::reader(&root).unwrap();
        let second = OutputLock::reader(&root).unwrap();
        assert!(OutputLock::writer(&root).is_err());
        drop((first, second));
        drop(OutputLock::writer(&root).unwrap());
        std::fs::remove_dir_all(root).unwrap();
    }
}
