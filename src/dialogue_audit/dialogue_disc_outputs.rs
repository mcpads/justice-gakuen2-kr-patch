use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};

#[path = "dialogue_disc_outputs/publication.rs"]
mod publication;

const OUTPUT_STEM: &str = "justice-gakuen2-korean-development";

pub(super) struct DialogueDiscOutputs {
    _lock: crate::output_lock::OutputLock,
    bin: PathBuf,
    cue: PathBuf,
    manifest: PathBuf,
    staged_bin: PathBuf,
    staged_cue: PathBuf,
    staged_manifest: PathBuf,
}

impl DialogueDiscOutputs {
    pub(super) fn prepare(output_dir: &Path, force: bool) -> Result<Self> {
        std::fs::create_dir_all(output_dir)
            .with_context(|| format!("failed to create {}", output_dir.display()))?;
        let lock = crate::output_lock::OutputLock::writer(output_dir)?;
        let outputs = Self::in_directory(output_dir, lock);
        publication::recover(&outputs.final_paths())?;

        if !force
            && outputs
                .final_paths()
                .into_iter()
                .chain(outputs.staged_paths())
                .any(Path::exists)
        {
            bail!("dialogue development disc output exists; pass --force to replace it");
        }

        for path in outputs.final_paths() {
            ensure!(
                !path.exists() || path.is_file(),
                "dialogue development disc output is not a file: {}",
                path.display()
            );
        }
        if force {
            outputs.remove_staged()?;
        }
        Ok(outputs)
    }

    pub(super) fn bin(&self) -> &Path {
        &self.bin
    }

    pub(super) fn cue(&self) -> &Path {
        &self.cue
    }

    #[cfg(test)]
    pub(super) fn manifest(&self) -> &Path {
        &self.manifest
    }

    pub(super) fn staged_bin(&self) -> &Path {
        &self.staged_bin
    }

    pub(super) fn staged_cue(&self) -> &Path {
        &self.staged_cue
    }

    pub(super) fn staged_manifest(&self) -> &Path {
        &self.staged_manifest
    }

    pub(super) fn publish(&self) -> Result<()> {
        self.publish_with(|source, target| std::fs::rename(source, target))
    }

    pub(super) fn publish_with(
        &self,
        move_output: impl FnMut(&Path, &Path) -> std::io::Result<()>,
    ) -> Result<()> {
        publication::publish(&self.staged_paths(), &self.final_paths(), move_output)
    }

    pub(super) fn remove_staged(&self) -> Result<()> {
        for path in self.staged_paths() {
            if path.exists() {
                std::fs::remove_file(path)
                    .with_context(|| format!("failed to remove {}", path.display()))?;
            }
        }
        Ok(())
    }

    fn in_directory(output_dir: &Path, lock: crate::output_lock::OutputLock) -> Self {
        Self {
            _lock: lock,
            bin: output_dir.join(format!("{OUTPUT_STEM}.bin")),
            cue: output_dir.join(format!("{OUTPUT_STEM}.cue")),
            manifest: output_dir.join(format!("{OUTPUT_STEM}.json")),
            staged_bin: output_dir.join(format!("{OUTPUT_STEM}.bin.tmp")),
            staged_cue: output_dir.join(format!("{OUTPUT_STEM}.cue.tmp")),
            staged_manifest: output_dir.join(format!("{OUTPUT_STEM}.json.tmp")),
        }
    }

    fn final_paths(&self) -> [&Path; 3] {
        [&self.bin, &self.cue, &self.manifest]
    }

    fn staged_paths(&self) -> [&Path; 3] {
        [&self.staged_bin, &self.staged_cue, &self.staged_manifest]
    }
}
