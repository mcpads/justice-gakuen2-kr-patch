use anyhow::{Context, Result, ensure};

use super::model::{DiaryHeaderEntry, DiaryHeaderFontRole};
use crate::tim::Cell;

// MGAME 0x800c66c0 indexes this table by the numeric value. Zero is the last
// atlas cell; these are 20x20 profile sprites, not the 16x24 HUD digits.
const PROFILE_DIGIT_UV: [u8; 20] = [
    180, 184, 0, 184, 20, 184, 40, 184, 60, 184, 80, 184, 100, 184, 120, 184, 140, 184, 160, 184,
];

pub(super) fn validate(source: &[u8], entries: &[DiaryHeaderEntry]) -> Result<()> {
    ensure!(
        source.get(0x2fc8..0x2fdc) == Some(PROFILE_DIGIT_UV.as_slice()),
        "profile decimal sprite lookup changed"
    );
    for e in entries
        .iter()
        .filter(|e| e.id.starts_with("profile_") || e.id.starts_with("profile-digit-"))
    {
        ensure!(
            e.font_role == DiaryHeaderFontRole::ProfileText && e.cell.height == 20,
            "{} leaves the shared native profile typography",
            e.id
        );
    }
    for (digit, uv) in PROFILE_DIGIT_UV.as_chunks::<2>().0.iter().enumerate() {
        let e = entries
            .iter()
            .find(|e| e.id == format!("profile-digit-{digit}"))
            .context("missing profile decimal glyph")?;
        ensure!(
            e.cell
                == Cell {
                    x: 512 + usize::from(uv[0]),
                    y: usize::from(uv[1]),
                    width: 20,
                    height: 20
                }
                && e.korean_text == digit.to_string()
                && e.font_role == DiaryHeaderFontRole::ProfileText,
            "profile decimal glyph differs from its native value/cell/style"
        );
    }
    for digit in 0..10 {
        let e = entries
            .iter()
            .find(|e| e.id == format!("date-digit-{digit}"))
            .context("missing calendar decimal glyph")?;
        ensure!(
            e.font_role == DiaryHeaderFontRole::CalendarText
                && e.korean_text == digit.to_string()
                && e.cell
                    == Cell {
                        x: digit * 16,
                        y: 0,
                        width: 16,
                        height: 24
                    },
            "calendar decimal glyph differs from its native value/cell/style"
        );
    }
    for e in entries.iter().filter(|e| {
        matches!(
            e.font_role,
            DiaryHeaderFontRole::CalendarText | DiaryHeaderFontRole::ProfileText
        )
    }) {
        ensure!(
            e.font_px.is_none() && e.vertical_shift_px.is_none(),
            "{} overrides its shared date/profile typography",
            e.id
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diary_header::assets::load_diary_header_assets;
    use std::path::Path;

    #[test]
    #[ignore = "requires assets/"]
    fn dates_require_all_native_digits_and_shared_typography() {
        let (_, entries) = load_diary_header_assets(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/diary/header"),
        )
        .unwrap();
        let mut source = vec![0; 0x2fdc];
        source[0x2fc8..0x2fdc].copy_from_slice(&PROFILE_DIGIT_UV);
        validate(&source, &entries).unwrap();
        let (_, mut changed) = load_diary_header_assets(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/diary/header"),
        )
        .unwrap();
        changed.retain(|e| e.id != "profile-digit-0");
        assert!(validate(&source, &changed).is_err());
        let (_, mut changed) = load_diary_header_assets(
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/diary/header"),
        )
        .unwrap();
        changed
            .iter_mut()
            .find(|e| e.id == "profile-digit-1")
            .unwrap()
            .cell
            .y = 180;
        assert!(validate(&source, &changed).is_err());
        let mut changed = entries;
        changed
            .iter_mut()
            .find(|e| e.id == "calendar-month-monday")
            .unwrap()
            .font_px = Some(16.0);
        assert!(validate(&source, &changed).is_err());
    }
}
