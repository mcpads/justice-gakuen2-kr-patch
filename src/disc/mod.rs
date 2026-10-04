pub mod iso9660;
pub mod mode2;
pub mod rebuild;
pub mod record_plan;
pub mod record_relocation;
mod sector_finalizer;

use std::fs::File;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::Path;

use anyhow::{Context, Result, bail, ensure};

pub const RAW_SECTOR_SIZE: usize = 2352;
pub const USER_DATA_SIZE: usize = 2048;
pub const USER_DATA_OFFSET: usize = 0x18;
pub const SYNC: [u8; 12] = [
    0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x00,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectorKind {
    Mode1,
    Mode2Form1,
    Mode2Form2,
}

pub struct RawTrack {
    file: File,
    sector_count: u64,
}

impl RawTrack {
    pub fn open(path: &Path) -> Result<Self> {
        let file =
            File::open(path).with_context(|| format!("failed to open BIN: {}", path.display()))?;
        let size = file.metadata()?.len();
        ensure!(
            size.is_multiple_of(RAW_SECTOR_SIZE as u64),
            "BIN size is not a multiple of 2352 bytes"
        );
        Ok(Self {
            file,
            sector_count: size / RAW_SECTOR_SIZE as u64,
        })
    }

    pub fn sector_count(&self) -> u64 {
        self.sector_count
    }

    pub(crate) fn copy_bytes_to(&mut self, target: &mut impl Write) -> Result<u64> {
        self.file.seek(SeekFrom::Start(0))?;
        Ok(std::io::copy(&mut self.file, target)?)
    }

    pub fn raw_sector(&mut self, lba: u32) -> Result<[u8; RAW_SECTOR_SIZE]> {
        ensure!(
            u64::from(lba) < self.sector_count,
            "LBA {lba} outside track"
        );
        self.file
            .seek(SeekFrom::Start(u64::from(lba) * RAW_SECTOR_SIZE as u64))?;
        let mut raw = [0u8; RAW_SECTOR_SIZE];
        self.file.read_exact(&mut raw)?;
        ensure!(raw[..12] == SYNC, "invalid raw sector at LBA {lba}");
        Ok(raw)
    }

    pub fn user_sector(&mut self, lba: u32) -> Result<(Vec<u8>, SectorKind)> {
        let raw = self.raw_sector(lba)?;
        match raw[15] {
            1 => Ok((raw[16..16 + USER_DATA_SIZE].to_vec(), SectorKind::Mode1)),
            2 => {
                ensure!(
                    raw[16..20] == raw[20..24],
                    "Mode 2 subheader copies disagree at LBA {lba}"
                );
                if raw[18] & 0x20 != 0 {
                    Ok((raw[24..24 + 2324].to_vec(), SectorKind::Mode2Form2))
                } else {
                    Ok((
                        raw[USER_DATA_OFFSET..USER_DATA_OFFSET + USER_DATA_SIZE].to_vec(),
                        SectorKind::Mode2Form1,
                    ))
                }
            }
            mode => bail!("unsupported sector mode {mode} at LBA {lba}"),
        }
    }
}
