//! Source-bound two-column choices. Authored prose and protected source controls
//! remain immutable; preparation lays out the admitted menu labels in ten-cell slots.
use super::translation_model::DialogueTranslationControl;
use anyhow::{Result, ensure};
use serde::Deserialize;
use std::borrow::Cow;
use std::sync::OnceLock;

fn asset() -> Result<&'static [u8]> {
    crate::product_assets::read("dialogue/choice-columns.json")
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Columns {
    right_column: usize,
    semantic_source_sha256: Vec<String>,
}
pub(super) fn asset_sha256() -> Result<String> {
    Ok(crate::pipeline::sha256_bytes(asset()?))
}

pub(super) fn prepare<'a>(
    semantic_hash: &str,
    segments: &'a [String],
    controls: &'a [DialogueTranslationControl],
) -> Result<(Cow<'a, [String]>, Cow<'a, [DialogueTranslationControl]>)> {
    static COLUMNS: OnceLock<Columns> = OnceLock::new();
    let columns = match COLUMNS.get() {
        Some(columns) => columns,
        None => {
            let parsed: Columns = serde_json::from_slice(asset()?)?;
            COLUMNS.get_or_init(|| parsed)
        }
    };
    if !columns
        .semantic_source_sha256
        .iter()
        .any(|h| h == semantic_hash)
    {
        return Ok((Cow::Borrowed(segments), Cow::Borrowed(controls)));
    }
    ensure!(columns.right_column == 10, "choice renderer column changed");
    ensure!(
        segments.len() == controls.len() + 1 && segments.last().is_some_and(String::is_empty),
        "choice shape changed"
    );
    let mut out_segments = Vec::new();
    let mut out_controls = Vec::new();
    let mut labels = Vec::new();
    let mut ended = false;
    for (segment, control) in segments.iter().zip(controls) {
        ensure!(
            !ended && control.arguments.is_empty(),
            "choice control shape changed"
        );
        if !segment.trim().is_empty() {
            ensure!(
                !segment.contains(['\n', '\r', '\t']),
                "choice contains inline control spacing"
            );
            labels.push(segment.trim_end());
        }
        match control.semantic_name.as_str() {
            "name_buffer_padding" => (),
            "line_break" | "message_end" => {
                ensure!(
                    (1..=2).contains(&labels.len()),
                    "choice row lost option ownership"
                );
                let mut row = labels[0].to_string();
                if labels.len() == 2 {
                    let left_width = row.chars().count();
                    ensure!(
                        left_width < columns.right_column,
                        "left choice overlaps right column"
                    );
                    row.push_str(&" ".repeat(columns.right_column - left_width));
                    row.push_str(labels[1]);
                }
                ensure!(row.chars().count() <= 20, "choice row exceeds native width");
                out_segments.push(row);
                out_controls.push(control.clone());
                labels.clear();
                ended = control.semantic_name == "message_end";
            }
            _ => anyhow::bail!("non-layout control in admitted static choice"),
        }
    }
    ensure!(
        ended && labels.is_empty() && out_controls.len() <= 4,
        "choice row/end shape changed"
    );
    out_segments.push(String::new());
    Ok((Cow::Owned(out_segments), Cow::Owned(out_controls)))
}

