use crate::contextual_texture_upload::{
    ContextualTextureUploadProgram, VRAM_UPLOAD_ROUTINE_ADDRESS, VRAM_UPLOAD_SYNC_ROUTINE_ADDRESS,
};
use anyhow::{Result, ensure};
use psx_r3000a::{Assembler, Instruction::*, Register as R, load_address, verify_placed_program};

pub(super) const MAX_PACKED_BYTES: usize = 104;

// Four local palette indices encode the original GPU nibbles without changing CLUTs.
pub(super) fn pack(payload: &[u8]) -> Result<Vec<u8>> {
    ensure!(
        payload.len() == 200,
        "title glyph is not a 20x20 4-bpp cell"
    );
    let mut palette: Vec<u8> = payload.iter().flat_map(|b| [b & 15, b >> 4]).collect();
    palette.sort_unstable();
    palette.dedup();
    ensure!(
        palette.len() <= 4,
        "title glyph uses more than four palette indices"
    );
    let word = palette
        .iter()
        .enumerate()
        .fold(0u16, |a, (i, v)| a | (u16::from(*v) << (i * 4)));
    let pairs = payload.as_chunks::<2>().0;
    let first = pairs.iter().position(|p| *p != [0, 0]).unwrap_or(0);
    let end = pairs
        .iter()
        .rposition(|p| *p != [0, 0])
        .map_or(1, |i| i + 1);
    let mut output = Vec::with_capacity(MAX_PACKED_BYTES);
    output.extend_from_slice(&word.to_le_bytes());
    output.extend([first as u8, (end - first) as u8]);
    for pair in &pairs[first..end] {
        let mut byte = 0;
        for (i, nibble) in [pair[0] & 15, pair[0] >> 4, pair[1] & 15, pair[1] >> 4]
            .into_iter()
            .enumerate()
        {
            byte |= (palette.iter().position(|v| *v == nibble).unwrap() as u8) << (i * 2);
        }
        output.push(byte);
    }
    output.resize(output.len().next_multiple_of(2), 0);
    Ok(output)
}

// A0 points to palette, zero-prefix length, packed length and visible 2-bpp data.
// A1 is a 200-byte destination. Clear it before decoding the nonzero span.
fn emit_unpack(a: &mut Assembler) {
    a.label("unpack_title_cell")
        .emit(Addu {
            rd: R::T6,
            rs: R::A1,
            rt: R::ZERO,
        })
        .emit(Ori {
            rt: R::T2,
            rs: R::ZERO,
            immediate: 100,
        })
        .label("clear_title_cell")
        .emit(Sh {
            rt: R::ZERO,
            base: R::T6,
            offset: 0,
        })
        .emit(Addiu {
            rt: R::T2,
            rs: R::T2,
            immediate: -1,
        })
        .bne(R::T2, R::ZERO, "clear_title_cell")
        .emit(Addiu {
            rt: R::T6,
            rs: R::T6,
            immediate: 2,
        })
        .emit(Lhu {
            rt: R::T0,
            base: R::A0,
            offset: 0,
        })
        .emit(Lbu {
            rt: R::T1,
            base: R::A0,
            offset: 2,
        })
        .emit(Lbu {
            rt: R::T2,
            base: R::A0,
            offset: 3,
        })
        .emit(Sll {
            rd: R::T1,
            rt: R::T1,
            shift: 1,
        })
        .emit(Addu {
            rd: R::A1,
            rs: R::A1,
            rt: R::T1,
        })
        .emit(Addiu {
            rt: R::A0,
            rs: R::A0,
            immediate: 4,
        })
        .label("title_packed_byte")
        .emit(Lbu {
            rt: R::T1,
            base: R::A0,
            offset: 0,
        })
        .emit(Ori {
            rt: R::T3,
            rs: R::ZERO,
            immediate: 2,
        })
        .label("title_pixel_pair")
        .emit(Andi {
            rt: R::T4,
            rs: R::T1,
            immediate: 3,
        })
        .emit(Sll {
            rd: R::T4,
            rt: R::T4,
            shift: 2,
        })
        .emit(Srlv {
            rd: R::T4,
            rt: R::T0,
            rs: R::T4,
        })
        .emit(Andi {
            rt: R::T4,
            rs: R::T4,
            immediate: 15,
        })
        .emit(Srl {
            rd: R::T5,
            rt: R::T1,
            shift: 2,
        })
        .emit(Andi {
            rt: R::T5,
            rs: R::T5,
            immediate: 3,
        })
        .emit(Sll {
            rd: R::T5,
            rt: R::T5,
            shift: 2,
        })
        .emit(Srlv {
            rd: R::T5,
            rt: R::T0,
            rs: R::T5,
        })
        .emit(Andi {
            rt: R::T5,
            rs: R::T5,
            immediate: 15,
        })
        .emit(Sll {
            rd: R::T5,
            rt: R::T5,
            shift: 4,
        })
        .emit(Or {
            rd: R::T4,
            rs: R::T4,
            rt: R::T5,
        })
        .emit(Sb {
            rt: R::T4,
            base: R::A1,
            offset: 0,
        })
        .emit(Srl {
            rd: R::T1,
            rt: R::T1,
            shift: 4,
        })
        .emit(Addiu {
            rt: R::A1,
            rs: R::A1,
            immediate: 1,
        })
        .emit(Addiu {
            rt: R::T3,
            rs: R::T3,
            immediate: -1,
        })
        .bne(R::T3, R::ZERO, "title_pixel_pair")
        .emit(psx_r3000a::Instruction::nop())
        .emit(Addiu {
            rt: R::T2,
            rs: R::T2,
            immediate: -1,
        })
        .bne(R::T2, R::ZERO, "title_packed_byte")
        .emit(Addiu {
            rt: R::A0,
            rs: R::A0,
            immediate: 1,
        })
        .emit(Jr { rs: R::RA })
        .emit(psx_r3000a::Instruction::nop());
}

