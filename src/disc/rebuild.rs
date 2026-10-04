use std::fs::File;
use std::io::Read;
use std::path::Path;

use anyhow::{Result, ensure};
use sha2::{Digest, Sha256};

use super::iso9660::{FileRecord, Iso9660};
use super::{RAW_SECTOR_SIZE, RawTrack};

pub use super::record_plan::{
    DiscFinalization, DiscRecordContribution, DiscRecordPlan, DiscRecordSourceIdentity,
    FixedRecordReplacement, RecordRebuild,
};

pub fn read_record(source_bin: &Path, target_path: &str) -> Result<(FileRecord, Vec<u8>)> {
    let mut track = RawTrack::open(source_bin)?;
    let mut iso = Iso9660::open(&mut track)?;
    let record = iso.find(target_path)?;
    ensure!(
        record.extended_attribute_blocks == 0,
        "{target_path} uses unsupported extended attributes"
    );
    let data = iso.read_record(&record)?;
    Ok((record, data))
}

pub fn copy_and_replace_record(
    source_bin: &Path,
    output_bin: &Path,
    target_path: &str,
    replacement: &[u8],
) -> Result<RecordRebuild> {
    let replacements = [FixedRecordReplacement {
        path: target_path,
        data: replacement,
    }];
    let mut rebuilt = copy_and_replace_records(source_bin, output_bin, &replacements)?;
    ensure!(
        rebuilt.len() == 1,
        "single-record rebuild returned wrong count"
    );
    Ok(rebuilt.remove(0))
}

pub fn copy_and_replace_records<'a>(
    source_bin: &Path,
    output_bin: &Path,
    replacements: &[FixedRecordReplacement<'a>],
) -> Result<Vec<RecordRebuild>> {
    let mut plan = DiscRecordPlan::new();
    for replacement in replacements {
        plan.register_legacy(*replacement)?;
    }
    Ok(plan.finalize(source_bin, output_bin)?.records)
}

pub fn compare_and_hash_images(original: &Path, rebuilt: &Path) -> Result<(Vec<u32>, String)> {
    let mut left = File::open(original)?;
    let mut right = File::open(rebuilt)?;
    let mut changed_lbas = Vec::new();
    let mut digest = Sha256::new();
    let mut lba = 0u32;
    loop {
        let mut original_sector = [0u8; RAW_SECTOR_SIZE];
        let mut rebuilt_sector = [0u8; RAW_SECTOR_SIZE];
        let left_count = read_sector_or_eof(&mut left, &mut original_sector)?;
        let right_count = read_sector_or_eof(&mut right, &mut rebuilt_sector)?;
        ensure!(left_count == right_count, "rebuilt image length changed");
        if left_count == 0 {
            break;
        }
        ensure!(
            left_count == RAW_SECTOR_SIZE,
            "image contains a partial sector"
        );
        digest.update(rebuilt_sector);
        if original_sector != rebuilt_sector {
            changed_lbas.push(lba);
        }
        lba += 1;
    }
    Ok((changed_lbas, format!("{:x}", digest.finalize())))
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

#[cfg(test)]
#[path = "rebuild_tests.rs"]
mod tests;
