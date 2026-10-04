use crate::tim::Cell;

use std::path::PathBuf;

use super::build::{
    MenuGlyphBuildCell, MenuGlyphBuildReport, select_glyph_report_regions, validate_restore_regions,
};
use super::model::{
    MenuGlyphReportRestoreSpec, MenuTextureRepresentation, MenuTextureRestoreRegion,
};

fn region(role: &str) -> MenuTextureRestoreRegion {
    MenuTextureRestoreRegion {
        role: role.to_string(),
        tim_offset: 0x22800,
        cell: Cell {
            x: 304,
            y: 48,
            width: 112,
            height: 32,
        },
        representation: MenuTextureRepresentation::Indexed4bppWithClut,
    }
}

#[test]
fn distinct_restore_regions_are_accepted() {
    let mut second = region("detail-title");
    second.cell.y += 32;
    validate_restore_regions(&[region("list-label"), second]).unwrap();
}

#[test]
fn repeated_restore_region_is_rejected() {
    assert!(validate_restore_regions(&[region("first"), region("second")]).is_err());
}

fn glyph_report_restore(roles: &[&str]) -> MenuGlyphReportRestoreSpec {
    MenuGlyphReportRestoreSpec {
        path: PathBuf::from("glyph-report.json"),
        sha256: "00".repeat(32),
        tim_offset: 0,
        roles: roles.iter().map(|role| (*role).to_string()).collect(),
        code_ranges: Vec::new(),
        representation: MenuTextureRepresentation::Indexed4bppWithClut,
    }
}

fn glyph_report() -> MenuGlyphBuildReport {
    MenuGlyphBuildReport {
        glyphs: vec![
            MenuGlyphBuildCell {
                role: "label".to_string(),
                code: "0x0100".to_string(),
                cell: Cell {
                    x: 0,
                    y: 20,
                    width: 20,
                    height: 20,
                },
            },
            MenuGlyphBuildCell {
                role: "value".to_string(),
                code: "0x0101".to_string(),
                cell: Cell {
                    x: 20,
                    y: 20,
                    width: 20,
                    height: 20,
                },
            },
        ],
    }
}

#[test]
fn empty_role_filter_restores_every_reported_glyph() {
    let regions = select_glyph_report_regions(&glyph_report(), &glyph_report_restore(&[])).unwrap();
    assert_eq!(regions.len(), 2);
}

#[test]
fn role_filter_restores_only_matching_reported_glyphs() {
    let regions =
        select_glyph_report_regions(&glyph_report(), &glyph_report_restore(&["value"])).unwrap();
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].role, "value glyph 0x0101");
}

#[test]
fn missing_report_role_is_rejected() {
    assert!(
        select_glyph_report_regions(&glyph_report(), &glyph_report_restore(&["missing"])).is_err()
    );
}

#[test]
fn code_range_filter_restores_only_matching_reported_glyphs() {
    let mut restore = glyph_report_restore(&[]);
    restore.code_ranges = vec![[0x0101, 0x0102]];
    let regions = select_glyph_report_regions(&glyph_report(), &restore).unwrap();
    assert_eq!(regions.len(), 1);
    assert_eq!(regions[0].role, "value glyph 0x0101");
}

#[test]
fn empty_code_range_is_rejected() {
    let mut restore = glyph_report_restore(&[]);
    restore.code_ranges = vec![[0x0101, 0x0101]];
    assert!(select_glyph_report_regions(&glyph_report(), &restore).is_err());
}
