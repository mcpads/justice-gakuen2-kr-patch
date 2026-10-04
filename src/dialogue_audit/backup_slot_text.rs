use anyhow::{Result, ensure};
use serde::Serialize;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, serde::Deserialize)]
pub struct BackupSlotTextCounts {
    pub empty: i16,
    pub error: i16,
    pub clear: i16,
    #[serde(default)]
    pub translated_empty: bool,
    #[serde(default)]
    pub translated_error: bool,
}

pub(super) fn backup_slot_text_counts(
    translations: &BTreeMap<String, Vec<String>>,
) -> Result<BackupSlotTextCounts> {
    fn count(
        translations: &BTreeMap<String, Vec<String>>,
        key: &str,
        capacity: i16,
    ) -> Result<i16> {
        let Some(segments) = translations.get(key) else {
            return Ok(capacity);
        };
        ensure!(
            segments.iter().all(|s| !s.contains(['\n', '\r', '\t'])),
            "backup slot labels must fit one native row"
        );
        let n = segments.iter().flat_map(|s| s.chars()).count();
        ensure!(
            n > 0 && n <= capacity as usize,
            "backup slot label exceeds its {capacity} native cells"
        );
        Ok(n as i16)
    }
    Ok(BackupSlotTextCounts {
        translated_empty: translations
            .contains_key("f0e2af86ae9831e7281406f8dc23e2bde07b9ca8f881b729a5405195341a1606"),
        translated_error: translations
            .contains_key("16d72cd24e3f4b97c6872745ebdfeb128463b9052f33081088e061ea794d6e06"),
        empty: count(
            translations,
            "f0e2af86ae9831e7281406f8dc23e2bde07b9ca8f881b729a5405195341a1606",
            6,
        )?,
        error: count(
            translations,
            "16d72cd24e3f4b97c6872745ebdfeb128463b9052f33081088e061ea794d6e06",
            9,
        )?,
        clear: count(
            translations,
            "ba47cfa055b2894242a3394a702cf91e8139cb6a5b8129b14802e838b24bf9e7",
            6,
        )?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn backup_labels_keep_source_lengths_or_use_translated_visible_cells() {
        let mut texts = BTreeMap::new();
        assert_eq!(
            backup_slot_text_counts(&texts).unwrap(),
            BackupSlotTextCounts {
                empty: 6,
                error: 9,
                clear: 6,
                translated_empty: false,
                translated_error: false
            }
        );
        texts.insert(
            "f0e2af86ae9831e7281406f8dc23e2bde07b9ca8f881b729a5405195341a1606".into(),
            vec![
                "".into(),
                "데이터".into(),
                "".into(),
                " 없음".into(),
                "".into(),
            ],
        );
        texts.insert(
            "16d72cd24e3f4b97c6872745ebdfeb128463b9052f33081088e061ea794d6e06".into(),
            vec!["데이터".into(), " 오류".into(), "".into()],
        );
        let counts = backup_slot_text_counts(&texts).unwrap();
        assert_eq!((counts.empty, counts.error), (6, 6));
        assert!(counts.translated_empty && counts.translated_error);
        let key = "ba47cfa055b2894242a3394a702cf91e8139cb6a5b8129b14802e838b24bf9e7".to_string();
        texts.insert(key.clone(), vec!["클리어!".into(), String::new()]);
        assert_eq!(backup_slot_text_counts(&texts).unwrap().clear, 4);
        texts.insert(key, vec!["넘치는글자일곱".into()]);
        assert!(backup_slot_text_counts(&texts).is_err());
    }
}
