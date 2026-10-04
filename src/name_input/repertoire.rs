use anyhow::{Context, Result, ensure};
use encoding_rs::EUC_KR;
use std::sync::OnceLock;

use super::glyph_pack::MODERN_HANGUL_START;

pub const KS_X_1001_HANGUL_COUNT: usize = 2_350;

static REPERTOIRE: OnceLock<Vec<char>> = OnceLock::new();

pub fn load_ks_x_1001_hangul() -> Result<Vec<char>> {
    Ok(repertoire()?.clone())
}

pub fn is_ks_x_1001_hangul(character: char) -> bool {
    repertoire()
        .expect("the fixed KS X 1001 Hangul table must decode")
        .binary_search(&character)
        .is_ok()
}

fn repertoire() -> Result<&'static Vec<char>> {
    if let Some(characters) = REPERTOIRE.get() {
        return Ok(characters);
    }
    let mut characters = Vec::with_capacity(KS_X_1001_HANGUL_COUNT);
    for lead in 0xb0..=0xc8 {
        for trail in 0xa1..=0xfe {
            let encoded = [lead, trail];
            let decoded = EUC_KR
                .decode_without_bom_handling_and_without_replacement(&encoded)
                .with_context(|| format!("KS X 1001 code {lead:02X}{trail:02X} did not decode"))?;
            let mut decoded_characters = decoded.chars();
            let character = decoded_characters
                .next()
                .context("KS X 1001 Hangul code decoded empty")?;
            ensure!(
                decoded_characters.next().is_none(),
                "KS X 1001 Hangul code decoded to more than one character"
            );
            characters.push(character);
        }
    }
    characters.sort_unstable();
    ensure!(
        characters.len() == KS_X_1001_HANGUL_COUNT
            && characters.windows(2).all(|pair| pair[0] < pair[1]),
        "KS X 1001 Hangul repertoire is not 2,350 unique syllables"
    );
    ensure!(
        characters
            .iter()
            .all(|character| u32::from(*character) >= MODERN_HANGUL_START),
        "KS X 1001 Hangul repertoire contains a non-Hangul character"
    );
    let _ = REPERTOIRE.set(characters);
    Ok(REPERTOIRE
        .get()
        .expect("KS X 1001 repertoire was initialized"))
}

// Include the no-final predecessors needed to reach every admitted KS syllable.
// A visible intermediate is also a valid name character across all consumers.
pub fn load_name_input_hangul() -> Result<Vec<char>> {
    let mut characters = load_ks_x_1001_hangul()?;
    let predecessors = characters
        .iter()
        .map(|&c| {
            let scalar = c as u32;
            char::from_u32(scalar - (scalar - 0xac00) % 28).unwrap()
        })
        .collect::<Vec<_>>();
    characters.extend(predecessors);
    characters.sort_unstable();
    characters.dedup();
    Ok(characters)
}

pub fn is_name_input_hangul(character: char) -> bool {
    static INPUT_REPERTOIRE: OnceLock<Vec<char>> = OnceLock::new();
    INPUT_REPERTOIRE
        .get_or_init(|| load_name_input_hangul().expect("valid name repertoire"))
        .binary_search(&character)
        .is_ok()
}