pub(super) fn build_uploader(
    origin: u32,
    native: u32,
    descriptors: u32,
    count: usize,
) -> Result<ContextualTextureUploadProgram> {
    ensure!(count > 0, "title upload has no glyphs");
    let mut a = Assembler::new();
    a.emit(Addiu {
        rt: R::SP,
        rs: R::SP,
        immediate: -240,
    });
    for (r, offset) in [(R::RA, 236), (R::S0, 232), (R::S1, 228)] {
        a.emit(Sw {
            rt: r,
            base: R::SP,
            offset,
        });
    }
    a.emit(Jal { target: native })
        .emit(psx_r3000a::Instruction::nop())
        .emit(Sw {
            rt: R::V0,
            base: R::SP,
            offset: 224,
        })
        .emit_all(load_address(R::S0, descriptors))
        .emit(Ori {
            rt: R::S1,
            rs: R::ZERO,
            immediate: u16::try_from(count)?,
        })
        .label("upload_title_cell")
        .emit(Lw {
            rt: R::A0,
            base: R::S0,
            offset: 8,
        })
        .call("unpack_title_cell")
        .emit(Addiu {
            rt: R::A1,
            rs: R::SP,
            immediate: 16,
        })
        .emit(Addu {
            rd: R::A0,
            rs: R::S0,
            rt: R::ZERO,
        })
        .emit(Jal {
            target: VRAM_UPLOAD_ROUTINE_ADDRESS,
        })
        .emit(Addiu {
            rt: R::A1,
            rs: R::SP,
            immediate: 16,
        })
        // The upload may queue DMA. Do not reuse the scratch cell before DrawSync.
        .emit(Jal {
            target: VRAM_UPLOAD_SYNC_ROUTINE_ADDRESS,
        })
        .emit(Addu {
            rd: R::A0,
            rs: R::ZERO,
            rt: R::ZERO,
        })
        .emit(Addiu {
            rt: R::S1,
            rs: R::S1,
            immediate: -1,
        })
        .bne(R::S1, R::ZERO, "upload_title_cell")
        .emit(Addiu {
            rt: R::S0,
            rs: R::S0,
            immediate: 12,
        })
        .emit(Lw {
            rt: R::V0,
            base: R::SP,
            offset: 224,
        });
    for (r, offset) in [(R::RA, 236), (R::S0, 232), (R::S1, 228)] {
        a.emit(Lw {
            rt: r,
            base: R::SP,
            offset,
        });
    }
    a.emit(Jr { rs: R::RA }).emit(Addiu {
        rt: R::SP,
        rs: R::SP,
        immediate: 240,
    });
    emit_unpack(&mut a);
    let p = a.assemble(origin)?;
    ensure!(
        verify_placed_program(p.bytes(), origin)? == p.instructions(),
        "title packed upload typed readback changed"
    );
    Ok(ContextualTextureUploadProgram {
        bytes: p.bytes().to_vec(),
        typed_instruction_count: p.instructions().len(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn emitted_unpack_preserves_all_gpu_nibbles_and_memory_bounds() {
        let mut a = Assembler::new();
        emit_unpack(&mut a);
        let p = a.assemble(0x80080000).unwrap();
        let mut payloads = Vec::new();
        for palette in [
            [0, 3, 14, 15],
            [1, 4, 8, 12],
            [0, 0, 0, 0],
            [15, 15, 15, 15],
        ] {
            let payload: Vec<u8> = (0..200)
                .map(|i| palette[i % 4] | palette[(i / 4) % 4] << 4)
                .collect();
            payloads.push(payload);
        }
        for range in [1..199, 2..4, 50..150, 198..200] {
            let mut payload = vec![0; 200];
            payload[range].fill(0xe3);
            assert!(pack(&payload).unwrap().len() <= MAX_PACKED_BYTES);
            payloads.push(payload);
        }
        for payload in payloads {
            let packed = pack(&payload).unwrap();
            assert!(packed.len() <= MAX_PACKED_BYTES);
            let mut mem = vec![0xa5; 0x4000];
            mem[0x1000..0x1000 + packed.len()].copy_from_slice(&packed);
            let before = mem.clone();
            let mut r = [0; 32];
            r[4] = 0x80001000;
            r[5] = 0x80002000;
            crate::name_input::runtime_test_machine::execute(
                p.bytes(),
                0x80080000,
                &mut r,
                &mut mem,
            );
            assert_eq!(&mem[0x2000..0x20c8], payload);
            assert_eq!(&mem[..0x2000], &before[..0x2000]);
            assert_eq!(&mem[0x20c8..], &before[0x20c8..]);
        }
        let mut five = vec![0; 200];
        five[..3].copy_from_slice(&[0x10, 0x32, 0x44]);
        assert!(pack(&five).is_err());
    }
}

#[cfg(test)]
mod upload_tests {
    use super::*;

    #[test]
    fn queued_uploads_finish_before_scratch_reuse_and_preserve_native_return() {
        let native = 0x80040000;
        let descriptors = 0x80001000;
        let program = build_uploader(0x80080000, native, descriptors, 3).unwrap();
        let payloads = [vec![0xe3; 200], vec![0x0e; 200], vec![0x30; 200]];
        let mut memory = vec![0xa5; 0x10000];
        for (i, payload) in payloads.iter().enumerate() {
            let packed = pack(payload).unwrap();
            let data = 0x2000 + i * MAX_PACKED_BYTES;
            memory[data..data + packed.len()].copy_from_slice(&packed);
            memory[0x1000 + i * 12 + 8..0x1000 + i * 12 + 12]
                .copy_from_slice(&(0x80000000 + data as u32).to_le_bytes());
        }
        let mut registers = [0; 32];
        registers[16] = 0x11223344;
        registers[17] = 0x55667788;
        registers[29] = 0x80008000;
        let mut pending: Option<(usize, Vec<u8>)> = None;
        let mut native_calls = 0;
        let mut uploaded = 0;
        let mut synced = 0;
        crate::name_input::runtime_test_machine::execute_with_callbacks(
            &program.bytes,
            0x80080000,
            &mut registers,
            &mut memory,
            None,
            &mut |pc, r, mem| {
                if let Some((address, snapshot)) = &pending {
                    assert_eq!(&mem[*address..*address + 200], snapshot);
                }
                match pc {
                    p if p == native => {
                        native_calls += 1;
                        r[2] = 0x12345678;
                    }
                    VRAM_UPLOAD_ROUTINE_ADDRESS => {
                        assert!(pending.is_none());
                        assert_eq!(r[4], descriptors + uploaded as u32 * 12);
                        let address = (r[5] & 0x1fffffff) as usize;
                        assert_eq!(&mem[address..address + 200], payloads[uploaded]);
                        pending = Some((address, mem[address..address + 200].to_vec()));
                        uploaded += 1;
                    }
                    VRAM_UPLOAD_SYNC_ROUTINE_ADDRESS => {
                        assert_eq!(r[4], 0);
                        assert!(pending.take().is_some());
                        synced += 1;
                    }
                    _ => return false,
                }
                // o32 callees may overwrite the outgoing argument area and scratch registers.
                let sp = (r[29] & 0x1fffffff) as usize;
                mem[sp..sp + 16].fill(0xcc);
                for index in [3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 24, 25] {
                    r[index] = 0xdead0000 + index as u32;
                }
                true
            },
        );
        assert_eq!((native_calls, uploaded, synced), (1, 3, 3));
        assert!(pending.is_none());
        assert_eq!(registers[2], 0x12345678);
        assert_eq!(registers[16], 0x11223344);
        assert_eq!(registers[17], 0x55667788);
        assert_eq!(registers[29], 0x80008000);
    }
}
