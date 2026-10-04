use anyhow::{Result, bail, ensure};

use super::{RawTrack, SectorKind, USER_DATA_SIZE};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRecord {
    pub extent_lba: u32,
    pub extended_attribute_blocks: u8,
    pub size: u32,
    pub flags: u8,
    pub name: String,
}

/// Physical directory metadata location, not the file's data extent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRecordLocation {
    pub record: FileRecord,
    pub directory_lba: u32,
    pub offset_in_sector: usize,
    pub byte_count: usize,
}

impl FileRecord {
    pub fn is_directory(&self) -> bool {
        self.flags & 0x02 != 0
    }
}

pub struct Iso9660<'a> {
    track: &'a mut RawTrack,
    block_size: usize,
    root: FileRecord,
}

impl<'a> Iso9660<'a> {
    pub fn open(track: &'a mut RawTrack) -> Result<Self> {
        let (pvd, kind) = track.user_sector(16)?;
        ensure!(kind == SectorKind::Mode2Form1, "PVD is not Mode 2 Form 1");
        ensure!(
            pvd[0] == 1 && &pvd[1..6] == b"CD001",
            "ISO 9660 primary volume descriptor not found"
        );
        let block_size = usize::from(u16_both(&pvd, 128)?);
        ensure!(block_size == USER_DATA_SIZE, "unsupported ISO block size");
        let root_len = usize::from(pvd[156]);
        let root = parse_record(&pvd[156..156 + root_len])?;
        Ok(Self {
            track,
            block_size,
            root,
        })
    }

    pub fn block_size(&self) -> usize {
        self.block_size
    }

    pub fn read_record(&mut self, record: &FileRecord) -> Result<Vec<u8>> {
        let lba = record.extent_lba + u32::from(record.extended_attribute_blocks);
        self.read_extent(lba, record.size as usize)
    }

    pub fn find(&mut self, target: &str) -> Result<FileRecord> {
        Ok(self.locate(target)?.record)
    }

    pub fn locate(&mut self, target: &str) -> Result<FileRecordLocation> {
        let root = self.root.clone();
        self.find_in(&root, "", target)?
            .ok_or_else(|| anyhow::anyhow!("ISO path not found: {target}"))
    }

    pub fn files(&mut self) -> Result<Vec<(String, FileRecord)>> {
        let root = self.root.clone();
        let mut output = Vec::new();
        self.collect_files(&root, "", &mut output)?;
        output.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(output)
    }

    fn find_in(
        &mut self,
        directory: &FileRecord,
        parent: &str,
        target: &str,
    ) -> Result<Option<FileRecordLocation>> {
        for location in self.located_directory_records(directory)? {
            let record = &location.record;
            if record.name == "." || record.name == ".." {
                continue;
            }
            let path = if parent.is_empty() {
                record.name.clone()
            } else {
                format!("{parent}/{}", record.name)
            };
            if path == target {
                return Ok(Some(location));
            }
            if record.is_directory()
                && let Some(found) = self.find_in(record, &path, target)?
            {
                return Ok(Some(found));
            }
        }
        Ok(None)
    }

    fn collect_files(
        &mut self,
        directory: &FileRecord,
        parent: &str,
        output: &mut Vec<(String, FileRecord)>,
    ) -> Result<()> {
        for record in self.directory_records(directory)? {
            if record.name == "." || record.name == ".." {
                continue;
            }
            let path = if parent.is_empty() {
                record.name.clone()
            } else {
                format!("{parent}/{}", record.name)
            };
            if record.is_directory() {
                self.collect_files(&record, &path, output)?;
            } else {
                output.push((path, record));
            }
        }
        Ok(())
    }

    fn directory_records(&mut self, directory: &FileRecord) -> Result<Vec<FileRecord>> {
        Ok(self
            .located_directory_records(directory)?
            .into_iter()
            .map(|location| location.record)
            .collect())
    }

    fn located_directory_records(
        &mut self,
        directory: &FileRecord,
    ) -> Result<Vec<FileRecordLocation>> {
        let data = self.read_record(directory)?;
        let mut records = Vec::new();
        let mut offset = 0usize;
        while offset < data.len() {
            let length = usize::from(data[offset]);
            if length == 0 {
                offset = (offset / self.block_size + 1) * self.block_size;
                continue;
            }
            ensure!(
                offset + length <= data.len(),
                "truncated ISO directory record"
            );
            let offset_in_sector = offset % self.block_size;
            ensure!(
                offset_in_sector + length <= self.block_size,
                "ISO directory record crosses a logical sector"
            );
            let relative_sector = u32::try_from(offset / self.block_size)?;
            let directory_lba = directory
                .extent_lba
                .checked_add(u32::from(directory.extended_attribute_blocks))
                .and_then(|lba| lba.checked_add(relative_sector))
                .ok_or_else(|| anyhow::anyhow!("ISO directory metadata address overflow"))?;
            records.push(FileRecordLocation {
                record: parse_record(&data[offset..offset + length])?,
                directory_lba,
                offset_in_sector,
                byte_count: length,
            });
            offset += length;
        }
        Ok(records)
    }

    fn read_extent(&mut self, mut lba: u32, mut size: usize) -> Result<Vec<u8>> {
        let mut output = Vec::with_capacity(size);
        while size > 0 {
            let (sector, _) = self.track.user_sector(lba)?;
            let take = size.min(self.block_size);
            ensure!(sector.len() >= take, "short logical sector at LBA {lba}");
            output.extend_from_slice(&sector[..take]);
            size -= take;
            lba += 1;
        }
        Ok(output)
    }
}

fn parse_record(raw: &[u8]) -> Result<FileRecord> {
    ensure!(raw.len() >= 34, "invalid ISO directory record length");
    let length = usize::from(raw[0]);
    ensure!(
        length >= 34 && length <= raw.len(),
        "invalid ISO directory record"
    );
    let name_length = usize::from(raw[32]);
    ensure!(33 + name_length <= length, "truncated ISO file name");
    let name_bytes = &raw[33..33 + name_length];
    let name = match name_bytes {
        [0] => ".".to_string(),
        [1] => "..".to_string(),
        _ => {
            let raw_name = std::str::from_utf8(name_bytes)?;
            raw_name.split(';').next().unwrap_or(raw_name).to_string()
        }
    };
    Ok(FileRecord {
        extent_lba: u32_both(raw, 2)?,
        extended_attribute_blocks: raw[1],
        size: u32_both(raw, 10)?,
        flags: raw[25],
        name,
    })
}

fn u16_both(data: &[u8], offset: usize) -> Result<u16> {
    ensure!(offset + 4 <= data.len(), "truncated dual-endian u16");
    let little = u16::from_le_bytes(data[offset..offset + 2].try_into()?);
    let big = u16::from_be_bytes(data[offset + 2..offset + 4].try_into()?);
    if little != big {
        bail!("ISO dual-endian u16 mismatch at 0x{offset:x}");
    }
    Ok(little)
}

fn u32_both(data: &[u8], offset: usize) -> Result<u32> {
    ensure!(offset + 8 <= data.len(), "truncated dual-endian u32");
    let little = u32::from_le_bytes(data[offset..offset + 4].try_into()?);
    let big = u32::from_be_bytes(data[offset + 4..offset + 8].try_into()?);
    if little != big {
        bail!("ISO dual-endian u32 mismatch at 0x{offset:x}");
    }
    Ok(little)
}
