//! The password heading mixes large EDITMOJI cells with the shared MENU atlas.
//! Reserve the complete sampled rectangles, including the second glyph row.

use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};

use crate::pipeline::sha256_bytes;

const TITLE_TABLE_OFFSET: usize = 0x064c;
const TITLE_SELECTORS: [u8; 21] = [
    0x0b, 0, 0, 0x0f, 5, 2, 0x0b, 1, 0, 0x0f, 1, 1, 0x0f, 3, 2, 0x0b, 2, 0, 0x0b, 3, 0,
];
const TITLE_RENDERER_START: usize = 0x1f10;
const TITLE_RENDERER_END: usize = 0x2114;
const TITLE_RENDERER_SHA256: &str =
    "7f395fe9f4886da625bd70e64261d86c0d50300aa2232f00f998c456a7976b31";

pub(super) fn validate_password_title(pass: &[u8]) -> Result<()> {
    ensure!(
        pass.get(TITLE_TABLE_OFFSET..TITLE_TABLE_OFFSET + TITLE_SELECTORS.len())
            == Some(TITLE_SELECTORS.as_slice()),
        "PASS password-title texture selectors changed"
    );
    let renderer = pass
        .get(TITLE_RENDERER_START..TITLE_RENDERER_END)
        .context("PASS password-title renderer is truncated")?;
    ensure!(
        sha256_bytes(renderer) == TITLE_RENDERER_SHA256,
        "PASS password-title sprite size, page selection, or consumer changed"
    );
    Ok(())
}

pub(super) fn password_title_codes() -> Result<BTreeSet<u16>> {
    title_codes(&TITLE_SELECTORS)
}

fn title_codes(selectors: &[u8]) -> Result<BTreeSet<u16>> {
    let (selectors, remainder) = selectors.as_chunks::<3>();
    ensure!(remainder.is_empty(), "incomplete password-title selector");
    let mut codes = BTreeSet::new();
    for &[page, column, row] in selectors {
        // Physical page 11 is PASS's direct-page namespace, while physical
        // page 15 is logical page 3 of the MENU image rooted at VRAM x=768.
        let logical_page = match page {
            0x0b => 0x0f00,
            0x0f => 0x0300,
            _ => anyhow::bail!("unbound password-title physical page {page}"),
        };
        let u = u16::from(column) * 40;
        let v = u16::from(row) * 40;
        ensure!(
            u + 40 <= 256 && v + 40 <= 256,
            "password title wraps its texture page"
        );
        for dy in [0, 20] {
            for dx in [0, 20] {
                codes.insert(logical_page | (((v + dy) / 20) << 4) | ((u + dx) / 20));
            }
        }
    }
    Ok(codes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn large_title_cells_reserve_both_rows_in_their_renderer_namespaces() {
        let codes = title_codes(&[0x0f, 5, 2, 0x0b, 1, 0]).unwrap();
        assert_eq!(
            codes,
            BTreeSet::from([
                0x034a, 0x034b, 0x035a, 0x035b, 0x0f02, 0x0f03, 0x0f12, 0x0f13
            ])
        );
        assert!(title_codes(&[0x0c, 0, 0]).is_err());
        assert!(title_codes(&[0x0f, 6, 0]).is_err());
        assert!(title_codes(&[0x0b, 0, 6]).is_err());
        assert!(title_codes(&[0x0b, 0]).is_err());
    }
}
