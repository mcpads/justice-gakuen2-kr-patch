use super::model::DiaryHeaderEntry;
use crate::pipeline::sha256_bytes;
use crate::tim::Cell;
use anyhow::{Context, Result, ensure};

pub(super) fn validate_backup_schools(source: &[u8], entries: &[DiaryHeaderEntry]) -> Result<()> {
    for (start, end, hash) in [
        (
            0x2600,
            0x2614,
            "679d486caa5196cae527139566e2546dc2ee6f734217620363a5d86e4ebe68c1",
        ),
        (
            0x21f10,
            0x21ff4,
            "42b0ccf40b260a8a4b02ad3ace5db73f1dbacac3415e7af89f7b6c9216ee849e",
        ),
    ] {
        ensure!(
            sha256_bytes(
                source
                    .get(start..end)
                    .context("truncated backup school consumer")?
            ) == hash,
            "backup school descriptor/renderer binding changed"
        );
    }
    for (id, x, y, width) in [
        ("taiyo", 592, 0, 72),
        ("gorin", 592, 20, 72),
        ("pacific", 512, 0, 80),
        ("gedo", 664, 0, 72),
        ("justice", 512, 20, 80),
    ] {
        let id = format!("backup-school-{id}");
        let e = entries
            .iter()
            .find(|e| e.id == id)
            .with_context(|| format!("missing backup school {id}"))?;
        ensure!(
            e.cell
                == Cell {
                    x,
                    y,
                    width,
                    height: 20
                },
            "backup school {id} leaves its native sprite cell"
        );
    }
    Ok(())
}
