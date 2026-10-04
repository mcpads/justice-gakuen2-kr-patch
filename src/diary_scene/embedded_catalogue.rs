use anyhow::{Result, ensure};

// Match a complete source catalogue image, not just its label or TIM header.
// Offset zero is already owned by the runtime-prefix producer.
pub(super) fn replacements(
    decoded: &[u8],
    sources: &[Vec<u8>],
    patched: &[Vec<u8>],
) -> Result<Vec<(usize, usize)>> {
    ensure!(
        sources.len() == patched.len(),
        "catalogue population changed"
    );
    let mut matches = Vec::<(usize, usize)>::new();
    for (offset, header) in decoded.windows(8).enumerate().skip(1) {
        if !offset.is_multiple_of(4) || header != [0x10, 0, 0, 0, 9, 0, 0, 0] {
            continue;
        }
        let mut selected = None;
        for (index, (source, replacement)) in sources.iter().zip(patched).enumerate() {
            if source == replacement || !decoded[offset..].starts_with(source) {
                continue;
            }
            ensure!(
                source.len() == replacement.len(),
                "embedded catalogue extent changed"
            );
            if let Some(previous) = selected {
                ensure!(
                    patched[previous] == *replacement,
                    "aliased embedded catalogue owners disagree at {offset:#x}"
                );
            } else {
                selected = Some(index);
            }
        }
        if let Some(index) = selected {
            if let Some(&(previous_offset, previous_index)) = matches.last() {
                ensure!(
                    previous_offset + sources[previous_index].len() <= offset,
                    "embedded catalogue images overlap"
                );
            }
            matches.push((offset, index));
        }
    }
    Ok(matches)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(tail: u8) -> Vec<u8> {
        vec![0x10, 0, 0, 0, 9, 0, 0, 0, tail, tail, tail, tail]
    }

    #[test]
    fn finds_later_images_without_reclaiming_the_prefix_or_partial_matches() {
        let source = image(3);
        let mut decoded = source.clone();
        decoded.extend(image(7));
        decoded.extend(&source);
        decoded.extend(&source[..8]);
        assert_eq!(
            replacements(&decoded, &[source], &[image(4)]).unwrap(),
            vec![(24, 0)]
        );
    }

    #[test]
    fn identical_aliases_are_coalesced_and_conflicting_translations_are_rejected() {
        let source = image(3);
        let mut decoded = vec![0; 4];
        decoded.extend(&source);
        assert_eq!(
            replacements(
                &decoded,
                &[source.clone(), source.clone()],
                &[image(4), image(4)]
            )
            .unwrap(),
            vec![(4, 0)]
        );
        assert!(replacements(&decoded, &[source.clone(), source], &[image(4), image(5)]).is_err());
    }
}
