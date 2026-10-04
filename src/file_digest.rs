use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result, anyhow, ensure};
use sha2::{Digest, Sha256};

#[cfg(unix)]
use std::collections::HashMap;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
#[cfg(unix)]
use std::sync::{Mutex, OnceLock};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct FileDigestIdentity {
    canonical_path: PathBuf,
    byte_count: u64,
    modified_at: SystemTime,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(unix)]
    changed_at_seconds: i64,
    #[cfg(unix)]
    changed_at_nanoseconds: i64,
}

#[cfg(unix)]
static FILE_DIGESTS: OnceLock<Mutex<HashMap<FileDigestIdentity, String>>> = OnceLock::new();

pub(crate) fn sha256_file(path: &Path) -> Result<String> {
    let identity = FileDigestIdentity::read(path)?;
    #[cfg(unix)]
    {
        let cache = FILE_DIGESTS.get_or_init(|| Mutex::new(HashMap::new()));
        let cache = cache
            .lock()
            .map_err(|_| anyhow!("file digest cache lock poisoned"))?;
        if let Some(digest) = cache.get(&identity).cloned() {
            return Ok(digest);
        }
    }

    let digest = hash_file(&identity.canonical_path)?;
    let verified_identity = FileDigestIdentity::read(&identity.canonical_path)?;
    ensure!(
        verified_identity == identity,
        "file changed while hashing: {}",
        identity.canonical_path.display()
    );
    #[cfg(unix)]
    {
        let cache = FILE_DIGESTS.get_or_init(|| Mutex::new(HashMap::new()));
        let mut cache = cache
            .lock()
            .map_err(|_| anyhow!("file digest cache lock poisoned"))?;
        cache.retain(|cached, _| cached.canonical_path != identity.canonical_path);
        cache.insert(identity, digest.clone());
    }
    Ok(digest)
}

impl FileDigestIdentity {
    fn read(path: &Path) -> Result<Self> {
        let canonical_path = std::fs::canonicalize(path)
            .with_context(|| format!("failed to resolve {}", path.display()))?;
        let metadata = std::fs::metadata(&canonical_path)
            .with_context(|| format!("failed to inspect {}", canonical_path.display()))?;
        ensure!(
            metadata.is_file(),
            "SHA-256 input is not a file: {}",
            canonical_path.display()
        );
        Ok(Self {
            canonical_path,
            byte_count: metadata.len(),
            modified_at: metadata.modified()?,
            #[cfg(unix)]
            device: metadata.dev(),
            #[cfg(unix)]
            inode: metadata.ino(),
            #[cfg(unix)]
            changed_at_seconds: metadata.ctime(),
            #[cfg(unix)]
            changed_at_nanoseconds: metadata.ctime_nsec(),
        })
    }
}

fn hash_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 1024 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("{:x}", digest.finalize()))
}

#[cfg(test)]
#[path = "file_digest_tests.rs"]
mod tests;
