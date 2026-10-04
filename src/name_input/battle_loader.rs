//! Battle-phase loading. The supplier tail runs before native stage loading reuses RAM.
use super::selector_loader::{overlaps, ram_range};
use anyhow::{Result, ensure};
use psx_r3000a::{Assembler, Instruction::*, Register as R, load_address, verify_placed_program};

pub const BATTLE_NAME_GENERATOR_ORIGIN: u32 = 0x8002_90cc;
pub const BATTLE_NAME_GENERATOR_CAPACITY: usize = 416;
pub const BATTLE_NAME_LOAD_DESTINATION: u32 = 0x800d_4000;
pub const BATTLE_NAME_NATIVE_SUPPLIER_BYTES: usize = 16916;
pub const BATTLE_NAME_FONT_PIXELS: u32 = 0x800e_d2e0;
pub const BATTLE_NAME_LEGACY_STORAGE_BYTES: usize = 11284;
const NATIVE_LOADER: u32 = 0x8001_5414;

/// Generate the replacement entry only; admission must additionally bind the
/// appended supplier payload, compressed record capacity, font cells, GPU
/// allocation and sprite consumers. This constructor does not install a hook.
/// The pre-sampled legacy atlas occupies the supplier prefix; the appended
/// code and the source-bound pixel conversion must be installed together.
pub fn build_battle_name_loader(
    supplier_bytes: usize,
    entry: u32,
    font_decoded_bytes: usize,
) -> Result<Vec<u8>> {
    let code = ram_range(BATTLE_NAME_GENERATOR_ORIGIN, BATTLE_NAME_GENERATOR_CAPACITY)?;
    ensure!(
        (0x33800..=0x38000).contains(&font_decoded_bytes),
        "battle font load exceeds admitted resource extent"
    );
    let font = ram_range(BATTLE_NAME_LOAD_DESTINATION, font_decoded_bytes)?;
    let supplier = ram_range(BATTLE_NAME_LOAD_DESTINATION, supplier_bytes)?;
    ensure!(
        entry.is_multiple_of(4)
            && entry >= BATTLE_NAME_LOAD_DESTINATION + BATTLE_NAME_LEGACY_STORAGE_BYTES as u32
            && entry.checked_add(4).is_some_and(|end| end <= supplier.end)
            && supplier.end <= BATTLE_NAME_FONT_PIXELS
            && !overlaps(&code, &font),
        "battle supplier entry is outside its appended payload or overwrites shared font pixels"
    );
    let mut a = Assembler::new();
    a.emit_all(load_address(R::T0, 0x8009_a978))
        .emit(Lw {
            rt: R::T0,
            base: R::T0,
            offset: 0,
        })
        .emit(psx_r3000a::Instruction::nop())
        .beq(R::T0, R::ZERO, "return")
        .emit(psx_r3000a::Instruction::nop())
        .emit(Addiu {
            rt: R::SP,
            rs: R::SP,
            immediate: -24,
        })
        .emit(Sw {
            rt: R::RA,
            base: R::SP,
            offset: 20,
        });
    // MA_ENT first; COCKEDIT overwrites only its lower prefix. The scattered
    // name pack at FONT_PIXELS and above remains resident for the payload.
    for index in [718, 49] {
        a.emit_all(load_address(R::A0, BATTLE_NAME_LOAD_DESTINATION))
            .emit(Jal {
                target: NATIVE_LOADER,
            })
            .emit(Ori {
                rt: R::A1,
                rs: R::ZERO,
                immediate: index,
            });
    }
    // Execute newly loaded code only after the native instruction-cache flush.
    for target in [0x8006_0058, 0x8006_0008, 0x8006_0068, entry] {
        a.emit(Jal { target }).emit(psx_r3000a::Instruction::nop());
    }
    a.emit(Lw {
        rt: R::RA,
        base: R::SP,
        offset: 20,
    })
    .emit(Addiu {
        rt: R::SP,
        rs: R::SP,
        immediate: 24,
    })
    .label("return")
    .emit(Jr { rs: R::RA })
    .emit(psx_r3000a::Instruction::nop());
    let bytes = a.assemble(BATTLE_NAME_GENERATOR_ORIGIN)?.bytes().to_vec();
    ensure!(
        bytes.len() <= BATTLE_NAME_GENERATOR_CAPACITY,
        "battle loader exceeds native function"
    );
    crate::psx_machine_code_sources::verify_r3000a_load_delays(
        &verify_placed_program(&bytes, BATTLE_NAME_GENERATOR_ORIGIN)?,
        BATTLE_NAME_GENERATOR_ORIGIN,
        "battle name loader",
    )?;
    Ok(bytes)
}

#[cfg(test)]
#[path = "battle_loader_tests.rs"]
mod tests;
