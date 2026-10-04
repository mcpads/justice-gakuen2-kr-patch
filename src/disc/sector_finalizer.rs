//! Seals raw Mode 2/Form 1 sector candidates and mutates the staged BIN once.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use anyhow::{Context, Result, ensure};
use expected_write::{ExpectedWrite, ImageRegion, RegionKind, WriteIntent, WritePlan};
use sha2::{Digest, Sha256};

use super::iso9660::Iso9660;
use super::mode2;
use super::record_plan::{
    ChangedRecordSector, DiscFinalization, RecordRebuild, SealedDiscRecord, SealedDiscRecordPlan,
};
use super::{RAW_SECTOR_SIZE, RawTrack, USER_DATA_OFFSET};

const MODE2_FORM1_PROTECTION_OFFSET: usize = 0x818;

#[derive(Debug)]
struct SealedSectorCandidate {
    lba: u32,
    owner: String,
    path: String,
    source: Option<[u8; RAW_SECTOR_SIZE]>,
    replacement: [u8; RAW_SECTOR_SIZE],
}

struct DiscImageAudit {
    changed_lbas: Vec<u32>,
    output_sha256: String,
}

pub(super) fn finalize(
    source_bin: &Path,
    output_bin: &Path,
    source_track: &mut RawTrack,
    plan: SealedDiscRecordPlan<'_>,
) -> Result<DiscFinalization> {
    let mut candidates = seal_sector_candidates(source_track, &plan.records)?;
    if let Some(relocation) = &plan.relocation {
        for metadata in &relocation.metadata {
            ensure!(
                source_track.raw_sector(metadata.lba)? == metadata.original,
                "relocation metadata source changed"
            );
            let mut writes = WritePlan::new().region(ImageRegion {
                id: "relocation-metadata".into(),
                range: 0..RAW_SECTOR_SIZE,
                kind: RegionKind::Metadata,
                reason: "bound ISO extent, size and sector protection".into(),
            });
            for (index, range) in metadata
                .fields
                .iter()
                .cloned()
                .chain(std::iter::once(
                    MODE2_FORM1_PROTECTION_OFFSET..RAW_SECTOR_SIZE,
                ))
                .enumerate()
            {
                writes = writes.write(ExpectedWrite {
                    id: format!("relocation-field-{index}"),
                    owner: relocation.path.clone(),
                    purpose: "relocate declared file and update its volume size".into(),
                    offset: range.start,
                    expected_original: metadata.original[range.clone()].to_vec(),
                    replacement: metadata.replacement[range].to_vec(),
                    intent: WriteIntent::Metadata,
                });
            }
            ensure!(
                writes.apply(&metadata.original, None)?.as_slice() == metadata.replacement,
                "relocation metadata has unclaimed changes"
            );
            candidates.push(SealedSectorCandidate {
                lba: metadata.lba,
                owner: relocation.path.clone(),
                path: relocation.path.clone(),
                source: Some(metadata.original),
                replacement: metadata.replacement,
            });
        }
        for (index, sector) in relocation.appended_sectors.iter().enumerate() {
            candidates.push(SealedSectorCandidate {
                lba: relocation.source_sector_count + u32::try_from(index)?,
                owner: relocation.path.clone(),
                path: relocation.path.clone(),
                source: None,
                replacement: *sector,
            });
        }
    }
    candidates.sort_by_key(|c| c.lba);
    ensure!(
        candidates.windows(2).all(|w| w[0].lba != w[1].lba),
        "record and relocation sector owners overlap"
    );
    write_staged_image(output_bin, source_track, &candidates)?;
    verify_record_readback(output_bin, &plan.records, &candidates)?;
    let audit = audit_staged_image(source_bin, output_bin, &candidates)?;

    let metadata_lbas = plan
        .relocation
        .as_ref()
        .map_or_else(Vec::new, |r| r.metadata.iter().map(|s| s.lba).collect());
    let source_sector_count = source_track.sector_count();
    let output_sector_count = plan
        .relocation
        .as_ref()
        .map_or(source_sector_count, |r| u64::from(r.output_sector_count));
    let appended_path = plan.relocation.as_ref().map(|r| r.path.as_str());
    let records = plan
        .records
        .into_iter()
        .map(|record| RecordRebuild {
            owner: record.owner,
            path: record.path.clone(),
            record: record.record,
            source_sha256: record.source_sha256,
            replacement_sha256: record.replacement_sha256,
            changed_lbas: if appended_path == Some(record.path.as_str()) {
                record.target_lbas.clone()
            } else {
                record
                    .changed_sectors
                    .into_iter()
                    .map(|sector| sector.lba)
                    .collect()
            },
            target_lbas: record.target_lbas,
        })
        .collect();
    Ok(DiscFinalization {
        records,
        metadata_lbas,
        source_sector_count,
        output_sector_count,
        changed_lbas: audit.changed_lbas,
        output_sha256: audit.output_sha256,
    })
}

