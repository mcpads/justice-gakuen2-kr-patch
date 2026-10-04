use std::collections::BTreeSet;
use std::fs::{File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{
    DiscRecordContribution, DiscRecordPlan, DiscRecordSourceIdentity, FixedRecordReplacement,
    compare_and_hash_images, copy_and_replace_records, read_record,
};
use crate::disc::{RAW_SECTOR_SIZE, SYNC, USER_DATA_OFFSET, USER_DATA_SIZE, mode2};

const PVD_LBA: u32 = 16;
const ROOT_LBA: u32 = 20;
const FILE_LBA: u32 = 21;
const DISTINCT_ALIAS_LBA: u32 = 23;
const TRACK_SECTOR_COUNT: usize = 24;
const TARGET_PATH: &str = "FILE.BIN";
const ALIAS_PATH: &str = "ALIAS.BIN";
const EDC_START: usize = 0x818;

static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

#[test]
fn appended_record_plan_preserves_old_extent_and_rebinds_iso_and_native_views() {
    let source_data = patterned_bytes(3000);
    let fixture = TinyMode2Iso::new_with_extent_alias(&source_data);
    let source_image = std::fs::read(&fixture.source).unwrap();
    let replacement = patterned_bytes(4097);
    let native =
        crate::native_disc_resource::NativeDiscResource::replacement(FILE_LBA, &source_data)
            .unwrap();
    let mut track = crate::disc::RawTrack::open(&fixture.source).unwrap();
    let plan = crate::disc::record_relocation::DiscRecordRelocation::prepare(
        &mut track,
        TARGET_PATH,
        &DiscRecordSourceIdentity::from_bytes(&source_data),
        &replacement,
        &native,
    )
    .unwrap();
    assert_eq!(
        (plan.source_sector_count, plan.output_sector_count),
        (24, 27)
    );
    assert_eq!(plan.output_record.extent_lba, 24);
    assert_eq!(plan.appended_sectors[0][12..15], [0, 2, 0x24]);
    assert_eq!(plan.appended_sectors[2][12..15], [0, 2, 0x26]);
    assert!(
        plan.appended_sectors[2][USER_DATA_OFFSET + 1..USER_DATA_OFFSET + USER_DATA_SIZE]
            .iter()
            .all(|&v| v == 0)
    );
    let rebound =
        crate::native_disc_resource::NativeDiscResource::parse(&plan.native_entry).unwrap();
    assert_eq!((rebound.lba, rebound.byte_count), (24, 4097));
    assert_eq!(rebound.prefix, replacement[..4]);
    let mut image = source_image.clone();
    for metadata in &plan.metadata {
        let start = metadata.lba as usize * RAW_SECTOR_SIZE;
        assert_eq!(image[start..start + RAW_SECTOR_SIZE], metadata.original);
        assert!(mode2::verify(&metadata.replacement));
        image[start..start + RAW_SECTOR_SIZE].copy_from_slice(&metadata.replacement);
    }
    for sector in &plan.appended_sectors {
        assert!(mode2::verify(sector));
        image.extend_from_slice(sector);
    }
    std::fs::write(&fixture.output, &image).unwrap();
    assert_eq!(
        read_record(&fixture.output, TARGET_PATH).unwrap().1,
        replacement
    );
    assert_eq!(
        read_record(&fixture.output, ALIAS_PATH).unwrap().1,
        source_data
    );
    for lba in 0..TRACK_SECTOR_COUNT {
        if ![PVD_LBA as usize, ROOT_LBA as usize].contains(&lba) {
            let range = lba * RAW_SECTOR_SIZE..(lba + 1) * RAW_SECTOR_SIZE;
            assert_eq!(image[range.clone()], source_image[range]);
        }
    }
    assert_eq!(std::fs::read(&fixture.source).unwrap(), source_image);
    let pvd = read_raw_sector(&fixture.output, PVD_LBA);
    assert_eq!(
        pvd[USER_DATA_OFFSET + 80..USER_DATA_OFFSET + 88],
        [27, 0, 0, 0, 0, 0, 0, 27]
    );
}

#[test]
fn relocation_rejects_mismatched_source_or_native_metadata_without_writes() {
    let source = patterned_bytes(100);
    let fixture = TinyMode2Iso::new(&source);
    let image = std::fs::read(&fixture.source).unwrap();
    let mut track = crate::disc::RawTrack::open(&fixture.source).unwrap();
    let native =
        crate::native_disc_resource::NativeDiscResource::replacement(FILE_LBA, &source).unwrap();
    for index in [0, 4, 8] {
        let mut bad = native;
        bad[index] ^= 1;
        assert!(
            crate::disc::record_relocation::DiscRecordRelocation::prepare(
                &mut track,
                TARGET_PATH,
                &DiscRecordSourceIdentity::from_bytes(&source),
                &[1, 2, 3, 4],
                &bad
            )
            .is_err()
        );
    }
    assert!(
        crate::disc::record_relocation::DiscRecordRelocation::prepare(
            &mut track,
            TARGET_PATH,
            &DiscRecordSourceIdentity::from_bytes(&[0; 100]),
            &[1, 2, 3, 4],
            &native
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&fixture.source).unwrap(), image);
    assert!(!fixture.output.exists());
}

#[test]
fn directory_location_is_metadata_not_payload_and_distinguishes_aliases() {
    let fixture = TinyMode2Iso::new_with_extent_alias(&[1, 2, 3, 4]);
    let mut track = crate::disc::RawTrack::open(&fixture.source).unwrap();
    let mut iso = crate::disc::iso9660::Iso9660::open(&mut track).unwrap();
    let file = iso.locate(TARGET_PATH).unwrap();
    let alias = iso.locate(ALIAS_PATH).unwrap();
    assert_eq!(
        (file.directory_lba, file.offset_in_sector, file.byte_count),
        (ROOT_LBA, 68, 44)
    );
    assert_eq!(alias.directory_lba, ROOT_LBA);
    assert_eq!(alias.offset_in_sector, 112);
    assert_eq!(file.record.extent_lba, FILE_LBA);
    assert_eq!(alias.record.extent_lba, FILE_LBA);
    assert_eq!(iso.find(TARGET_PATH).unwrap(), file.record);
    assert!(iso.locate("MISSING.BIN").is_err());
}

#[test]
#[ignore = "requires private original disc"]
fn name_font_directory_location_matches_independent_source_measurement() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).bin");
    let mut track = crate::disc::RawTrack::open(&path).unwrap();
    let mut iso = crate::disc::iso9660::Iso9660::open(&mut track).unwrap();
    let location = iso.locate("DAT2/MA_ENT.BIZ").unwrap();
    assert_eq!(
        (
            location.directory_lba,
            location.offset_in_sector,
            location.byte_count
        ),
        (836, 1018, 60)
    );
    assert_eq!(
        (location.record.extent_lba, location.record.size),
        (28248, 109762)
    );
    let records = iso.files().unwrap();
    assert_eq!(records.len(), 947);
    for (path, record) in records {
        assert_eq!(iso.locate(&path).unwrap().record, record);
    }
}

