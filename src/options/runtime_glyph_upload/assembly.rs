#[path = "assembly/options.rs"]
mod options;
#[path = "assembly/records.rs"]
mod records;

use anyhow::Result;

use crate::contextual_texture_upload::{
    ContextualTextureUploadProgram, NativeCallTarget, assemble_upload_after_native_call,
};

pub(super) use crate::contextual_texture_upload::VRAM_UPLOAD_ROUTINE_ADDRESS as UPLOAD_ROUTINE_ADDRESS;
pub(super) use options::*;
pub(crate) use records::*;

pub(super) type UploadProgram = ContextualTextureUploadProgram;

pub(super) fn build_upload_after_native_call(
    program_runtime_address: u32,
    native_call_address: u32,
    descriptor_runtime_address: u32,
    entry_count: usize,
    loop_label: &'static str,
    context: &'static str,
) -> Result<UploadProgram> {
    assemble_upload_after_native_call(
        program_runtime_address,
        NativeCallTarget::Address(native_call_address),
        descriptor_runtime_address,
        entry_count,
        loop_label,
        context,
    )
}
