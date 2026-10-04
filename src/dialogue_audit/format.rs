pub(super) fn hex_offset(offset: usize) -> String {
    format!("0x{offset:05x}")
}

pub(super) fn hex_address(address: u32) -> String {
    format!("0x{address:08x}")
}

pub(super) fn hex_code(code: u16) -> String {
    format!("0x{code:04x}")
}
