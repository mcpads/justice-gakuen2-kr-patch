//! Prepared whole-card pixels own the member instead of legacy text writers.
use super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct WholeImage {
    pub member_index: usize,
    source_decoded_sha256: String,
    indices: String,
    indices_sha256: String,
    protected_source_regions: Vec<Cell>,
    review_status: String,
}

pub(super) fn load(assets: &Path) -> Result<BTreeMap<usize, WholeImage>> {
    let path = assets.join("whole-images.json");
    if !path.try_exists()? {
        return Ok(BTreeMap::new());
    }
    select(read_json::<Entries<WholeImage>>(&path)?.entries)
}

fn select(entries: Vec<WholeImage>) -> Result<BTreeMap<usize, WholeImage>> {
    let mut selected = BTreeMap::new();
    for entry in entries {
        ensure!(
            entry.member_index < 60,
            "whole card member outside card family"
        );
        ensure!(
            !entry.indices.is_empty()
                && Path::new(&entry.indices)
                    .components()
                    .all(|c| matches!(c, Component::Normal(_))),
            "unsafe whole card indices path"
        );
        ensure!(
            !entry.review_status.trim().is_empty(),
            "whole card review status missing"
        );
        ensure!(
            selected.insert(entry.member_index, entry).is_none(),
            "duplicate whole card member"
        );
    }
    Ok(selected)
}

fn apply(source: &[u8], indices: &[u8], entry: &WholeImage) -> Result<Vec<u8>> {
    ensure!(
        sha256_bytes(source) == entry.source_decoded_sha256,
        "whole card source changed"
    );
    ensure!(
        sha256_bytes(indices) == entry.indices_sha256,
        "whole card indices changed"
    );
    let tim = parse_8bpp_prefix(source)?;
    ensure!(
        tim.pixel_width() == WIDTH
            && tim.image_height == HEIGHT
            && tim.pixel_offset == 544
            && tim.clut_width == 256
            && tim.clut_height == 1
            && tim.total_size == source.len()
            && indices.len() == WIDTH * HEIGHT,
        "whole card geometry changed"
    );
    ensure!(
        source.len() == 544 + indices.len(),
        "whole card TIM layout changed"
    );
    for cell in &entry.protected_source_regions {
        ensure!(
            cell.width > 0
                && cell.height > 0
                && cell.x.checked_add(cell.width).is_some_and(|v| v <= WIDTH)
                && cell.y.checked_add(cell.height).is_some_and(|v| v <= HEIGHT),
            "whole card protected region outside image"
        );
        for y in cell.y..cell.y + cell.height {
            let at = y * WIDTH + cell.x;
            ensure!(
                indices[at..at + cell.width] == source[544 + at..544 + at + cell.width],
                "whole card protected source pixels changed"
            );
        }
    }
    let mut output = source[..544].to_vec();
    output.extend_from_slice(indices);
    Ok(output)
}

pub(super) fn render(
    source: &[u8],
    assets: &Path,
    entry: &WholeImage,
) -> Result<(Vec<u8>, serde_json::Value)> {
    let indices = std::fs::read(assets.join(&entry.indices))?;
    let output = apply(source, &indices, entry)?;
    Ok((
        output,
        json!({
            "family":"whole_card_image", "member_index":entry.member_index,
            "source_decoded_sha256":entry.source_decoded_sha256,
            "indices":entry.indices,"indices_sha256":entry.indices_sha256,
            "review_status":entry.review_status,
            "protected_source_regions":entry.protected_source_regions,
            "palette_and_header_source_identical":true,
            "legacy_text_and_decoration_writers_skipped":true
        }),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (Vec<u8>, Vec<u8>, WholeImage) {
        let mut source = vec![7; 544 + WIDTH * HEIGHT];
        for (at, word) in [
            (0, 16_u32),
            (4, 9),
            (8, 524),
            (532, (12 + WIDTH * HEIGHT) as u32),
        ] {
            source[at..at + 4].copy_from_slice(&word.to_le_bytes());
        }
        for (at, word) in [(16, 256_u16), (18, 1), (540, 152), (542, 384)] {
            source[at..at + 2].copy_from_slice(&word.to_le_bytes());
        }
        let mut indices = source[544..].to_vec();
        indices[224 * WIDTH..].fill(0);
        let entry = WholeImage {
            member_index: 38,
            source_decoded_sha256: sha256_bytes(&source),
            indices: "whole-images/member-38.indices".into(),
            indices_sha256: sha256_bytes(&indices),
            protected_source_regions: vec![Cell {
                x: 0,
                y: 0,
                width: WIDTH,
                height: 224,
            }],
            review_status: "candidate_needs_human_review".into(),
        };
        (source, indices, entry)
    }

    #[test]
    fn whole_card_preserves_source_header_and_portrait_without_overwriting_authored_paper() {
        let (source, indices, entry) = fixture();
        let output = apply(&source, &indices, &entry).unwrap();
        assert_eq!(&output[..544 + 224 * WIDTH], &source[..544 + 224 * WIDTH]);
        assert_eq!(&output[544..], indices);
    }

    #[test]
    fn changed_source_pixels_and_invalid_protection_fail_even_with_updated_input_hash() {
        let (source, mut indices, mut entry) = fixture();
        indices[0] ^= 1;
        assert!(apply(&source, &indices, &entry).is_err());
        entry.indices_sha256 = sha256_bytes(&indices);
        assert!(apply(&source, &indices, &entry).is_err());
        indices[0] ^= 1;
        entry.indices_sha256 = sha256_bytes(&indices);
        entry.protected_source_regions[0].x = usize::MAX;
        assert!(apply(&source, &indices, &entry).is_err());
        entry.protected_source_regions.clear();
        let mut altered = source.clone();
        altered[20] ^= 1;
        assert!(apply(&altered, &indices, &entry).is_err());
        indices.pop();
        entry.indices_sha256 = sha256_bytes(&indices);
        assert!(apply(&source, &indices, &entry).is_err());
    }

    #[test]
    fn whole_card_selection_rejects_duplicate_members_and_unsafe_paths() {
        let (_, _, a) = fixture();
        let (_, _, b) = fixture();
        assert!(select(vec![a, b]).is_err());
        let (_, _, mut a) = fixture();
        a.indices = "../outside.indices".into();
        assert!(select(vec![a]).is_err());
        let (_, _, mut a) = fixture();
        a.member_index = 60;
        assert!(select(vec![a]).is_err());
    }
}
