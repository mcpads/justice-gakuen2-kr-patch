//! Recoverable replacement of the fixed BIN/CUE/report names. This is not an
//! atomic snapshot for external readers: use a separate output directory while
//! an emulator owns a disc. The journal rolls interrupted replacement back on
//! the next locked prepare; member-rename errors roll back before returning.
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, ensure};

const DIRECTORY: &str = ".disc-publication";
const JOURNAL: &str = "previous.json";

pub(super) fn publish(
    staged: &[&Path; 3],
    finals: &[&Path; 3],
    mut move_output: impl FnMut(&Path, &Path) -> std::io::Result<()>,
) -> Result<()> {
    for path in staged {
        ensure!(
            regular_file(path)?,
            "dialogue development disc staged output is missing: {}",
            path.display()
        );
        std::fs::File::open(path)?.sync_all()?;
    }
    let directory = journal_directory(finals)?;
    ensure!(
        !directory.exists(),
        "unfinished disc publication must be recovered before publishing"
    );
    let mut previous = [false; 3];
    for (index, path) in finals.iter().enumerate() {
        previous[index] = regular_file(path)?;
    }
    std::fs::create_dir(&directory)?;
    // Hard links retain the previous bytes without another full-disc copy.
    // No final is changed until every backup and the journal are ready.
    for (index, path) in finals.iter().enumerate() {
        if previous[index] {
            std::fs::hard_link(path, backup(&directory, index))?;
        }
    }
    let temporary = directory.join("previous.json.tmp");
    std::fs::write(&temporary, serde_json::to_vec(&previous)?)?;
    std::fs::File::open(&temporary)?.sync_all()?;
    std::fs::rename(temporary, directory.join(JOURNAL))?;
    sync_directory(&directory)?;
    sync_directory(directory.parent().context("publication has no parent")?)?;

    for (source, target) in staged.iter().zip(finals) {
        if let Err(error) = move_output(source, target) {
            recover(finals).context("publication failed and rollback is incomplete; retain the output directory for recovery")?;
            return Err(error).with_context(|| {
                format!(
                    "failed to publish {}; previous set restored",
                    target.display()
                )
            });
        }
    }
    sync_directory(directory.parent().context("publication has no parent")?)?;
    // Removing the journal commits the set. A restart before this boundary
    // restores all previous members, even if all three renames had completed.
    std::fs::remove_file(directory.join(JOURNAL))?;
    sync_directory(&directory)?;
    std::fs::remove_dir_all(directory)?;
    Ok(())
}

pub(super) fn recover(finals: &[&Path; 3]) -> Result<()> {
    let directory = journal_directory(finals)?;
    if !directory.exists() {
        return Ok(());
    }
    let journal = directory.join(JOURNAL);
    if journal.exists() {
        let previous: [bool; 3] = serde_json::from_slice(&std::fs::read(&journal)?)?;
        // Check all backups before changing any final. Retain them throughout
        // rollback so recovery itself can be retried after interruption.
        for (index, exists) in previous.iter().enumerate() {
            if *exists {
                ensure!(
                    regular_file(&backup(&directory, index))?,
                    "missing publication recovery member {index}"
                );
            }
        }
        for (index, target) in finals.iter().enumerate() {
            regular_file(target)?;
            if previous[index] {
                let restored = directory.join(format!("restore-{index}"));
                if restored.exists() {
                    std::fs::remove_file(&restored)?;
                }
                std::fs::hard_link(backup(&directory, index), &restored)?;
                std::fs::rename(restored, target)?;
            } else if target.exists() {
                std::fs::remove_file(target)?;
            }
        }
        sync_directory(directory.parent().context("publication has no parent")?)?;
        std::fs::remove_file(journal)?;
        sync_directory(&directory)?;
    }
    // Without a journal, replacement either never began or already committed.
    std::fs::remove_dir_all(directory)?;
    Ok(())
}

fn regular_file(path: &Path) -> Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            ensure!(
                metadata.file_type().is_file(),
                "disc publication path is not a regular file: {}",
                path.display()
            );
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error.into()),
    }
}

fn journal_directory(finals: &[&Path; 3]) -> Result<PathBuf> {
    Ok(finals[0]
        .parent()
        .context("disc output has no parent")?
        .join(DIRECTORY))
}

fn backup(directory: &Path, index: usize) -> PathBuf {
    directory.join(format!("previous-{index}"))
}

fn sync_directory(path: &Path) -> Result<()> {
    std::fs::File::open(path)?.sync_all()?;
    Ok(())
}
