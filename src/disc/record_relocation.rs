//! Prepare an appended file extent and both of its metadata views without I/O mutation.
use super::{
    RAW_SECTOR_SIZE, RawTrack, USER_DATA_OFFSET, USER_DATA_SIZE,
    iso9660::{FileRecord, Iso9660},
    mode2,
    record_plan::DiscRecordSourceIdentity,
};
use crate::native_disc_resource::NativeDiscResource;
use anyhow::{Result, ensure};

#[derive(Debug)]
pub struct RelocationMetadataSector {
    pub lba: u32,
    pub fields: Vec<std::ops::Range<usize>>,
    pub original: [u8; RAW_SECTOR_SIZE],
    pub replacement: [u8; RAW_SECTOR_SIZE],
}

/// The final writer must compose the native entry with executable changes and
/// seal metadata/appended sectors with other owners before publishing any BIN.
#[derive(Debug)]
pub struct DiscRecordRelocation {
    pub path: String,
    pub source_sector_count: u32,
    pub output_sector_count: u32,
    pub source_record: FileRecord,
    pub output_record: FileRecord,
    pub native_entry: [u8; 12],
    pub metadata: Vec<RelocationMetadataSector>,
    pub appended_sectors: Vec<[u8; RAW_SECTOR_SIZE]>,
}

impl DiscRecordRelocation {
    pub fn prepare(
        track: &mut RawTrack,
        path: &str,
        source: &DiscRecordSourceIdentity,
        stored: &[u8],
        native_entry: &[u8],
    ) -> Result<Self> {
        let source_sector_count = u32::try_from(track.sector_count())?;
        let (location, original) = {
            let mut iso = Iso9660::open(track)?;
            let location = iso.locate(path)?;
            let original = iso.read_record(&location.record)?;
            (location, original)
        };
        ensure!(
            !location.record.is_directory() && location.record.extended_attribute_blocks == 0,
            "relocation requires an ordinary file without extended attributes"
        );
        ensure!(
            &DiscRecordSourceIdentity::from_bytes(&original) == source,
            "relocation source identity changed"
        );
        let native = NativeDiscResource::parse(native_entry)?;
        ensure!(
            native.lba == location.record.extent_lba
                && native.byte_count == location.record.size
                && original.get(..4) == Some(native.prefix.as_slice()),
            "native and ISO source views disagree"
        );
        let native_entry = NativeDiscResource::replacement(source_sector_count, stored)?;
        let output_sector_count = source_sector_count
            .checked_add(u32::try_from(stored.len().div_ceil(USER_DATA_SIZE))?)
            .ok_or_else(|| anyhow::anyhow!("relocated extent overflows"))?;
        // Also validates the final sector's representable CD position.
        NativeDiscResource::replacement(output_sector_count - 1, &[0; 4])?;
        let mut output_record = location.record.clone();
        output_record.extent_lba = source_sector_count;
        output_record.size = u32::try_from(stored.len())?;
        ensure!(
            location.directory_lba != 16,
            "file directory overlaps volume descriptor"
        );
        let mut metadata = Vec::new();
        let (terminator, _) = track.user_sector(17)?;
        ensure!(
            terminator[..7] == [255, b'C', b'D', b'0', b'0', b'1', 1],
            "relocation requires a single primary volume descriptor"
        );
        let pvd = track.raw_sector(16)?;
        ensure!(mode2::verify(&pvd), "invalid volume sector checksums");
        let at = USER_DATA_OFFSET + 80;
        let declared = u32::from_le_bytes(pvd[at..at + 4].try_into()?);
        ensure!(
            declared == u32::from_be_bytes(pvd[at + 4..at + 8].try_into()?)
                && declared > 0
                && declared <= source_sector_count,
            "invalid source volume size"
        );
        let mut replacement = pvd;
        write_both(&mut replacement, at, output_sector_count);
        mode2::regenerate(&mut replacement)?;
        metadata.push(RelocationMetadataSector {
            lba: 16,
            fields: vec![at..at + 8],
            original: pvd,
            replacement,
        });
        let directory = track.raw_sector(location.directory_lba)?;
        ensure!(
            mode2::verify(&directory),
            "invalid directory sector checksums"
        );
        let mut replacement = directory;
        let at = USER_DATA_OFFSET + location.offset_in_sector;
        ensure!(
            directory[at + 25] & 0x80 == 0 && directory[at + 26..at + 28] == [0, 0],
            "interleaved or multi-extent relocation is unsupported"
        );
        write_both(&mut replacement, at + 2, output_record.extent_lba);
        write_both(&mut replacement, at + 10, output_record.size);
        mode2::regenerate(&mut replacement)?;
        metadata.push(RelocationMetadataSector {
            lba: location.directory_lba,
            fields: vec![at + 2..at + 10, at + 10..at + 18],
            original: directory,
            replacement,
        });
        let first = track.raw_sector(location.record.extent_lba)?;
        let last_lba = location
            .record
            .extent_lba
            .checked_add(location.record.size.div_ceil(USER_DATA_SIZE as u32) - 1)
            .ok_or_else(|| anyhow::anyhow!("source extent overflows"))?;
        let last = track.raw_sector(last_lba)?;
        ensure!(
            mode2::verify(&first) && mode2::verify(&last),
            "invalid source file sector checksums"
        );
        let mut appended_sectors = Vec::new();
        let count = stored.len().div_ceil(USER_DATA_SIZE);
        for (index, chunk) in stored.chunks(USER_DATA_SIZE).enumerate() {
            let lba = source_sector_count + u32::try_from(index)?;
            let position = NativeDiscResource::replacement(lba, &[0; 4])?;
            let mut sector = first;
            sector[12..15].copy_from_slice(&position[..3]);
            sector[0x12] &= !0x81;
            if index + 1 == count {
                sector[0x12] |= last[0x12] & 0x81;
            }
            let subheader: [u8; 4] = sector[0x10..0x14].try_into()?;
            sector[0x14..0x18].copy_from_slice(&subheader);
            sector[USER_DATA_OFFSET..USER_DATA_OFFSET + USER_DATA_SIZE].fill(0);
            sector[USER_DATA_OFFSET..USER_DATA_OFFSET + chunk.len()].copy_from_slice(chunk);
            mode2::regenerate(&mut sector)?;
            appended_sectors.push(sector);
        }
        Ok(Self {
            path: path.into(),
            source_sector_count,
            output_sector_count,
            source_record: location.record,
            output_record,
            native_entry,
            metadata,
            appended_sectors,
        })
    }
}

fn write_both(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    bytes[offset + 4..offset + 8].copy_from_slice(&value.to_be_bytes());
}
