//! Registers complete ISO-record replacements before raw-sector finalization.

use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Result, ensure};
use sha2::{Digest, Sha256};

use super::iso9660::{FileRecord, Iso9660};
use super::record_relocation::DiscRecordRelocation;
use super::sector_finalizer;
use super::{RawTrack, USER_DATA_SIZE};

#[derive(Debug, Clone)]
pub struct RecordRebuild {
    pub owner: String,
    pub path: String,
    pub record: FileRecord,
    pub source_sha256: String,
    pub replacement_sha256: String,
    pub target_lbas: Vec<u32>,
    pub changed_lbas: Vec<u32>,
}

#[derive(Debug, Clone)]
pub struct DiscFinalization {
    pub records: Vec<RecordRebuild>,
    /// Directory/volume sectors owned separately from file payload extents.
    pub metadata_lbas: Vec<u32>,
    pub source_sector_count: u64,
    pub output_sector_count: u64,
    pub changed_lbas: Vec<u32>,
    pub output_sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscRecordSourceIdentity {
    byte_count: usize,
    sha256: String,
}

impl DiscRecordSourceIdentity {
    pub fn new(byte_count: usize, sha256: impl Into<String>) -> Result<Self> {
        let sha256 = sha256.into();
        ensure!(
            sha256.len() == 64 && sha256.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "disc record source SHA-256 is not a 64-digit hexadecimal digest"
        );
        Ok(Self {
            byte_count,
            sha256: sha256.to_ascii_lowercase(),
        })
    }

    pub fn from_bytes(source: &[u8]) -> Self {
        Self {
            byte_count: source.len(),
            sha256: sha256_hex(source),
        }
    }

    pub fn byte_count(&self) -> usize {
        self.byte_count
    }

    pub fn sha256(&self) -> &str {
        &self.sha256
    }
}

#[derive(Debug, Clone)]
pub struct DiscRecordContribution<'a> {
    pub owner: String,
    pub path: &'a str,
    pub data: &'a [u8],
    pub source: DiscRecordSourceIdentity,
}

#[derive(Debug, Clone, Copy)]
pub struct FixedRecordReplacement<'a> {
    pub path: &'a str,
    pub data: &'a [u8],
}

#[derive(Debug)]
struct PendingRecord<'a> {
    owner: String,
    path: String,
    data: &'a [u8],
    source: PendingSourceIdentity,
}

#[derive(Debug)]
enum PendingSourceIdentity {
    Declared(DiscRecordSourceIdentity),
    DeriveForLegacyWrapper,
}

/// Central ownership plan for complete ISO-record replacements.
///
/// Registration does not touch source or output media. Finalization first
/// resolves and seals every registered record against one source track, then
/// delegates the only staged-image mutation to the raw-sector finalizer.
#[derive(Debug, Default)]
pub struct DiscRecordPlan<'a> {
    relocation: Option<(String, NativeCatalogBinding)>,
    records: Vec<PendingRecord<'a>>,
    owners_by_path: BTreeMap<String, String>,
}

/// Native loader entry that must be present in the composed executable output.
#[derive(Debug)]
pub struct NativeCatalogBinding {
    pub executable_path: String,
    pub entry_offset: usize,
}

#[derive(Debug)]
pub(super) struct ChangedRecordSector {
    pub(super) lba: u32,
    pub(super) payload_offset: usize,
    pub(super) byte_count: usize,
    pub(super) expected_source_payload: Vec<u8>,
}

#[derive(Debug)]
pub(super) struct SealedDiscRecord<'a> {
    pub(super) owner: String,
    pub(super) path: String,
    pub(super) data: &'a [u8],
    pub(super) record: FileRecord,
    pub(super) source_sha256: String,
    pub(super) replacement_sha256: String,
    pub(super) target_lbas: Vec<u32>,
    pub(super) changed_sectors: Vec<ChangedRecordSector>,
}

#[derive(Debug)]
pub(super) struct SealedDiscRecordPlan<'a> {
    pub(super) relocation: Option<DiscRecordRelocation>,
    pub(super) records: Vec<SealedDiscRecord<'a>>,
}

