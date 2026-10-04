use anyhow::{Context, Result, ensure};
use serde::Serialize;

// KANRI 800a3b98 loads placement+8; 800a3d00..04 advances by 20 minus it.
pub(super) const SOURCE_GLYPH_ADVANCE: i16 = 20;
const BUTTON_GLYPH_ADVANCE: i16 = 16;

pub(super) fn button_index(id: &str) -> Option<usize> {
    // 800a4380 draws the first five placements; 800a9368 draws all six.
    // Both walk the same sprite pointer table at overlay +0x2b8.
    match id {
        "character_registration" | "cpu_edit" => Some(0),
        "load" | "status_tab" => Some(1),
        "register" | "command_tab" => Some(2),
        "password_input" | "password_display" => Some(3),
        "exit" | "unregister" => Some(4),
        "back" => Some(5),
        _ => None,
    }
}

#[derive(Debug, Serialize)]
pub(super) struct ButtonLayout {
    pub(super) bounds: [i16; 4],
    pub(super) x: i16,
    pub(super) glyph_advance_px: i16,
}

pub(super) fn layout_button(
    overlay: &[u8],
    index: usize,
    count: usize,
    y: i16,
) -> Result<ButtonLayout> {
    ensure!(index < 6, "unknown EDIT registration button");
    // The main and selected-character loops select these shared sprite pointers.
    // 800a39a4 consumes width/height at +12/+14 and x/y at +16/+18.
    let pointer_offset = 0x2b8 + index * 4;
    let pointer = u32::from_le_bytes(
        overlay
            .get(pointer_offset..pointer_offset + 4)
            .context("EDIT button pointer out of bounds")?
            .try_into()?,
    );
    let offset = usize::try_from(
        pointer
            .checked_sub(0x800a_2000)
            .context("EDIT button pointer below overlay")?,
    )?;
    ensure!(
        offset == 0x240 + index * 20,
        "EDIT button source pointer changed"
    );
    let sprite = overlay
        .get(offset..offset + 20)
        .context("EDIT button sprite out of bounds")?;
    let half = |offset| i16::from_le_bytes([sprite[offset], sprite[offset + 1]]);
    let (width, height, left, top) = (half(12), half(14), half(16), half(18));
    ensure!(width > 0 && height > 0, "EDIT button has invalid bounds");
    layout_button_bounds([left, top, width, height], count, y)
}

pub(super) fn layout_button_bounds(
    [left, top, width, height]: [i16; 4],
    count: usize,
    y: i16,
) -> Result<ButtonLayout> {
    ensure!(width > 0 && height > 0, "EDIT button has invalid bounds");
    let bounds = [
        left,
        top,
        left.checked_add(width).context("EDIT button x overflow")?,
        top.checked_add(height).context("EDIT button y overflow")?,
    ];
    ensure!(count > 0, "EDIT button label is empty");
    let span = i32::try_from(count - 1)? * i32::from(BUTTON_GLYPH_ADVANCE) + 20;
    ensure!(
        span <= i32::from(width),
        "EDIT button label exceeds parent button width"
    );
    ensure!(
        y >= top && i32::from(y) + 20 <= i32::from(bounds[3]),
        "EDIT button label exceeds parent button height"
    );
    Ok(ButtonLayout {
        bounds,
        x: left + i16::try_from((i32::from(width) - span) / 2)?,
        glyph_advance_px: BUTTON_GLYPH_ADVANCE,
    })
}

impl ButtonLayout {
    pub(super) fn validate_ink(&self, column: usize, y: i16, ink: [usize; 4]) -> Result<()> {
        let x = i32::from(self.x) + i32::try_from(column)? * i32::from(self.glyph_advance_px);
        ensure!(
            x + i32::try_from(ink[0])? >= i32::from(self.bounds[0])
                && x + i32::try_from(ink[2])? <= i32::from(self.bounds[2])
                && i32::from(y) + i32::try_from(ink[1])? >= i32::from(self.bounds[1])
                && i32::from(y) + i32::try_from(ink[3])? <= i32::from(self.bounds[3]),
            "EDIT button glyph ink exceeds parent button bounds"
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn submenu_uses_shared_buttons_including_last_row() {
        let mut overlay = vec![0; 0x2d0];
        overlay[0x2cc..0x2d0].copy_from_slice(&0x800a_22a4_u32.to_le_bytes());
        for (offset, value) in [(12, 118_i16), (14, 40), (16, 372), (18, 424)] {
            overlay[0x2a4 + offset..0x2a6 + offset].copy_from_slice(&value.to_le_bytes());
        }
        let index = button_index("back").unwrap();
        let layout = layout_button(&overlay, index, 2, 434).unwrap();
        assert_eq!(layout.bounds, [372, 424, 490, 464]);
        assert_eq!(layout.x, 413);
        assert!(layout_button(&overlay, index, 2, 450).is_err());
        assert_eq!(
            button_index("password_display"),
            button_index("password_input")
        );
        assert_eq!(
            button_index("cpu_edit"),
            button_index("character_registration")
        );
        assert_eq!(button_index("not_loaded"), None);
    }

    #[test]
    fn password_label_fits_parent_and_rejects_overflow() {
        let mut overlay = vec![0; 0x2cc];
        overlay[0x2c4..0x2c8].copy_from_slice(&0x800a_227c_u32.to_le_bytes());
        for (offset, value) in [(12, 118_i16), (14, 40), (16, 372), (18, 336)] {
            overlay[0x27c + offset..0x27e + offset].copy_from_slice(&value.to_le_bytes());
        }
        let layout = layout_button(&overlay, 3, 6, 345).unwrap();
        assert_eq!(layout.x, 381);
        assert_eq!(layout.glyph_advance_px, 16);
        for column in 0..6 {
            layout.validate_ink(column, 345, [0, 0, 20, 20]).unwrap();
        }
        assert!(layout_button(&overlay, 3, 8, 345).is_err());
        assert!(layout_button(&overlay, 3, 6, 360).is_err());
        assert!(layout.validate_ink(5, 345, [0, 0, 30, 20]).is_err());
        assert!(layout_button(&overlay, 3, 0, 345).is_err());
        overlay[0x2c4] ^= 1;
        assert!(layout_button(&overlay, 3, 6, 345).is_err());
    }
}