#[test]
fn unchanged_payload_sector_preserves_source_raw_bytes() {
    let source_data = patterned_bytes(USER_DATA_SIZE * 2);
    let fixture = TinyMode2Iso::new(&source_data);
    fixture.corrupt_edc(FILE_LBA + 1);
    let unchanged_source_sector = read_raw_sector(&fixture.source, FILE_LBA + 1);
    assert!(!mode2::verify(&unchanged_source_sector));

    let mut replacement = source_data;
    replacement[31] ^= 0x5a;
    let rebuilt = copy_and_replace_records(
        &fixture.source,
        &fixture.output,
        &[FixedRecordReplacement {
            path: TARGET_PATH,
            data: &replacement,
        }],
    )
    .unwrap();

    assert_eq!(rebuilt[0].target_lbas, vec![FILE_LBA, FILE_LBA + 1]);
    assert_eq!(rebuilt[0].changed_lbas, vec![FILE_LBA]);
    assert!(mode2::verify(&read_raw_sector(&fixture.output, FILE_LBA)));
    assert_eq!(
        read_raw_sector(&fixture.output, FILE_LBA + 1),
        unchanged_source_sector
    );
    assert_eq!(
        read_record(&fixture.output, TARGET_PATH).unwrap().1,
        replacement
    );
    assert_planned_changes_match_image_diff(&fixture, &rebuilt);
}

