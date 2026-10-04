use psx_r3000a::{Instruction, Register, verify_placed_program};

use super::{
    SHARED_NAME_OUTLINE_RUNTIME_BYTE_CAPACITY, SHARED_NAME_OUTLINE_RUNTIME_ORIGIN,
    build_shared_name_outline_runtime_program,
};

#[test]
fn shared_outline_runtime_is_one_typed_stride_parameterized_program() {
    let program = build_shared_name_outline_runtime_program().unwrap();
    let instructions =
        verify_placed_program(&program.bytes, SHARED_NAME_OUTLINE_RUNTIME_ORIGIN).unwrap();

    assert!(program.bytes.len() <= SHARED_NAME_OUTLINE_RUNTIME_BYTE_CAPACITY);
    assert_eq!(instructions.len(), program.report.typed_instruction_count);
    assert!(program.outline_cleanup_address > program.outline_pixel_address);
    assert_eq!(program.report.row_stride_register, "t8");
    assert!(program.report.fits_main_executable_region);
    assert!(!program.report.installed);
    assert!(!program.report.runtime_execution_verified);
    assert!(instructions.contains(&Instruction::Subu {
        rd: Register::A0,
        rs: Register::A0,
        rt: Register::T8,
    }));
    assert!(instructions.contains(&Instruction::Addu {
        rd: Register::A0,
        rs: Register::A0,
        rt: Register::T8,
    }));
}