impl<'a> DiscRecordPlan<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, contribution: DiscRecordContribution<'a>) -> Result<()> {
        let DiscRecordContribution {
            owner,
            path,
            data,
            source,
        } = contribution;
        self.register_pending(owner, path, data, PendingSourceIdentity::Declared(source))
    }

    /// One appended resource per plan; metadata and executable ownership are
    /// checked before the staged image is opened.
    pub fn register_appended(
        &mut self,
        contribution: DiscRecordContribution<'a>,
        binding: NativeCatalogBinding,
    ) -> Result<()> {
        ensure!(
            self.relocation.is_none(),
            "only one appended record is supported per plan"
        );
        ensure!(
            contribution.path != binding.executable_path,
            "resource cannot contain its own native catalog"
        );
        let path = contribution.path.to_owned();
        self.register(contribution)?;
        self.relocation = Some((path, binding));
        Ok(())
    }

    pub(super) fn register_legacy(
        &mut self,
        replacement: FixedRecordReplacement<'a>,
    ) -> Result<()> {
        self.register_pending(
            replacement.path.to_string(),
            replacement.path,
            replacement.data,
            PendingSourceIdentity::DeriveForLegacyWrapper,
        )
    }

    fn register_pending(
        &mut self,
        owner: String,
        path: &str,
        data: &'a [u8],
        source: PendingSourceIdentity,
    ) -> Result<()> {
        ensure!(!owner.trim().is_empty(), "disc record owner is empty");
        ensure!(
            !path.trim().is_empty(),
            "disc record path is empty for owner {owner}"
        );
        if let Some(previous_owner) = self.owners_by_path.get(path) {
            anyhow::bail!("disc record {path} has duplicate owners: {previous_owner} and {owner}");
        }
        self.owners_by_path.insert(path.to_string(), owner.clone());
        self.records.push(PendingRecord {
            owner,
            path: path.to_string(),
            data,
            source,
        });
        Ok(())
    }

    pub fn finalize(self, source_bin: &Path, output_bin: &Path) -> Result<DiscFinalization> {
        ensure!(
            !self.records.is_empty(),
            "no disc record replacements requested"
        );
        ensure_distinct_source_and_output(source_bin, output_bin)?;
        let mut source_track = RawTrack::open(source_bin)?;
        let sealed = self.seal(&mut source_track)?;
        sector_finalizer::finalize(source_bin, output_bin, &mut source_track, sealed)
    }

    fn seal(self, source_track: &mut RawTrack) -> Result<SealedDiscRecordPlan<'a>> {
        let source_sector_count = source_track.sector_count();
        let relocation = if let Some((path, binding)) = &self.relocation {
            let resource = self
                .records
                .iter()
                .find(|r| &r.path == path)
                .context("missing appended resource owner")?;
            let executable = self
                .records
                .iter()
                .find(|r| r.path == binding.executable_path)
                .context("native catalog executable must be registered")?;
            let end = binding
                .entry_offset
                .checked_add(12)
                .context("native catalog offset overflow")?;
            let source_entry = {
                let mut iso = Iso9660::open(source_track)?;
                let record = iso.find(&binding.executable_path)?;
                let bytes = iso.read_record(&record)?;
                bytes
                    .get(binding.entry_offset..end)
                    .context("native catalog entry leaves source executable")?
                    .to_vec()
            };
            let PendingSourceIdentity::Declared(identity) = &resource.source else {
                anyhow::bail!("appended resource requires declared source identity")
            };
            let relocation = DiscRecordRelocation::prepare(
                source_track,
                path,
                identity,
                resource.data,
                &source_entry,
            )?;
            ensure!(
                executable.data.get(binding.entry_offset..end)
                    == Some(relocation.native_entry.as_slice()),
                "composed native catalog does not bind the appended resource"
            );
            Some(relocation)
        } else {
            None
        };
        let records = {
            let mut iso = Iso9660::open(source_track)?;
            let mut records = Vec::with_capacity(self.records.len());
            for pending in self.records {
                let record = iso
                    .find(&pending.path)
                    .with_context(|| format!("failed to resolve disc record {}", pending.path))?;
                ensure!(
                    !record.is_directory(),
                    "disc replacement target {} is a directory",
                    pending.path
                );
                ensure!(
                    record.extended_attribute_blocks == 0,
                    "{} uses unsupported extended attributes",
                    pending.path
                );
                let original = iso.read_record(&record)?;
                let actual_source = DiscRecordSourceIdentity::from_bytes(&original);
                if let PendingSourceIdentity::Declared(expected_source) = pending.source {
                    ensure!(
                        expected_source == actual_source,
                        "declared source identity for {} changed: expected {} bytes {}, found {} bytes {}",
                        pending.path,
                        expected_source.byte_count(),
                        expected_source.sha256(),
                        actual_source.byte_count(),
                        actual_source.sha256()
                    );
                }
                if let Some(relocated) = relocation.as_ref().filter(|r| r.path == pending.path) {
                    records.push(SealedDiscRecord {
                        owner: pending.owner,
                        path: pending.path,
                        data: pending.data,
                        record: relocated.output_record.clone(),
                        source_sha256: actual_source.sha256,
                        replacement_sha256: sha256_hex(pending.data),
                        target_lbas: (relocated.source_sector_count..relocated.output_sector_count)
                            .collect(),
                        changed_sectors: Vec::new(),
                    });
                    continue;
                }
                ensure!(
                    pending.data.len() == original.len(),
                    "replacement size for {} changed from {} to {} bytes",
                    pending.path,
                    original.len(),
                    pending.data.len()
                );

                let sector_count = original.len().div_ceil(USER_DATA_SIZE);
                let sector_count = u32::try_from(sector_count)
                    .context("disc record sector count does not fit in an LBA extent")?;
                let extent_end = record
                    .extent_lba
                    .checked_add(sector_count)
                    .context("disc record LBA extent overflows")?;
                ensure!(
                    u64::from(extent_end) <= source_sector_count,
                    "disc record {} extent escapes the source track",
                    pending.path
                );
                let target_lbas = (record.extent_lba..extent_end).collect::<Vec<_>>();
                let changed_sectors = target_lbas
                    .iter()
                    .enumerate()
                    .filter_map(|(index, &lba)| {
                        let payload_offset = index * USER_DATA_SIZE;
                        let byte_count = USER_DATA_SIZE.min(original.len() - payload_offset);
                        let source_payload = &original[payload_offset..payload_offset + byte_count];
                        let replacement_payload =
                            &pending.data[payload_offset..payload_offset + byte_count];
                        (source_payload != replacement_payload).then(|| ChangedRecordSector {
                            lba,
                            payload_offset,
                            byte_count,
                            expected_source_payload: source_payload.to_vec(),
                        })
                    })
                    .collect();

                records.push(SealedDiscRecord {
                    owner: pending.owner,
                    path: pending.path,
                    data: pending.data,
                    record,
                    source_sha256: actual_source.sha256,
                    replacement_sha256: sha256_hex(pending.data),
                    target_lbas,
                    changed_sectors,
                });
            }
            records
        };

        let mut extent_owners = BTreeMap::<u32, (&str, &str)>::new();
        for record in &records {
            for &lba in &record.target_lbas {
                if let Some((previous_owner, previous_path)) =
                    extent_owners.insert(lba, (&record.owner, &record.path))
                {
                    anyhow::bail!(
                        "disc record extents overlap at LBA {lba}: {previous_path} ({previous_owner}) and {} ({})",
                        record.path,
                        record.owner
                    );
                }
            }
        }

        if let Some(relocation) = &relocation {
            for metadata in &relocation.metadata {
                ensure!(
                    !extent_owners.contains_key(&metadata.lba),
                    "relocation metadata and record sector owners overlap at LBA {}",
                    metadata.lba
                );
            }
        }
        Ok(SealedDiscRecordPlan {
            records,
            relocation,
        })
    }
}