#[test]
fn read_only_source_produces_a_writable_rebuilt_image() {
    let source_data = patterned_bytes(USER_DATA_SIZE);
    let fixture = TinyMode2Iso::new(&source_data);
    let source_identity = DiscRecordSourceIdentity::from_bytes(&source_data);
    let original_permissions = std::fs::metadata(&fixture.source).unwrap().permissions();
    let mut read_only_permissions = original_permissions.clone();
    read_only_permissions.set_readonly(true);
    std::fs::set_permissions(&fixture.source, read_only_permissions).unwrap();

    let mut replacement = source_data;
    replacement[7] ^= 0x5a;
    let mut plan = DiscRecordPlan::new();
    plan.register(DiscRecordContribution {
        owner: "fixture producer".to_string(),
        path: TARGET_PATH,
        data: &replacement,
        source: source_identity,
    })
    .unwrap();
    let finalized = plan.finalize(&fixture.source, &fixture.output).unwrap();

    assert_eq!(finalized.changed_lbas, vec![FILE_LBA]);
    assert_eq!(finalized.records[0].owner, "fixture producer");
    assert_eq!(finalized.records[0].changed_lbas, vec![FILE_LBA]);
    assert_eq!(
        finalized.output_sha256,
        crate::pipeline::sha256_file(&fixture.output).unwrap()
    );
    OpenOptions::new()
        .write(true)
        .open(&fixture.output)
        .unwrap();
    std::fs::set_permissions(&fixture.source, original_permissions).unwrap();
}

#[test]
fn changed_payload_sector_with_invalid_source_protection_is_rejected_before_output() {
    let source_data = patterned_bytes(USER_DATA_SIZE);
    let fixture = TinyMode2Iso::new(&source_data);
    fixture.corrupt_edc(FILE_LBA);
    let sentinel = b"existing staged output";
    std::fs::write(&fixture.output, sentinel).unwrap();

    let mut replacement = source_data;
    replacement[0] ^= 0x80;
    let error = copy_and_replace_records(
        &fixture.source,
        &fixture.output,
        &[FixedRecordReplacement {
            path: TARGET_PATH,
            data: &replacement,
        }],
    )
    .unwrap_err();

    assert!(
        error
            .to_string()
            .contains("is not a valid Mode 2/Form 1 sector")
    );
    assert_eq!(std::fs::read(&fixture.output).unwrap(), sentinel);
}

#[test]
fn partial_final_sector_preserves_payload_bytes_outside_the_record() {
    let source_data = patterned_bytes(USER_DATA_SIZE + 17);
    let fixture = TinyMode2Iso::new(&source_data);
    let source_first_sector = read_raw_sector(&fixture.source, FILE_LBA);
    let source_final_sector = read_raw_sector(&fixture.source, FILE_LBA + 1);

    let mut replacement = source_data;
    replacement[USER_DATA_SIZE + 3] ^= 0x33;
    let rebuilt = copy_and_replace_records(
        &fixture.source,
        &fixture.output,
        &[FixedRecordReplacement {
            path: TARGET_PATH,
            data: &replacement,
        }],
    )
    .unwrap();

    let output_final_sector = read_raw_sector(&fixture.output, FILE_LBA + 1);
    assert_eq!(rebuilt[0].changed_lbas, vec![FILE_LBA + 1]);
    assert_eq!(
        read_raw_sector(&fixture.output, FILE_LBA),
        source_first_sector
    );
    assert_eq!(
        &output_final_sector[..USER_DATA_OFFSET],
        &source_final_sector[..USER_DATA_OFFSET]
    );
    assert_eq!(
        &output_final_sector[USER_DATA_OFFSET + 17..USER_DATA_OFFSET + USER_DATA_SIZE],
        &source_final_sector[USER_DATA_OFFSET + 17..USER_DATA_OFFSET + USER_DATA_SIZE]
    );
    assert!(mode2::verify(&output_final_sector));
    assert_eq!(
        read_record(&fixture.output, TARGET_PATH).unwrap().1,
        replacement
    );
    assert_planned_changes_match_image_diff(&fixture, &rebuilt);
}

#[test]
fn duplicate_record_path_reports_both_owners_before_output() {
    let source_data = patterned_bytes(USER_DATA_SIZE * 2);
    let fixture = TinyMode2Iso::new(&source_data);
    let source_identity = DiscRecordSourceIdentity::from_bytes(&source_data);
    let mut first_sector_replacement = source_data.clone();
    first_sector_replacement[0] ^= 1;
    let mut second_sector_replacement = source_data.clone();
    second_sector_replacement[USER_DATA_SIZE] ^= 1;

    let mut plan = DiscRecordPlan::new();
    plan.register(DiscRecordContribution {
        owner: "first producer".to_string(),
        path: TARGET_PATH,
        data: &first_sector_replacement,
        source: source_identity.clone(),
    })
    .unwrap();
    let error = plan
        .register(DiscRecordContribution {
            owner: "second producer".to_string(),
            path: TARGET_PATH,
            data: &second_sector_replacement,
            source: source_identity,
        })
        .unwrap_err();

    let message = error.to_string();
    assert!(message.contains(TARGET_PATH));
    assert!(message.contains("first producer"));
    assert!(message.contains("second producer"));
    assert!(!fixture.output.exists());
}

