//! PASS owns a separate four-button list. Sharing its text with KANRI does
//! not share the placement descriptors or the renderer's tracking value.
use super::button_layout::{ButtonLayout, layout_button_bounds};
use crate::pipeline::sha256_bytes;
use anyhow::{Context, Result, ensure};

pub(super) fn layout(
    source: &[u8],
    id: &str,
    count: usize,
) -> Result<Option<(usize, ButtonLayout)>> {
    let index = match id {
        "pass_cpu_skill_usage" => 0,
        "pass_cpu_password_display" => 1,
        "pass_cpu_password_input" => 2,
        "pass_cpu_back" => 3,
        _ => return Ok(None),
    };
    // Both selected and idle branches walk the same four source sprites and
    // 12-byte text descriptors. The text loop advances by 20 - descriptor[8].
    for (start, end, hash) in [
        (
            0x6300,
            0x64dc,
            "3ffa7d54972d78221dad46bcc323eaaea00fb8afabf36d9e65ee34fb9382ccb8",
        ),
        (
            0x0f88,
            0x1180,
            "7e9d26f8cbc49f8be76880cc995a6eceaa1e2802455ceda42c46a04623d662ca",
        ),
        (
            0x6870,
            0x69b4,
            "4d942fe6f7bdc93ea79069714b6ab5f5f181163417c2786f453a5fd2eb6ad481",
        ),
    ] {
        ensure!(
            sha256_bytes(
                source
                    .get(start..end)
                    .context("truncated PASS button consumer")?
            ) == hash,
            "PASS button consumer changed"
        );
    }
    let pointer = 0x22c + 4 * index;
    let offset = usize::try_from(
        u32::from_le_bytes(source[pointer..pointer + 4].try_into()?)
            .checked_sub(0x8017_a000)
            .context("PASS button pointer below overlay")?,
    )?;
    ensure!(
        offset == 0x1b4 + 20 * index,
        "PASS button sprite pointer changed"
    );
    let half = |at: usize| i16::from_le_bytes([source[at], source[at + 1]]);
    let placement = 0x420 + 12 * index;
    ensure!(
        half(placement + 8) == 0,
        "PASS source button tracking changed"
    );
    let bounds = [
        half(offset + 16),
        half(offset + 18),
        half(offset + 12),
        half(offset + 14),
    ];
    let result = layout_button_bounds(bounds, count, half(placement + 2))?;
    for column in 0..count {
        result.validate_ink(column, half(placement + 2), [0, 0, 20, 20])?;
    }
    Ok(Some((placement, result)))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    #[ignore = "requires the private original disc; emits no ROM"]
    fn source_cpu_buttons_fit_all_authored_labels_and_reject_consumer_drift() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let disc = crate::source_disc::SupportedSourceDisc::open(
            &root.join("roms/Shiritsu Justice Gakuen - Nekketsu Seishun Nikki 2 (Japan).cue"),
        )
        .unwrap();
        let (_, source) = disc.read_record("DAT1/PASS.BIN").unwrap();
        for (id, count) in [
            ("pass_cpu_skill_usage", 5),
            ("pass_cpu_password_display", 6),
            ("pass_cpu_password_input", 4),
            ("pass_cpu_back", 2),
        ] {
            let (_, result) = layout(&source, id, count).unwrap().unwrap();
            assert_eq!(result.glyph_advance_px, 16);
            assert!(layout(&source, id, 8).is_err());
        }
        let (_, display) = layout(&source, "pass_cpu_password_display", 6)
            .unwrap()
            .unwrap();
        assert_eq!(display.x, 381);
        assert!(380 + 5 * 20 + 20 > display.bounds[2]); // reproduced prior full-cell overflow
        let mut changed = source.clone();
        changed[0x1168] ^= 1;
        assert!(layout(&changed, "pass_cpu_password_display", 6).is_err());
    }
}
