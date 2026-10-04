use anyhow::{Context, Result, bail, ensure};

use super::description_model::{DescriptionSpan, DescriptionStream};

pub(super) const CELLS_PER_ROW: usize = 12;
pub(super) const CELL_ROWS: usize = 10;
pub(super) const CELL_WIDTH: usize = 20;
pub(super) const CELL_HEIGHT: usize = 24;
pub(super) const CELL_CAPACITY: usize = CELLS_PER_ROW * CELL_ROWS;
const LINE_BREAK: u8 = 0xfe;
const TERMINATOR: u8 = 0xff;

pub(super) fn parse_description_stream(bytes: &[u8]) -> Result<DescriptionStream> {
    let mut lines = vec![Vec::new()];
    let mut cursor = 0usize;
    loop {
        let token = *bytes.get(cursor).context("truncated description stream")?;
        cursor += 1;
        match token {
            TERMINATOR => {
                ensure!(
                    lines.iter().all(|line| !line.is_empty()),
                    "description stream contains an empty line"
                );
                return Ok(DescriptionStream { lines });
            }
            LINE_BREAK => {
                ensure!(
                    !lines
                        .last()
                        .context("description line disappeared")?
                        .is_empty(),
                    "description stream contains an empty line"
                );
                lines.push(Vec::new());
            }
            start_cell => {
                let cell_count = *bytes
                    .get(cursor)
                    .context("truncated description span width")?;
                cursor += 1;
                ensure!(cell_count > 0, "description span has zero width");
                let start = usize::from(start_cell);
                let count = usize::from(cell_count);
                ensure!(
                    start < CELL_CAPACITY && start + count <= CELL_CAPACITY,
                    "description span leaves the item atlas"
                );
                ensure!(
                    start / CELLS_PER_ROW == (start + count - 1) / CELLS_PER_ROW,
                    "description span crosses an atlas row"
                );
                lines
                    .last_mut()
                    .context("description line disappeared")?
                    .push(DescriptionSpan {
                        start_cell,
                        cell_count,
                    });
            }
        }
    }
}

pub(super) fn encode_description_stream(stream: &DescriptionStream) -> Result<Vec<u8>> {
    ensure!(!stream.lines.is_empty(), "description stream has no lines");
    let mut bytes = Vec::new();
    for (line_index, line) in stream.lines.iter().enumerate() {
        ensure!(
            !line.is_empty(),
            "description stream contains an empty line"
        );
        if line_index > 0 {
            bytes.push(LINE_BREAK);
        }
        for span in line {
            if span.start_cell >= LINE_BREAK {
                bail!("description span start collides with a control byte");
            }
            bytes.extend([span.start_cell, span.cell_count]);
        }
    }
    bytes.push(TERMINATOR);
    Ok(bytes)
}