fn sha256_hex(data: &[u8]) -> String {
    format!("{:x}", Sha256::digest(data))
}

fn ensure_distinct_source_and_output(source_bin: &Path, output_bin: &Path) -> Result<()> {
    ensure!(
        source_bin != output_bin,
        "source and staged output BIN paths must differ"
    );
    let source_canonical = source_bin
        .canonicalize()
        .with_context(|| format!("failed to resolve source BIN: {}", source_bin.display()))?;
    if output_bin.exists() {
        let output_canonical = output_bin.canonicalize().with_context(|| {
            format!(
                "failed to resolve existing staged output BIN: {}",
                output_bin.display()
            )
        })?;
        ensure!(
            source_canonical != output_canonical,
            "staged output BIN aliases the immutable source BIN"
        );
        ensure_existing_files_differ(source_bin, output_bin)?;
    }
    Ok(())
}

#[cfg(unix)]
fn ensure_existing_files_differ(source_bin: &Path, output_bin: &Path) -> Result<()> {
    use std::os::unix::fs::MetadataExt;

    let source = std::fs::metadata(source_bin)?;
    let output = std::fs::metadata(output_bin)?;
    ensure!(
        source.dev() != output.dev() || source.ino() != output.ino(),
        "staged output BIN is a hard-link alias of the immutable source BIN"
    );
    Ok(())
}

#[cfg(not(unix))]
fn ensure_existing_files_differ(_source_bin: &Path, _output_bin: &Path) -> Result<()> {
    anyhow::bail!(
        "cannot prove that an existing staged output is distinct from the immutable source on this platform"
    )
}