fn seal_sector_candidates(
    source_track: &mut RawTrack,
    records: &[SealedDiscRecord<'_>],
) -> Result<Vec<SealedSectorCandidate>> {
    let mut candidates = Vec::new();
    for record in records {
        for changed in &record.changed_sectors {
            let source = source_track.raw_sector(changed.lba)?;
            ensure!(
                mode2::verify(&source),
                "source sector {} for {} is not a valid Mode 2/Form 1 sector",
                changed.lba,
                record.path
            );
            ensure!(
                source[USER_DATA_OFFSET..USER_DATA_OFFSET + changed.byte_count]
                    == changed.expected_source_payload,
                "source payload changed while sealing LBA {} for {}",
                changed.lba,
                record.path
            );

            let mut replacement = source;
            replacement[USER_DATA_OFFSET..USER_DATA_OFFSET + changed.byte_count].copy_from_slice(
                &record.data[changed.payload_offset..changed.payload_offset + changed.byte_count],
            );
            mode2::regenerate(&mut replacement)?;
            seal_sector_write(record, changed, &source, &replacement)?;
            candidates.push(SealedSectorCandidate {
                lba: changed.lba,
                owner: record.owner.clone(),
                path: record.path.clone(),
                source: Some(source),
                replacement,
            });
        }
    }
    candidates.sort_by_key(|candidate| candidate.lba);
    Ok(candidates)
}

fn seal_sector_write(
    record: &SealedDiscRecord<'_>,
    changed: &ChangedRecordSector,
    source: &[u8; RAW_SECTOR_SIZE],
    replacement: &[u8; RAW_SECTOR_SIZE],
) -> Result<()> {
    let lba = changed.lba;
    let payload_end = USER_DATA_OFFSET + changed.byte_count;
    let payload_region = USER_DATA_OFFSET..payload_end;
    let protection_region = MODE2_FORM1_PROTECTION_OFFSET..RAW_SECTOR_SIZE;
    let mut plan = WritePlan::new()
        .region(ImageRegion {
            id: format!("{}:{lba}:header-region", record.path),
            range: 0..USER_DATA_OFFSET,
            kind: RegionKind::Protected,
            reason: "raw-sector sync, address, mode, and duplicated subheaders".to_string(),
        })
        .region(ImageRegion {
            id: format!("{}:{lba}:payload-region", record.path),
            range: payload_region.clone(),
            kind: RegionKind::Data,
            reason: format!("ISO record payload owned by {}", record.owner),
        })
        .region(ImageRegion {
            id: format!("{}:{lba}:protection-region", record.path),
            range: protection_region.clone(),
            kind: RegionKind::Metadata,
            reason: "Mode 2/Form 1 EDC/ECC protection fields".to_string(),
        });
    if payload_end < MODE2_FORM1_PROTECTION_OFFSET {
        plan = plan.region(ImageRegion {
            id: format!("{}:{lba}:payload-tail-region", record.path),
            range: payload_end..MODE2_FORM1_PROTECTION_OFFSET,
            kind: RegionKind::Protected,
            reason: format!("bytes outside the logical extent of {}", record.path),
        });
    }
    plan = plan
        .write(ExpectedWrite {
            id: format!("{}:{lba}:payload", record.path),
            owner: record.owner.clone(),
            purpose: format!("replace ISO record payload for {}", record.path),
            offset: payload_region.start,
            expected_original: source[payload_region.clone()].to_vec(),
            replacement: replacement[payload_region].to_vec(),
            intent: WriteIntent::Data,
        })
        .write(ExpectedWrite {
            id: format!("{}:{lba}:protection", record.path),
            owner: "Mode 2/Form 1 sector finalizer".to_string(),
            purpose: format!("regenerate EDC/ECC after replacing {}", record.path),
            offset: protection_region.start,
            expected_original: source[protection_region.clone()].to_vec(),
            replacement: replacement[protection_region].to_vec(),
            intent: WriteIntent::Metadata,
        });
    let applied = plan
        .apply(source, None)
        .with_context(|| format!("failed to seal raw-sector write for LBA {lba}"))?;
    ensure!(
        applied.as_slice() == replacement,
        "sealed write plan did not reproduce raw sector {lba}"
    );
    Ok(())
}

fn write_staged_image(
    output_bin: &Path,
    source_track: &mut RawTrack,
    candidates: &[SealedSectorCandidate],
) -> Result<()> {
    let mut output = OpenOptions::new()
        .create(true)
        .truncate(true)
        .read(true)
        .write(true)
        .open(output_bin)
        .with_context(|| {
            format!(
                "failed to create staged output BIN: {}",
                output_bin.display()
            )
        })?;
    let copied = source_track.copy_bytes_to(&mut output).with_context(|| {
        format!(
            "failed to copy source BIN to staged output: {}",
            output_bin.display()
        )
    })?;
    ensure!(
        copied == source_track.sector_count() * RAW_SECTOR_SIZE as u64,
        "source BIN copy returned an incomplete byte count"
    );
    for candidate in candidates {
        output.seek(SeekFrom::Start(
            u64::from(candidate.lba) * RAW_SECTOR_SIZE as u64,
        ))?;
        output.write_all(&candidate.replacement)?;
    }
    output.flush()?;
    output.sync_all()?;
    Ok(())
}

fn verify_record_readback(
    output_bin: &Path,
    records: &[SealedDiscRecord<'_>],
    candidates: &[SealedSectorCandidate],
) -> Result<()> {
    let mut rebuilt_track = RawTrack::open(output_bin)?;
    {
        let mut iso = Iso9660::open(&mut rebuilt_track)?;
        for record in records {
            let rebuilt_record = iso.find(&record.path)?;
            let rebuilt_data = iso.read_record(&rebuilt_record)?;
            ensure!(
                rebuilt_data == record.data,
                "rebuilt ISO record differs from replacement for {}",
                record.path
            );
            ensure!(
                rebuilt_record.extent_lba == record.record.extent_lba
                    && rebuilt_record.size == record.record.size,
                "ISO directory record changed for {}",
                record.path
            );
        }
    }
    for candidate in candidates {
        let readback = rebuilt_track.raw_sector(candidate.lba)?;
        ensure!(
            readback == candidate.replacement,
            "rebuilt raw sector {} for {} ({}) differs from its sealed candidate",
            candidate.lba,
            candidate.path,
            candidate.owner
        );
        ensure!(
            mode2::verify(&readback),
            "rebuilt sector {} for {} failed Mode 2/Form 1 readback verification",
            candidate.lba,
            candidate.path
        );
    }
    Ok(())
}

fn audit_staged_image(
    source_bin: &Path,
    output_bin: &Path,
    candidates: &[SealedSectorCandidate],
) -> Result<DiscImageAudit> {
    let candidates_by_lba = candidates
        .iter()
        .map(|candidate| (candidate.lba, candidate))
        .collect::<BTreeMap<_, _>>();
    ensure!(
        candidates_by_lba.len() == candidates.len(),
        "sealed sector candidates contain duplicate LBAs"
    );

    let mut source = File::open(source_bin)?;
    let mut output = File::open(output_bin)?;
    let mut observed = BTreeSet::new();
    let mut changed_lbas = Vec::new();
    let mut output_digest = Sha256::new();
    let mut lba = 0u32;
    loop {
        let mut source_sector = [0u8; RAW_SECTOR_SIZE];
        let mut output_sector = [0u8; RAW_SECTOR_SIZE];
        let source_count = read_sector_or_eof(&mut source, &mut source_sector)?;
        let output_count = read_sector_or_eof(&mut output, &mut output_sector)?;
        if source_count == 0 && output_count == 0 {
            break;
        }
        ensure!(
            output_count == RAW_SECTOR_SIZE
                && (source_count == 0 || source_count == RAW_SECTOR_SIZE),
            "source or staged image contains a partial sector"
        );
        output_digest.update(output_sector);
        if let Some(candidate) = candidates_by_lba.get(&lba) {
            if let Some(expected) = candidate.source {
                ensure!(
                    source_count == RAW_SECTOR_SIZE && source_sector == expected,
                    "source sector {lba} changed after its candidate was sealed"
                );
            } else {
                ensure!(
                    source_count == 0,
                    "appended sector {lba} overwrites original data"
                );
            }
            ensure!(
                output_sector == candidate.replacement,
                "staged sector {lba} differs from its sealed candidate"
            );
            ensure!(
                candidate.source.is_none() || output_sector != source_sector,
                "sealed sector candidate {lba} changes no source bytes"
            );
            observed.insert(lba);
            changed_lbas.push(lba);
        } else {
            ensure!(
                source_count == RAW_SECTOR_SIZE && output_sector == source_sector,
                "staged image has an unplanned change at LBA {lba}"
            );
        }
        lba = lba
            .checked_add(1)
            .context("raw-sector count exceeds the supported LBA range")?;
    }
    ensure!(
        observed.len() == candidates.len(),
        "staged image did not contain every sealed sector candidate"
    );
    Ok(DiscImageAudit {
        changed_lbas,
        output_sha256: format!("{:x}", output_digest.finalize()),
    })
}

fn read_sector_or_eof(file: &mut File, output: &mut [u8]) -> Result<usize> {
    let mut total = 0usize;
    while total < output.len() {
        let count = file.read(&mut output[total..])?;
        if count == 0 {
            break;
        }
        total += count;
    }
    Ok(total)
}