#[test]
fn distinct_record_paths_with_the_same_extent_are_rejected_before_output() {
    let source_data = patterned_bytes(USER_DATA_SIZE);
    let fixture = TinyMode2Iso::new_with_extent_alias(&source_data);
    let source_identity = DiscRecordSourceIdentity::from_bytes(&source_data);
    let mut first_replacement = source_data.clone();
    first_replacement[1] ^= 1;
    let mut alias_replacement = source_data;
    alias_replacement[2] ^= 1;

    let mut plan = DiscRecordPlan::new();
    plan.register(DiscRecordContribution {
        owner: "primary producer".to_string(),
        path: TARGET_PATH,
        data: &first_replacement,
        source: source_identity.clone(),
    })
    .unwrap();
    plan.register(DiscRecordContribution {
        owner: "alias producer".to_string(),
        path: ALIAS_PATH,
        data: &alias_replacement,
        source: source_identity,
    })
    .unwrap();
    let error = plan.finalize(&fixture.source, &fixture.output).unwrap_err();

    let message = error.to_string();
    assert!(message.contains("extents overlap"));
    assert!(message.contains(TARGET_PATH));
    assert!(message.contains(ALIAS_PATH));
    assert!(!fixture.output.exists());
}

#[test]
fn source_identity_from_another_same_size_path_preserves_existing_staged_output() {
    let source_data = patterned_bytes(USER_DATA_SIZE);
    let mut alias_data = source_data.clone();
    alias_data[4] ^= 1;
    let fixture = TinyMode2Iso::new_with_distinct_alias(&source_data, &alias_data);
    let sentinel = b"existing staged output";
    std::fs::write(&fixture.output, sentinel).unwrap();
    let mut replacement = alias_data;
    replacement[3] ^= 1;

    let mut plan = DiscRecordPlan::new();
    plan.register(DiscRecordContribution {
        owner: "source-bound producer".to_string(),
        path: ALIAS_PATH,
        data: &replacement,
        source: DiscRecordSourceIdentity::from_bytes(&source_data),
    })
    .unwrap();
    let error = plan.finalize(&fixture.source, &fixture.output).unwrap_err();

    assert!(error.to_string().contains("declared source identity"));
    assert_eq!(std::fs::read(&fixture.output).unwrap(), sentinel);
}

#[cfg(unix)]
#[test]
fn existing_symlink_and_hard_link_outputs_cannot_alias_the_source() {
    for hard_link in [false, true] {
        let source_data = patterned_bytes(USER_DATA_SIZE);
        let fixture = TinyMode2Iso::new(&source_data);
        let source_before = std::fs::read(&fixture.source).unwrap();
        if hard_link {
            std::fs::hard_link(&fixture.source, &fixture.output).unwrap();
        } else {
            std::os::unix::fs::symlink(&fixture.source, &fixture.output).unwrap();
        }
        let mut replacement = source_data.clone();
        replacement[5] ^= 1;
        let mut plan = DiscRecordPlan::new();
        plan.register(DiscRecordContribution {
            owner: "alias-safe producer".to_string(),
            path: TARGET_PATH,
            data: &replacement,
            source: DiscRecordSourceIdentity::from_bytes(&source_data),
        })
        .unwrap();

        let error = plan.finalize(&fixture.source, &fixture.output).unwrap_err();
        assert!(error.to_string().contains("source BIN"));
        assert_eq!(std::fs::read(&fixture.source).unwrap(), source_before);
    }
}

#[test]
fn changed_mode2_form2_sector_is_rejected_before_output() {
    let source_data = patterned_bytes(USER_DATA_SIZE);
    let fixture = TinyMode2Iso::new(&source_data);
    fixture.mark_form2(FILE_LBA);
    let sentinel = b"existing staged output";
    std::fs::write(&fixture.output, sentinel).unwrap();
    let mut replacement = source_data;
    replacement[0] ^= 1;

    let error = copy_and_replace_records(
        &fixture.source,
        &fixture.output,
        &[FixedRecordReplacement {
            path: TARGET_PATH,
            data: &replacement,
        }],
    )
    .unwrap_err();

    assert!(error.to_string().contains("Mode 2/Form 1"));
    assert_eq!(std::fs::read(&fixture.output).unwrap(), sentinel);
}

fn assert_planned_changes_match_image_diff(
    fixture: &TinyMode2Iso,
    rebuilt: &[super::RecordRebuild],
) {
    let planned = rebuilt
        .iter()
        .flat_map(|record| record.changed_lbas.iter().copied())
        .collect::<BTreeSet<_>>();
    let (observed, _) = compare_and_hash_images(&fixture.source, &fixture.output).unwrap();
    assert_eq!(observed.into_iter().collect::<BTreeSet<_>>(), planned);
}

