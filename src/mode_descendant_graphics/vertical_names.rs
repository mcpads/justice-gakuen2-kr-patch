//! Fixed HUD names share native cells across costume variants.
use anyhow::{Result, bail, ensure};

pub(super) type NameCell = (
    &'static str,
    &'static str,
    &'static str,
    usize,
    usize,
    usize,
    usize,
    usize,
);

pub(super) fn cell(id: u8) -> Result<NameCell> {
    Ok(match id {
        1 => ("hud-name-kyosuke", "恭介", "쿄스케", 144, 160, 24, 40, 768),
        2 => ("hud-name-batsu", "バツ", "바츠", 48, 152, 24, 32, 768),
        3 => ("hud-name-hinata", "ひなた", "히나타", 0, 208, 24, 48, 768),
        4 => ("hud-name-shoma", "将馬", "쇼마", 24, 152, 24, 32, 768),
        5 => ("hud-name-natsu", "夏", "나츠", 0, 184, 24, 24, 768),
        6 => (
            "hud-name-roberto",
            "ロベルト",
            "로베르토",
            144,
            200,
            24,
            48,
            768,
        ),
        7 => ("hud-name-roy", "ロイ", "로이", 96, 160, 24, 32, 768),
        8 => (
            "hud-name-tiffany",
            "ティファニー",
            "티파니",
            96,
            192,
            24,
            64,
            768,
        ),
        9 => ("hud-name-bowman", "ボーマン", "보먼", 48, 184, 24, 24, 768),
        10 => ("hud-name-edge", "エッジ", "에지", 168, 160, 24, 40, 768),
        11 => ("hud-name-akira", "アキラ", "아키라", 192, 160, 24, 48, 768),
        12 => ("hud-name-gan", "岩", "간", 72, 192, 24, 24, 768),
        13 => ("hud-name-hideo", "英雄", "히데오", 216, 160, 24, 32, 768),
        14 => ("hud-name-kyoko", "響子", "쿄코", 72, 152, 24, 40, 768),
        15 => ("hud-name-raizo", "雷蔵", "라이조", 72, 216, 24, 40, 768),
        16 => ("hud-name-hyo", "雹", "효", 24, 184, 24, 24, 768),
        18 => ("hud-name-sakura", "さくら", "사쿠라", 24, 208, 24, 48, 768),
        19 => ("hud-name-daigo", "醍醐", "다이고", 0, 152, 24, 32, 768),
        20 => (
            "hud-name-hayato",
            "熱血隼人",
            "열혈하야토",
            120,
            192,
            24,
            64,
            768,
        ),
        25 => ("hud-name-ran", "ラン", "란", 120, 160, 24, 32, 768),
        26 => ("hud-name-nagare", "流", "나가레", 48, 208, 24, 48, 768),
        _ => bail!("HUD name must use a canonical fixed character ID"),
    })
}

const TABLE: [u8; 93] = [
    0, 168, 32, 144, 160, 40, 48, 152, 32, 0, 208, 48, 24, 152, 32, 0, 184, 24, 144, 200, 48, 96,
    160, 32, 96, 192, 64, 48, 208, 48, 168, 160, 40, 192, 160, 48, 72, 192, 24, 216, 160, 32, 72,
    152, 40, 72, 216, 40, 24, 184, 24, 192, 160, 48, 24, 208, 48, 0, 152, 32, 120, 192, 64, 0, 208,
    48, 0, 184, 24, 96, 192, 64, 72, 152, 40, 120, 160, 32, 48, 184, 24, 0, 64, 32, 0, 64, 32, 0,
    64, 32, 0, 64, 32,
];
pub(super) fn validate_native_table(source: &[u8]) -> Result<()> {
    // 0x8005d270/2d0/2e4 read u, v and height; sprite width is 24.
    // ID 0 overlaps Daigo/Natsu; IDs 27..30 are dynamic EDIT names.
    ensure!(
        source.get(0x7f5e8..0x7f5e8 + TABLE.len()) == Some(TABLE.as_slice()),
        "native HUD name table changed"
    );
    Ok(())
}

pub(super) fn validate_selection(ids: &[u8]) -> Result<bool> {
    ensure!(
        ids.contains(&9) == ids.contains(&26),
        "repacked Bowman and Nagare require both artwork cells"
    );
    Ok(ids.contains(&9))
}

pub(super) fn layout_candidate(source: &[u8], ids: &[u8]) -> Result<Option<Vec<u8>>> {
    validate_native_table(source)?;
    if !validate_selection(ids)? {
        return Ok(None);
    }
    let mut candidate = source.to_vec();
    for id in [9u8, 26] {
        let (_, _, _, x, y, _, height, _) = cell(id)?;
        let offset = 0x7f5e8 + usize::from(id) * 3;
        candidate[offset..offset + 3].copy_from_slice(&[x as u8, y as u8, height as u8]);
    }
    Ok(Some(candidate))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paired_names_reuse_only_their_original_cells() {
        let mut source = vec![0xa5; 0x80000];
        source[0x7f5e8..0x7f5e8 + TABLE.len()].copy_from_slice(&TABLE);
        assert!(layout_candidate(&source, &[9]).is_err());
        assert!(layout_candidate(&source, &[26]).is_err());
        assert!(layout_candidate(&source, &[2]).unwrap().is_none());
        let candidate = layout_candidate(&source, &[9, 26]).unwrap().unwrap();
        assert_eq!(&candidate[0x7f603..0x7f606], &[48, 184, 24]);
        assert_eq!(&candidate[0x7f636..0x7f639], &[48, 208, 48]);
        for (i, (&a, &b)) in source.iter().zip(&candidate).enumerate() {
            if a != b {
                assert!((0x7f603..0x7f606).contains(&i) || (0x7f636..0x7f639).contains(&i));
            }
        }
        source[0x7f636] ^= 1;
        assert!(layout_candidate(&source, &[9, 26]).is_err());
    }

    #[test]
    fn aliases_and_dynamic_names_cannot_be_authored_as_separate_cells() {
        for id in [0, 17, 21, 22, 23, 24, 27, 28, 29, 30, 31, 255] {
            assert!(cell(id).is_err(), "unexpected fixed writer for ID {id}");
        }
        let cells: Vec<_> = (0..=30).filter_map(|id| cell(id).ok()).collect();
        for (i, a) in cells.iter().enumerate() {
            assert!(a.3 + a.5 <= 256 && a.4 >= 152 && a.4 + a.6 <= 256);
            for b in &cells[i + 1..] {
                assert!(
                    a.3 + a.5 <= b.3 || b.3 + b.5 <= a.3 || a.4 + a.6 <= b.4 || b.4 + b.6 <= a.4,
                    "fixed names overlap: {} and {}",
                    a.0,
                    b.0
                );
            }
        }
    }
}
