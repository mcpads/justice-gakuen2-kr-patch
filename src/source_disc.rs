use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use anyhow::{Context, Result, bail, ensure};
use sha2::{Digest, Sha256};

use crate::cue::CueSheet;

#[path = "source_disc/context.rs"]
mod context;
#[path = "source_disc/loaded_image.rs"]
mod loaded_image;
#[path = "source_disc/main_executable.rs"]
mod main_executable;
#[path = "source_disc/profile.rs"]
pub(crate) mod profile;

pub(crate) use context::SupportedSourceDisc;
#[cfg(test)]
pub(crate) use loaded_image::LoadedImageEntrypoint;
pub(crate) use loaded_image::{
    LoadedImage, dat1_runtime_base, load_dat1_images, load_main_executable_image,
    loaded_main_executable, read_dat1_images,
};
pub(crate) use main_executable::{
    MAIN_TEXT_RUNTIME_BASE, MAIN_TEXT_SIZE, PSX_EXE_HEADER_SIZE, main_executable_entrypoint,
    main_executable_text,
};
pub(crate) const MAIN_EXECUTABLE_PATH: &str = profile::MAIN_EXECUTABLE_RECORD.path;
pub(crate) const MAIN_EXECUTABLE_SHA256: &str = profile::MAIN_EXECUTABLE_RECORD.sha256;

use profile::{SOURCE_BIN_BYTE_COUNT, SOURCE_BIN_SHA256};

const SNAPSHOT_BIN_FILE: &str = "source.bin";
const SNAPSHOT_CUE_FILE: &str = "source.cue";
const COPY_BUFFER_BYTE_COUNT: usize = 1024 * 1024;

static NEXT_SNAPSHOT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

pub(crate) struct VerifiedSourceDisc {
    original_cue: CueSheet,
    source: SupportedSourceDisc,
    snapshot_cue_path: PathBuf,
    _snapshot_directory: SnapshotDirectory,
}

impl VerifiedSourceDisc {
    pub(crate) fn open_supported(cue_path: &Path) -> Result<Self> {
        Self::create_in(
            cue_path,
            SOURCE_BIN_SHA256,
            SOURCE_BIN_BYTE_COUNT,
            &std::env::temp_dir(),
        )
    }

    fn create_in(
        cue_path: &Path,
        expected_sha256: &str,
        expected_byte_count: u64,
        snapshot_parent: &Path,
    ) -> Result<Self> {
        let original_cue = CueSheet::parse(cue_path)?;
        let mut source = File::open(&original_cue.image_path).with_context(|| {
            format!(
                "failed to open source BIN: {}",
                original_cue.image_path.display()
            )
        })?;
        let source_metadata = source.metadata()?;
        ensure!(
            source_metadata.is_file(),
            "source BIN is not a file: {}",
            original_cue.image_path.display()
        );
        ensure!(
            source_metadata.len() == expected_byte_count,
            "unsupported source BIN size: expected {expected_byte_count}, got {}",
            source_metadata.len()
        );

        let snapshot_directory = SnapshotDirectory::create(snapshot_parent)?;
        let snapshot_bin_path = snapshot_directory.path().join(SNAPSHOT_BIN_FILE);
        let snapshot_cue_path = snapshot_directory.path().join(SNAPSHOT_CUE_FILE);
        let mut snapshot_bin = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&snapshot_bin_path)
            .with_context(|| {
                format!(
                    "failed to create source BIN snapshot: {}",
                    snapshot_bin_path.display()
                )
            })?;
        let (copied_byte_count, copied_sha256) = copy_and_hash(&mut source, &mut snapshot_bin)?;
        ensure!(
            copied_byte_count == expected_byte_count,
            "source BIN changed size while snapshotting: expected {expected_byte_count}, copied {copied_byte_count}"
        );
        ensure!(
            copied_sha256 == expected_sha256,
            "unsupported source BIN SHA-256: {copied_sha256}"
        );
        snapshot_bin.flush()?;
        snapshot_bin.sync_all()?;
        drop(snapshot_bin);

        let (stored_byte_count, stored_sha256) = hash_file(&snapshot_bin_path)?;
        ensure!(
            stored_byte_count == expected_byte_count && stored_sha256 == expected_sha256,
            "stored source BIN snapshot differs from its verified input"
        );
        set_snapshot_read_only(&snapshot_bin_path)?;

        let snapshot_cue_text =
            original_cue.rewritten_for(&snapshot_cue_path, &snapshot_bin_path)?;
        std::fs::write(&snapshot_cue_path, &snapshot_cue_text).with_context(|| {
            format!(
                "failed to write source CUE snapshot: {}",
                snapshot_cue_path.display()
            )
        })?;
        set_snapshot_read_only(&snapshot_cue_path)?;
        let snapshot_cue = CueSheet::parse(&snapshot_cue_path)?;
        ensure!(
            snapshot_cue.image_path == snapshot_bin_path
                && snapshot_cue.source_text == snapshot_cue_text,
            "stored source CUE snapshot differs from its verified input"
        );

