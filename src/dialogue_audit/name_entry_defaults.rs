use anyhow::{Result, ensure};

use crate::name_input::NameSlotCode;

// MGENT's empty-field copier (0x80180a98) selects these three terminated
// streams through the pointer table at 0x28. Keep stream extents and pointers
// intact; only replace legacy atlas identities with canonical name identities.
const DEFAULTS: [(usize, [u16; 4], &str); 3] = [
    (0x10, [0x00de, 0x00d2, 0x0100, 0x3001], "세이기"), // せいぎ
    (0x18, [0x00ef, 0x00da, 0x00e4, 0x3001], "마코토"), // まこと
    (0x20, [0x0075, 0x0060, 0x006a, 0x3001], "마코토"), // マコト
];

pub(super) fn patch_name_entry_defaults(source: &[u8]) -> Result<Vec<u8>> {
    let mut patched = source.to_vec();
    for (index, (offset, expected, text)) in DEFAULTS.iter().enumerate() {
        let expected_bytes: Vec<_> = expected
            .iter()
            .flat_map(|word| word.to_le_bytes())
            .collect();
        ensure!(
            source.get(*offset..offset + 8) == Some(expected_bytes.as_slice()),
            "name-entry default stream changed at {offset:#x}"
        );
        let pointer = (super::name_entry::OVERLAY_RUNTIME_BASE + *offset as u32).to_le_bytes();
        ensure!(
            source.get(0x28 + index * 4..0x2c + index * 4) == Some(pointer.as_slice()),
            "name-entry default pointer changed for field {index}"
        );
        let words = text
            .chars()
            .map(|c| NameSlotCode::Hangul(c).encode())
            .collect::<Result<Vec<_>>>()?;
        ensure!(
            words.len() == 3,
            "name-entry default exceeds its source stream"
        );
        for (i, word) in words.into_iter().enumerate() {
            patched[offset + i * 2..offset + i * 2 + 2].copy_from_slice(&word.to_le_bytes());
        }
    }
    Ok(patched)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source() -> Vec<u8> {
        let mut bytes = vec![0x5a; 0x40];
        for (index, (offset, words, _)) in DEFAULTS.iter().enumerate() {
            for (i, word) in words.iter().enumerate() {
                bytes[offset + i * 2..offset + i * 2 + 2].copy_from_slice(&word.to_le_bytes());
            }
            bytes[0x28 + index * 4..0x2c + index * 4].copy_from_slice(
                &(super::super::name_entry::OVERLAY_RUNTIME_BASE + *offset as u32).to_le_bytes(),
            );
        }
        bytes
    }

    #[test]
    fn empty_name_defaults_use_supported_names_and_preserve_native_stream_boundaries() {
        let source = source();
        let patched = patch_name_entry_defaults(&source).unwrap();
        for (offset, expected) in [(0x10, "세이기"), (0x18, "마코토"), (0x20, "마코토")] {
            let decoded: String = patched[offset..offset + 6]
                .chunks_exact(2)
                .map(|bytes| {
                    match NameSlotCode::decode(u16::from_le_bytes([bytes[0], bytes[1]])).unwrap() {
                        NameSlotCode::Hangul(c) => c,
                        other => panic!("default is not a supported Hangul name: {other:?}"),
                    }
                })
                .collect();
            assert_eq!(decoded, expected);
            assert_eq!(&patched[offset + 6..offset + 8], &0x3001u16.to_le_bytes());
        }
        for i in 0..source.len() {
            if !(0x10..0x16).contains(&i)
                && !(0x18..0x1e).contains(&i)
                && !(0x20..0x26).contains(&i)
            {
                assert_eq!(patched[i], source[i], "unowned byte {i:#x}");
            }
        }
        for offset in [0x10, 0x18, 0x20, 0x28, 0x2c, 0x30] {
            let mut changed = source.clone();
            changed[offset] ^= 1;
            assert!(patch_name_entry_defaults(&changed).is_err());
        }
    }
}
