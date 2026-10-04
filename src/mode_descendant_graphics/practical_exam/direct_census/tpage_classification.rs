use super::*;

pub(super) fn validate_get_tpage_classifications(
    overlay: &[u8],
    consumer: PracticalExamConsumer,
    profile: &DirectCensusProfile,
) -> Result<()> {
    for site in profile.shared_calls {
        ensure_argument_literal(overlay, site.call_offset, Register::A0, 0)?;
        ensure_argument_literal(overlay, site.call_offset, Register::A1, 0)?;
        match site.kind {
            SharedCallKind::PrimaryDescriptor => {
                ensure_argument_literal(overlay, site.call_offset, Register::A3, 0)?;
            }
            SharedCallKind::SecondaryDescriptorBankSelected => {
                ensure!(
                    argument_literal(overlay, site.call_offset, Register::A3)?.is_none(),
                    "{consumer:?} secondary practical-exam texture bank stopped being descriptor-selected at +0x{:04x}",
                    site.call_offset
                );
            }
            SharedCallKind::Direct { texture_x } => {
                ensure_argument_literal(overlay, site.call_offset, Register::A2, texture_x)?;
                ensure_argument_literal(overlay, site.call_offset, Register::A3, 0)?;
            }
        }
    }

    let mut non_shared = NonSharedCounts {
        external_y_256: 0,
        outside_producer_x: 0,
    };
    let mut found_eight_bit_calls = BTreeSet::new();
    for call_offset in profile.get_tpage_calls {
        if profile
            .shared_calls
            .iter()
            .any(|site| site.call_offset == *call_offset)
        {
            continue;
        }
        let pixel_mode = required_argument_literal(overlay, *call_offset, Register::A0)?;
        if pixel_mode != 0 {
            ensure!(
                pixel_mode == 1 && profile.eight_bit_calls.contains(call_offset),
                "{consumer:?} practical-exam GetTPage site +0x{call_offset:04x} has unclassified pixel mode {pixel_mode}"
            );
            ensure!(
                found_eight_bit_calls.insert(*call_offset),
                "{consumer:?} practical-exam 8bpp GetTPage site was counted twice"
            );
            continue;
        }

        let texture_y = required_argument_literal(overlay, *call_offset, Register::A3)?;
        if texture_y == 0x100 {
            non_shared.external_y_256 += 1;
            continue;
        }
        ensure!(
            texture_y == 0,
            "{consumer:?} practical-exam GetTPage site +0x{call_offset:04x} has unclassified texture y {texture_y:#x}"
        );
        let texture_x = required_argument_literal(overlay, *call_offset, Register::A2)?;
        ensure!(
            texture_x == 0,
            "{consumer:?} practical-exam GetTPage site +0x{call_offset:04x} reaches uncatalogued shared-y0 x {texture_x:#x}"
        );
        non_shared.outside_producer_x += 1;
    }

    ensure!(
        non_shared.external_y_256 == profile.expected_non_shared.external_y_256
            && non_shared.outside_producer_x == profile.expected_non_shared.outside_producer_x,
        "{consumer:?} practical-exam non-shared GetTPage classification changed"
    );
    ensure!(
        found_eight_bit_calls
            == profile
                .eight_bit_calls
                .iter()
                .copied()
                .collect::<BTreeSet<_>>(),
        "{consumer:?} practical-exam 8bpp GetTPage denominator changed"
    );
    ensure!(
        profile.shared_calls.len()
            + non_shared.external_y_256
            + found_eight_bit_calls.len()
            + non_shared.outside_producer_x
            == profile.get_tpage_calls.len(),
        "{consumer:?} practical-exam GetTPage classification omitted a call site"
    );
    Ok(())
}