        Ok(Self {
            original_cue,
            source: SupportedSourceDisc::from_verified_parts(snapshot_cue, stored_sha256.clone()),
            snapshot_cue_path,
            _snapshot_directory: snapshot_directory,
        })
    }

    pub(crate) fn snapshot_cue_path(&self) -> &Path {
        &self.snapshot_cue_path
    }

    pub(crate) fn snapshot_image_path(&self) -> &Path {
        self.source.image_path()
    }

    pub(crate) fn source_bin_sha256(&self) -> &str {
        self.source.source_bin_sha256()
    }

    pub(crate) fn source(&self) -> &SupportedSourceDisc {
        &self.source
    }

    pub(crate) fn rewritten_output_cue(
        &self,
        output_cue: &Path,
        output_bin: &Path,
    ) -> Result<String> {
        self.original_cue.rewritten_for(output_cue, output_bin)
    }
}

fn copy_and_hash(source: &mut File, target: &mut File) -> Result<(u64, String)> {
    let mut digest = Sha256::new();
    let mut byte_count = 0_u64;
    let mut buffer = [0_u8; COPY_BUFFER_BYTE_COUNT];
    loop {
        let count = source.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        target.write_all(&buffer[..count])?;
        digest.update(&buffer[..count]);
        byte_count = byte_count
            .checked_add(count as u64)
            .context("source BIN snapshot size overflow")?;
    }
    Ok((byte_count, format!("{:x}", digest.finalize())))
}

fn hash_file(path: &Path) -> Result<(u64, String)> {
    let mut source = File::open(path)?;
    let mut digest = Sha256::new();
    let mut byte_count = 0_u64;
    let mut buffer = [0_u8; COPY_BUFFER_BYTE_COUNT];
    loop {
        let count = source.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
        byte_count = byte_count
            .checked_add(count as u64)
            .context("source BIN snapshot readback size overflow")?;
    }
    Ok((byte_count, format!("{:x}", digest.finalize())))
}

fn set_snapshot_read_only(path: &Path) -> Result<()> {
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_readonly(true);
    std::fs::set_permissions(path, permissions)
        .with_context(|| format!("failed to protect source snapshot: {}", path.display()))
}

struct SnapshotDirectory {
    path: PathBuf,
}

impl SnapshotDirectory {
    fn create(parent: &Path) -> Result<Self> {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create source snapshot parent: {}",
                parent.display()
            )
        })?;
        for _ in 0..1024 {
            let sequence = NEXT_SNAPSHOT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
            let path = parent.join(format!(
                "justice-gakuen2-source-disc-{}-{sequence}",
                std::process::id()
            ));
            match std::fs::create_dir(&path) {
                Ok(()) => {
                    let directory = Self { path };
                    protect_snapshot_directory(directory.path())?;
                    return Ok(directory);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!(
                            "failed to create source snapshot directory: {}",
                            path.display()
                        )
                    });
                }
            }
        }
        bail!("failed to reserve a source snapshot directory")
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn remove(&self) -> Result<()> {
        for file_name in [SNAPSHOT_CUE_FILE, SNAPSHOT_BIN_FILE] {
            let path = self.path.join(file_name);
            if path.exists() {
                make_snapshot_writable(&path)?;
                std::fs::remove_file(&path).with_context(|| {
                    format!("failed to remove source snapshot: {}", path.display())
                })?;
            }
        }
        if self.path.exists() {
            std::fs::remove_dir(&self.path).with_context(|| {
                format!(
                    "failed to remove source snapshot directory: {}",
                    self.path.display()
                )
            })?;
        }
        Ok(())
    }
}

impl Drop for SnapshotDirectory {
    fn drop(&mut self) {
        let _ = self.remove();
    }
}

#[cfg(unix)]
fn protect_snapshot_directory(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))?;
    Ok(())
}

#[cfg(not(unix))]
fn protect_snapshot_directory(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(unix)]
fn make_snapshot_writable(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
    Ok(())
}

#[cfg(not(unix))]
fn make_snapshot_writable(path: &Path) -> Result<()> {
    let mut permissions = std::fs::metadata(path)?.permissions();
    permissions.set_readonly(false);
    std::fs::set_permissions(path, permissions)?;
    Ok(())
}

#[cfg(test)]
#[path = "source_disc_tests.rs"]
mod tests;

/// Private integration tests read original records directly, never cached extracts.
#[cfg(test)]
pub(crate) fn test_source_disc() -> SupportedSourceDisc {
    SupportedSourceDisc::open(
        &Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
    )
    .expect("private integration test requires the supported original disc")
}
