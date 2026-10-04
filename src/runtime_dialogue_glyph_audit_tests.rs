use std::collections::HashMap;

use super::{
    DR_TPAGE_PACKET_BYTES, GLYPH_BYTES, GLYPH_HEIGHT_PIXELS, GLYPH_ROW_BYTES, PSX_RAM_BYTES,
    RuntimeSourceGlyph, VRAM_HEIGHT, VRAM_ROW_STRIDE_BYTES, audit_active_runtime_image,
    hash_vram_glyph, scan_draw_consumers, sha256_digest,
};

#[test]
fn vram_glyph_hash_uses_the_ps1_row_stride() {
    let mut gpu = vec![0u8; VRAM_ROW_STRIDE_BYTES * VRAM_HEIGHT];
    let word_x = 17;
    let y = 23;
    let mut expected = Vec::with_capacity(GLYPH_BYTES);
    for row in 0..GLYPH_HEIGHT_PIXELS {
        let bytes = [row as u8; GLYPH_ROW_BYTES];
        let start = (y + row) * VRAM_ROW_STRIDE_BYTES + word_x * 2;
        gpu[start..start + GLYPH_ROW_BYTES].copy_from_slice(&bytes);
        expected.extend_from_slice(&bytes);
    }
    gpu[y * VRAM_ROW_STRIDE_BYTES + word_x * 2 + GLYPH_ROW_BYTES..]
        .iter_mut()
        .take(GLYPH_BYTES)
        .for_each(|byte| *byte = 0xff);

    assert_eq!(hash_vram_glyph(&gpu, word_x, y), sha256_digest(&expected));
}

#[test]
fn adjacent_draw_mode_and_sprite_packets_resolve_both_cells_of_a_heading() {
    let mut gpu = vec![0u8; VRAM_ROW_STRIDE_BYTES * VRAM_HEIGHT];
    let first = [0x21u8; GLYPH_BYTES];
    let second = [0x43u8; GLYPH_BYTES];
    install_glyph(&mut gpu, 814, 176, &first);
    install_glyph(&mut gpu, 819, 176, &second);
    let mut source_glyphs = HashMap::new();
    source_glyphs.insert(sha256_digest(&first), vec![glyph("0x017b")]);
    source_glyphs.insert(sha256_digest(&second), vec![glyph("0x017c")]);

    let mut ram = vec![0u8; PSX_RAM_BYTES];
    let packet = 0x1200;
    ram[packet - 4..packet].copy_from_slice(&0xe100_020cu32.to_le_bytes());
    ram[packet + 4..packet + 8].copy_from_slice(&0x6480_8080u32.to_le_bytes());
    ram[packet + 8..packet + 10].copy_from_slice(&84i16.to_le_bytes());
    ram[packet + 10..packet + 12].copy_from_slice(&108i16.to_le_bytes());
    ram[packet + 12] = 184;
    ram[packet + 13] = 176;
    ram[packet + 14..packet + 16].copy_from_slice(&0x7a54u16.to_le_bytes());
    ram[packet + 16..packet + 18].copy_from_slice(&40u16.to_le_bytes());
    ram[packet + 18..packet + 20].copy_from_slice(&20u16.to_le_bytes());

    let consumers = scan_draw_consumers(&ram, &gpu, &source_glyphs);

    assert_eq!(consumers.len(), 2);
    assert_eq!(consumers[0].texture_page_word_x, 768);
    assert_eq!(consumers[0].texture_vram_word_x, 814);
    assert_eq!(consumers[0].screen_x, 84);
    assert_eq!(consumers[1].texture_vram_word_x, 819);
    assert_eq!(consumers[1].screen_x, 104);
    assert_eq!(consumers[0].packet_addresses, vec!["0x80001200"]);
}

#[test]
fn sprite_without_an_adjacent_draw_mode_is_not_called_a_consumer() {
    let gpu = vec![0u8; VRAM_ROW_STRIDE_BYTES * VRAM_HEIGHT];
    let mut ram = vec![0u8; PSX_RAM_BYTES];
    let packet = DR_TPAGE_PACKET_BYTES;
    ram[packet + 7] = 0x64;
    ram[packet + 16..packet + 18].copy_from_slice(&20u16.to_le_bytes());
    ram[packet + 18..packet + 20].copy_from_slice(&20u16.to_le_bytes());

    assert!(scan_draw_consumers(&ram, &gpu, &HashMap::new()).is_empty());
}

#[test]
fn active_dialogue_image_binding_compares_the_complete_runtime_image() {
    let mut ram = vec![0u8; PSX_RAM_BYTES];
    let decoded = vec![0x5au8; 4096];
    ram[0x0d_0000..0x0d_0000 + decoded.len()].copy_from_slice(&decoded);

    let exact = audit_active_runtime_image(&ram, &decoded).unwrap();
    assert!(exact.matches);
    assert_eq!(exact.difference_count, 0);

    ram[0x0d_0123] ^= 0xff;
    let changed = audit_active_runtime_image(&ram, &decoded).unwrap();
    assert!(!changed.matches);
    assert_eq!(changed.difference_count, 1);
}

fn install_glyph(gpu: &mut [u8], word_x: usize, y: usize, glyph: &[u8; GLYPH_BYTES]) {
    for row in 0..GLYPH_HEIGHT_PIXELS {
        let source_start = row * GLYPH_ROW_BYTES;
        let target_start = (y + row) * VRAM_ROW_STRIDE_BYTES + word_x * 2;
        gpu[target_start..target_start + GLYPH_ROW_BYTES]
            .copy_from_slice(&glyph[source_start..source_start + GLYPH_ROW_BYTES]);
    }
}

fn glyph(code: &str) -> RuntimeSourceGlyph {
    RuntimeSourceGlyph {
        source_code: code.to_string(),
        source_text: None,
        source_semantic_id: None,
        codebook_status: None,
        current_character: None,
        same_code_allocation_relation: "source_code_not_assigned".to_string(),
        source_text_uses_japanese_script: false,
    }
}