struct TinyMode2Iso {
    directory: PathBuf,
    source: PathBuf,
    output: PathBuf,
}

impl TinyMode2Iso {
    fn new(file_data: &[u8]) -> Self {
        Self::from_file_data(file_data, None)
    }

    fn new_with_extent_alias(file_data: &[u8]) -> Self {
        Self::from_file_data(file_data, Some((FILE_LBA, file_data)))
    }

    fn new_with_distinct_alias(file_data: &[u8], alias_data: &[u8]) -> Self {
        assert_eq!(alias_data.len(), file_data.len());
        assert!(alias_data.len() <= USER_DATA_SIZE);
        Self::from_file_data(file_data, Some((DISTINCT_ALIAS_LBA, alias_data)))
    }

    fn from_file_data(file_data: &[u8], alias: Option<(u32, &[u8])>) -> Self {
        assert!(!file_data.is_empty());
        assert!(file_data.len() <= USER_DATA_SIZE * 2);
        let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let directory = std::env::temp_dir().join(format!(
            "justice-gakuen2-disc-rebuild-{}-{sequence}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir(&directory).unwrap();
        let source = directory.join("source.bin");
        let output = directory.join("output.bin");

        let mut payloads = vec![[0u8; USER_DATA_SIZE]; TRACK_SECTOR_COUNT];
        write_primary_volume_descriptor(&mut payloads[PVD_LBA as usize]);
        payloads[17][..7].copy_from_slice(&[255, b'C', b'D', b'0', b'0', b'1', 1]);
        write_root_directory(
            &mut payloads[ROOT_LBA as usize],
            file_data.len() as u32,
            alias.map(|(lba, data)| (lba, data.len() as u32)),
        );
        for (index, chunk) in file_data.chunks(USER_DATA_SIZE).enumerate() {
            payloads[FILE_LBA as usize + index][..chunk.len()].copy_from_slice(chunk);
            for (tail_index, byte) in payloads[FILE_LBA as usize + index][chunk.len()..]
                .iter_mut()
                .enumerate()
            {
                *byte = 0xa5 ^ tail_index as u8;
            }
        }
        if let Some((alias_lba, alias_data)) = alias
            && alias_lba != FILE_LBA
        {
            payloads[alias_lba as usize][..alias_data.len()].copy_from_slice(alias_data);
        }

        let mut image = Vec::with_capacity(TRACK_SECTOR_COUNT * RAW_SECTOR_SIZE);
        for payload in payloads {
            image.extend_from_slice(&mode2_form1_sector(&payload));
        }
        std::fs::write(&source, image).unwrap();
        Self {
            directory,
            source,
            output,
        }
    }

    fn corrupt_edc(&self, lba: u32) {
        let mut raw = read_raw_sector(&self.source, lba);
        raw[EDC_START] ^= 0x01;
        write_raw_sector(&self.source, lba, &raw);
    }

    fn mark_form2(&self, lba: u32) {
        let mut raw = read_raw_sector(&self.source, lba);
        raw[0x12] |= 0x20;
        raw[0x16] |= 0x20;
        write_raw_sector(&self.source, lba, &raw);
    }
}

impl Drop for TinyMode2Iso {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

fn write_primary_volume_descriptor(payload: &mut [u8; USER_DATA_SIZE]) {
    payload[0] = 1;
    payload[1..6].copy_from_slice(b"CD001");
    payload[6] = 1;
    write_u32_both(payload, 80, TRACK_SECTOR_COUNT as u32);
    write_u16_both(payload, 128, USER_DATA_SIZE as u16);
    let root = directory_record(ROOT_LBA, USER_DATA_SIZE as u32, 0x02, &[0]);
    payload[156..156 + root.len()].copy_from_slice(&root);
}

fn write_root_directory(
    payload: &mut [u8; USER_DATA_SIZE],
    file_size: u32,
    alias: Option<(u32, u32)>,
) {
    let root = directory_record(ROOT_LBA, USER_DATA_SIZE as u32, 0x02, &[0]);
    let parent = directory_record(ROOT_LBA, USER_DATA_SIZE as u32, 0x02, &[1]);
    let file = directory_record(FILE_LBA, file_size, 0, b"FILE.BIN;1");
    let mut offset = 0usize;
    let alias =
        alias.map(|(extent_lba, size)| directory_record(extent_lba, size, 0, b"ALIAS.BIN;1"));
    for record in [&root, &parent, &file].into_iter().chain(alias.as_ref()) {
        payload[offset..offset + record.len()].copy_from_slice(record);
        offset += record.len();
    }
}

fn directory_record(extent_lba: u32, size: u32, flags: u8, name: &[u8]) -> Vec<u8> {
    let padding = usize::from(name.len().is_multiple_of(2));
    let length = 33 + name.len() + padding;
    let mut record = vec![0u8; length];
    record[0] = length as u8;
    write_u32_both(&mut record, 2, extent_lba);
    write_u32_both(&mut record, 10, size);
    record[25] = flags;
    record[28..30].copy_from_slice(&1u16.to_le_bytes());
    record[30..32].copy_from_slice(&1u16.to_be_bytes());
    record[32] = name.len() as u8;
    record[33..33 + name.len()].copy_from_slice(name);
    record
}

fn write_u16_both(output: &mut [u8], offset: usize, value: u16) {
    output[offset..offset + 2].copy_from_slice(&value.to_le_bytes());
    output[offset + 2..offset + 4].copy_from_slice(&value.to_be_bytes());
}

fn write_u32_both(output: &mut [u8], offset: usize, value: u32) {
    output[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    output[offset + 4..offset + 8].copy_from_slice(&value.to_be_bytes());
}

fn mode2_form1_sector(payload: &[u8; USER_DATA_SIZE]) -> [u8; RAW_SECTOR_SIZE] {
    let mut raw = [0u8; RAW_SECTOR_SIZE];
    raw[..SYNC.len()].copy_from_slice(&SYNC);
    raw[0x0f] = 2;
    raw[0x10..0x14].copy_from_slice(&[0, 0, 0x08, 0]);
    raw[0x14..0x18].copy_from_slice(&[0, 0, 0x08, 0]);
    raw[USER_DATA_OFFSET..USER_DATA_OFFSET + USER_DATA_SIZE].copy_from_slice(payload);
    mode2::regenerate(&mut raw).unwrap();
    raw
}

fn read_raw_sector(path: &Path, lba: u32) -> [u8; RAW_SECTOR_SIZE] {
    let mut file = File::open(path).unwrap();
    file.seek(SeekFrom::Start(u64::from(lba) * RAW_SECTOR_SIZE as u64))
        .unwrap();
    let mut raw = [0u8; RAW_SECTOR_SIZE];
    file.read_exact(&mut raw).unwrap();
    raw
}

fn write_raw_sector(path: &Path, lba: u32, raw: &[u8; RAW_SECTOR_SIZE]) {
    let mut file = OpenOptions::new().write(true).open(path).unwrap();
    file.seek(SeekFrom::Start(u64::from(lba) * RAW_SECTOR_SIZE as u64))
        .unwrap();
    file.write_all(raw).unwrap();
    file.flush().unwrap();
}

fn patterned_bytes(length: usize) -> Vec<u8> {
    (0..length)
        .map(|index| (index as u8).wrapping_mul(37).wrapping_add(11))
        .collect()
}

#[test]
#[ignore = "requires private source and current translated name font"]
fn compact_name_font_candidate_has_bound_appended_extent() -> anyhow::Result<()> {
    use crate::{
        compression::decompress,
        name_input::RosterNameFont,
        paged_compression::{compress_page_safe_image_in_slot, source_paged_compression_profile},
        pipeline::sha256_bytes,
    };
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = root.join("build/roster-font-verification");
    std::fs::create_dir_all(&output)?;
    let font_path = root.join("../fonts/galmuri/Galmuri11.ttf");
    let compact = RosterNameFont::build(&font_path, 12.)?;
    assert_eq!(
        compact.report.hangul.font_sha256,
        "2c709890595668f7bdb6df408420fda957dde0288e95b31a1cc17a2ab98b4b4f"
    );
    let ascii = &compact.ascii;
    let names =
        std::path::PathBuf::from(std::env::var("JUSTICE_NAME_INPUT_DIR").map_err(|_| {
            anyhow::anyhow!("set JUSTICE_NAME_INPUT_DIR to an explicit composed name-input build")
        })?);
    let stored = std::fs::read(names.join("MA_ENT.BIZ"))?;
    let mut decoded = decompress(&stored, true)?;
    anyhow::ensure!(decoded.len() >= 0x33800, "composed font prefix truncated");
    decoded.truncate(0x33800);
    let original_decoded_hash = sha256_bytes(&decoded);
    let pack_offset = decoded.len();
    compact.append_to(&mut decoded)?;
    let ascii_offset = compact.report.ascii_offset;
    let profile = source_paged_compression_profile(&stored)?;
    let replacement = compress_page_safe_image_in_slot(&decoded, profile, stored.len() + 65536)?;
    assert_eq!(decompress(&replacement, true)?, decoded);
    assert_eq!(sha256_bytes(&decoded[..pack_offset]), original_decoded_hash);
    let mut track = crate::disc::RawTrack::open(
        &root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).bin"),
    )?;
    let original = {
        let mut iso = crate::disc::iso9660::Iso9660::open(&mut track)?;
        let file = iso.find("DAT2/MA_ENT.BIZ")?;
        iso.read_record(&file)?
    };
    assert_eq!(
        sha256_bytes(&original),
        "31b003368f9c7189448cdaa98baa746dc8dbea1638183c46554f96f480bf21ce"
    );
    let fixed = compress_page_safe_image_in_slot(
        &decoded,
        source_paged_compression_profile(&original)?,
        original.len(),
    )?;
    assert!(fixed.len() <= original.len());
    assert_eq!(decompress(&fixed, true)?, decoded);
    eprintln!(
        "compact supplier fits original extent: {} <= {}",
        fixed.len(),
        original.len()
    );
    let entry = [0x06, 0x18, 0x48, 0, 0xc2, 0xac, 1, 0, 0, 0, 0x10, 0];
    let plan = crate::disc::record_relocation::DiscRecordRelocation::prepare(
        &mut track,
        "DAT2/MA_ENT.BIZ",
        &DiscRecordSourceIdentity::from_bytes(&original),
        &replacement,
        &entry,
    )?;
    let payload: Vec<u8> = plan
        .appended_sectors
        .iter()
        .flat_map(|s| {
            s[USER_DATA_OFFSET..USER_DATA_OFFSET + USER_DATA_SIZE]
                .iter()
                .copied()
        })
        .take(replacement.len())
        .collect();
    assert_eq!(payload, replacement);
    assert!(plan.appended_sectors.iter().all(mode2::verify));
    let report = serde_json::json!({"status":"component_candidate_not_installed","source_decoded_sha256":original_decoded_hash,"font":compact.report,"pack_offset":pack_offset,"ascii_offset":ascii_offset,"ascii_crop":ascii.crop,"ascii_bytes":ascii.bytes.len(),"decoded_bytes":decoded.len(),"stored_bytes":replacement.len(),"stored_sha256":sha256_bytes(&replacement),"extent_lba":plan.output_record.extent_lba,"output_sector_count":plan.output_sector_count,"native_entry":plan.native_entry,"metadata_lbas":plan.metadata.iter().map(|s|s.lba).collect::<Vec<_>>(),"native_runtime_verified":false});
    std::fs::write(output.join("MA_ENT-compact-candidate.BIZ"), replacement)?;
    std::fs::write(
        output.join("relocation-candidate.json"),
        serde_json::to_vec_pretty(&report)?,
    )?;
    eprintln!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}

#[test]
fn central_finalizer_appends_resource_with_composed_native_catalog() {
    let source = patterned_bytes(1000);
    let old_entry =
        crate::native_disc_resource::NativeDiscResource::replacement(FILE_LBA, &source).unwrap();
    let mut executable = vec![0; 1000];
    executable[32..44].copy_from_slice(&old_entry);
    let fixture = TinyMode2Iso::new_with_distinct_alias(&source, &executable);
    let replacement = patterned_bytes(4097);
    let mut composed = executable.clone();
    composed[32..44].copy_from_slice(
        &crate::native_disc_resource::NativeDiscResource::replacement(24, &replacement).unwrap(),
    );
    composed[128] = 0x42;
    let mut plan = DiscRecordPlan::new();
    plan.register_appended(
        DiscRecordContribution {
            owner: "compact font".into(),
            path: TARGET_PATH,
            data: &replacement,
            source: DiscRecordSourceIdentity::from_bytes(&source),
        },
        crate::disc::record_plan::NativeCatalogBinding {
            executable_path: ALIAS_PATH.into(),
            entry_offset: 32,
        },
    )
    .unwrap();
    plan.register(DiscRecordContribution {
        owner: "composed executable".into(),
        path: ALIAS_PATH,
        data: &composed,
        source: DiscRecordSourceIdentity::from_bytes(&executable),
    })
    .unwrap();
    let result = plan.finalize(&fixture.source, &fixture.output).unwrap();
    assert_eq!(
        read_record(&fixture.output, TARGET_PATH).unwrap().1,
        replacement
    );
    assert_eq!(
        read_record(&fixture.output, ALIAS_PATH).unwrap().1,
        composed
    );
    assert_eq!(result.changed_lbas, vec![16, 20, 23, 24, 25, 26]);
    assert_eq!(result.metadata_lbas, vec![16, 20]);
    assert_eq!(
        (result.source_sector_count, result.output_sector_count),
        (24, 27)
    );
    assert_eq!(
        result
            .records
            .iter()
            .find(|r| r.path == TARGET_PATH)
            .unwrap()
            .changed_lbas,
        vec![24, 25, 26]
    );
    assert_eq!(
        read_raw_sector(&fixture.source, 21),
        read_raw_sector(&fixture.output, 21)
    );
    assert_eq!(
        std::fs::metadata(&fixture.output).unwrap().len(),
        27 * RAW_SECTOR_SIZE as u64
    );
    use sha2::Digest;
    assert_eq!(
        result.output_sha256,
        format!(
            "{:x}",
            sha2::Sha256::digest(std::fs::read(&fixture.output).unwrap())
        )
    );
}

#[test]
fn missing_or_stale_native_catalog_rejects_before_output_mutation() {
    let source = patterned_bytes(1000);
    let mut executable = vec![0; 1000];
    executable[32..44].copy_from_slice(
        &crate::native_disc_resource::NativeDiscResource::replacement(FILE_LBA, &source).unwrap(),
    );
    let fixture = TinyMode2Iso::new_with_distinct_alias(&source, &executable);
    let replacement = patterned_bytes(4097);
    std::fs::write(
        &fixture.output,
        b"existing output must survive rejected planning",
    )
    .unwrap();
    for register_executable in [false, true] {
        let mut plan = DiscRecordPlan::new();
        plan.register_appended(
            DiscRecordContribution {
                owner: "font".into(),
                path: TARGET_PATH,
                data: &replacement,
                source: DiscRecordSourceIdentity::from_bytes(&source),
            },
            crate::disc::record_plan::NativeCatalogBinding {
                executable_path: ALIAS_PATH.into(),
                entry_offset: 32,
            },
        )
        .unwrap();
        if register_executable {
            plan.register(DiscRecordContribution {
                owner: "stale catalog".into(),
                path: ALIAS_PATH,
                data: &executable,
                source: DiscRecordSourceIdentity::from_bytes(&executable),
            })
            .unwrap();
        }
        assert!(plan.finalize(&fixture.source, &fixture.output).is_err());
        assert_eq!(
            std::fs::read(&fixture.output).unwrap(),
            b"existing output must survive rejected planning"
        );
    }
}

#[test]
fn relocated_metadata_cannot_overlap_a_registered_payload_owner() {
    let source = patterned_bytes(1000);
    let entry =
        crate::native_disc_resource::NativeDiscResource::replacement(FILE_LBA, &source).unwrap();
    let fixture = TinyMode2Iso::new_with_distinct_alias(&source, &vec![0; 1000]);
    let mut root = read_raw_sector(&fixture.source, ROOT_LBA);
    write_u32_both(&mut root, USER_DATA_OFFSET + 112 + 2, PVD_LBA);
    mode2::regenerate(&mut root).unwrap();
    write_raw_sector(&fixture.source, ROOT_LBA, &root);
    let mut pvd = read_raw_sector(&fixture.source, PVD_LBA);
    pvd[USER_DATA_OFFSET + 32..USER_DATA_OFFSET + 44].copy_from_slice(&entry);
    mode2::regenerate(&mut pvd).unwrap();
    write_raw_sector(&fixture.source, PVD_LBA, &pvd);
    let executable = read_record(&fixture.source, ALIAS_PATH).unwrap().1;
    let replacement = patterned_bytes(4097);
    let mut composed = executable.clone();
    composed[32..44].copy_from_slice(
        &crate::native_disc_resource::NativeDiscResource::replacement(24, &replacement).unwrap(),
    );
    let mut plan = DiscRecordPlan::new();
    plan.register_appended(
        DiscRecordContribution {
            owner: "font".into(),
            path: TARGET_PATH,
            data: &replacement,
            source: DiscRecordSourceIdentity::from_bytes(&source),
        },
        crate::disc::record_plan::NativeCatalogBinding {
            executable_path: ALIAS_PATH.into(),
            entry_offset: 32,
        },
    )
    .unwrap();
    plan.register(DiscRecordContribution {
        owner: "metadata alias".into(),
        path: ALIAS_PATH,
        data: &composed,
        source: DiscRecordSourceIdentity::from_bytes(&executable),
    })
    .unwrap();
    let error = plan.finalize(&fixture.source, &fixture.output).unwrap_err();
    assert!(
        error.to_string().contains("sector owners overlap"),
        "{error:#}"
    );
    assert!(!fixture.output.exists());
}
