//! Native 12-byte CD resource locator, distinct from an ISO directory record.
use anyhow::{Result, ensure};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct NativeDiscResource {
    pub lba: u32,
    pub byte_count: u32,
    pub prefix: [u8; 4],
}

impl NativeDiscResource {
    pub fn parse(bytes: &[u8]) -> Result<Self> {
        ensure!(
            bytes.len() == 12 && bytes[3] == 0,
            "invalid native CD locator extent or reserved byte"
        );
        let bcd = |value: u8| -> Result<u32> {
            ensure!(
                value >> 4 <= 9 && value & 15 <= 9,
                "invalid CD position BCD"
            );
            Ok(u32::from(value >> 4) * 10 + u32::from(value & 15))
        };
        let (minutes, seconds, frames) = (bcd(bytes[0])?, bcd(bytes[1])?, bcd(bytes[2])?);
        ensure!(seconds < 60 && frames < 75, "invalid CD position units");
        let absolute = minutes * 4500 + seconds * 75 + frames;
        ensure!(absolute >= 150, "native CD locator precedes data LBA zero");
        let byte_count = u32::from_le_bytes(bytes[4..8].try_into()?);
        ensure!(
            byte_count >= 4,
            "native resource lacks its validation prefix"
        );
        Ok(Self {
            lba: absolute - 150,
            byte_count,
            prefix: bytes[8..12].try_into()?,
        })
    }

    /// Does not relocate a disc record. Its owner must separately bind the ISO
    /// extent and sealed sector writes to this same location and replacement.
    pub fn replacement(lba: u32, stored: &[u8]) -> Result<[u8; 12]> {
        ensure!(
            stored.len() >= 4,
            "native resource lacks its validation prefix"
        );
        let byte_count = u32::try_from(stored.len())?;
        let absolute = lba
            .checked_add(150)
            .ok_or_else(|| anyhow::anyhow!("CD position overflow"))?;
        ensure!(
            absolute < 100 * 4500,
            "CD position exceeds two-digit BCD minutes"
        );
        let bcd = |value: u32| ((value / 10) * 16 + value % 10) as u8;
        let mut entry = [0; 12];
        entry[..3].copy_from_slice(&[
            bcd(absolute / 4500),
            bcd(absolute / 75 % 60),
            bcd(absolute % 75),
        ]);
        entry[4..8].copy_from_slice(&byte_count.to_le_bytes());
        entry[8..12].copy_from_slice(&stored[..4]);
        Ok(entry)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn measured_name_font_locator_binds_location_size_and_signature() {
        let source = [0x06, 0x18, 0x48, 0, 0xc2, 0xac, 1, 0, 0, 0, 0x10, 0];
        let decoded = NativeDiscResource::parse(&source).unwrap();
        assert_eq!(decoded.lba, 28248);
        assert_eq!(decoded.byte_count, 109762);
        let mut stored = vec![0; 109762];
        stored[..4].copy_from_slice(&decoded.prefix);
        assert_eq!(
            NativeDiscResource::replacement(decoded.lba, &stored).unwrap(),
            source
        );
        stored.extend_from_slice(&[1, 2, 3, 4]);
        stored[..4].copy_from_slice(&[5, 6, 7, 8]);
        let moved =
            NativeDiscResource::parse(&NativeDiscResource::replacement(281980, &stored).unwrap())
                .unwrap();
        assert_eq!(
            moved,
            NativeDiscResource {
                lba: 281980,
                byte_count: 109766,
                prefix: [5, 6, 7, 8]
            }
        );
    }

    #[test]
    fn position_boundaries_roundtrip_and_malformed_entries_fail() {
        for lba in [0, 74, 75, 4349, 4350, 449849] {
            let entry = NativeDiscResource::replacement(lba, &[1, 2, 3, 4]).unwrap();
            assert_eq!(NativeDiscResource::parse(&entry).unwrap().lba, lba);
        }
        for lba in [449850, u32::MAX] {
            assert!(NativeDiscResource::replacement(lba, &[1, 2, 3, 4]).is_err());
        }
        assert!(NativeDiscResource::replacement(0, &[1, 2, 3]).is_err());
        let good = NativeDiscResource::replacement(0, &[1, 2, 3, 4]).unwrap();
        for (at, value) in [(0, 0xfa), (1, 0x60), (2, 0x75), (3, 1), (4, 0)] {
            let mut bad = good;
            bad[at] = value;
            assert!(NativeDiscResource::parse(&bad).is_err());
        }
        let mut before_data = good;
        before_data[1] = 0;
        assert!(NativeDiscResource::parse(&before_data).is_err());
        assert!(NativeDiscResource::parse(&good[..11]).is_err());
    }
}