#[cfg(test)]
mod tests {
    use super::super::dialogue_message_encoding::{
        encode_translated_message, encoded_message_byte_count,
    };
    use super::*;
    const ID: &str = "bf732d8ff1f7f72ebf44b5657c9c9499e60473b0680dc89423e797361211457b";
    fn control(name: &str) -> DialogueTranslationControl {
        DialogueTranslationControl {
            semantic_name: name.into(),
            arguments: vec![],
        }
    }
    #[test]
    #[ignore = "requires assets/"]
    fn short_and_long_labels_have_the_same_encoded_right_column() {
        for left in ["축구부", "잠재 능력", "티파니를"] {
            let segments = vec![left.into(), "스모부".into(), "".into()];
            let controls = vec![control("name_buffer_padding"), control("message_end")];
            let (s, c) = prepare(ID, &segments, &controls).unwrap();
            let codes = s
                .iter()
                .flat_map(|v| v.chars())
                .map(|c| (c, c as u16))
                .collect();
            let bytes = encode_translated_message(&s, &c, &codes).unwrap();
            assert_eq!(u16::from_le_bytes([bytes[20], bytes[21]]), '스' as u16);
            assert_eq!(bytes.len(), encoded_message_byte_count(&s, &c).unwrap());
            assert_eq!(
                s[0].split_whitespace().collect::<Vec<_>>(),
                format!("{left} 스모부")
                    .split_whitespace()
                    .collect::<Vec<_>>()
            );
        }
    }
    #[test]
    #[ignore = "requires assets/"]
    fn admitted_assets_preserve_labels_and_row_boundaries() {
        let columns: Columns = serde_json::from_slice(asset().unwrap()).unwrap();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let manifest: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join("assets/dialogue/translations/manifest.json")).unwrap(),
        )
        .unwrap();
        let base = root.join("assets/dialogue/translations");
        let mut found = std::collections::BTreeSet::new();
        for index in manifest["owner_index_manifests"].as_array().unwrap() {
            let owners: serde_json::Value = serde_json::from_slice(
                &std::fs::read(base.join(index["manifest_path"].as_str().unwrap())).unwrap(),
            )
            .unwrap();
            for owner in owners["owners"].as_array().unwrap() {
                for part in owner["manifests"].as_array().unwrap() {
                    let shards: serde_json::Value = serde_json::from_slice(
                        &std::fs::read(base.join(part["manifest_path"].as_str().unwrap())).unwrap(),
                    )
                    .unwrap();
                    for shard in shards["shards"].as_array().unwrap() {
                        let data: serde_json::Value = serde_json::from_slice(
                            &std::fs::read(base.join(shard["path"].as_str().unwrap())).unwrap(),
                        )
                        .unwrap();
                        for entry in data["entries"].as_array().unwrap() {
                            let hash = entry["semantic_source_sha256"].as_str().unwrap();
                            if !columns.semantic_source_sha256.iter().any(|v| v == hash) {
                                continue;
                            }
                            assert!(found.insert(hash.to_string()));
                            let segments: Vec<String> =
                                serde_json::from_value(entry["korean_segments"].clone()).unwrap();
                            let controls: Vec<DialogueTranslationControl> =
                                serde_json::from_value(entry["controls"].clone()).unwrap();
                            let (s, c) = prepare(hash, &segments, &controls).unwrap();
                            assert_eq!(
                                s.iter()
                                    .flat_map(|v| v.chars())
                                    .filter(|v| !v.is_whitespace())
                                    .collect::<String>(),
                                segments
                                    .iter()
                                    .flat_map(|v| v.chars())
                                    .filter(|v| !v.is_whitespace())
                                    .collect::<String>()
                            );
                            assert_eq!(
                                c.as_ref(),
                                controls
                                    .iter()
                                    .filter(|v| v.semantic_name != "name_buffer_padding")
                                    .cloned()
                                    .collect::<Vec<_>>()
                            );
                        }
                    }
                }
            }
        }
        assert_eq!(found.len(), columns.semantic_source_sha256.len());
    }
    #[test]
    #[ignore = "requires assets/"]
    fn unrelated_messages_are_not_reflowed() {
        let s = vec!["긴 대사".into(), "".into()];
        let c = vec![control("message_end")];
        let (actual, controls) = prepare("not-admitted", &s, &c).unwrap();
        assert!(matches!(actual, Cow::Borrowed(_)));
        assert_eq!(controls.as_ref(), c);
    }
    #[test]
    fn dynamic_controls_and_overlapping_labels_fail_closed() {
        let c = vec![control("protagonist_family_name"), control("message_end")];
        assert!(prepare(ID, &["".into(), "".into(), "".into()], &c).is_err());
        let c = vec![control("name_buffer_padding"), control("message_end")];
        assert!(prepare(ID, &["가".repeat(10), "나".into(), "".into()], &c).is_err());
    }
}
