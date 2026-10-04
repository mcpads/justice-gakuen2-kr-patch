pub(super) const LOGICAL_SLOT_CAPACITY: usize = 104;
pub(super) const FIRST_SLOT_OFFSET: usize = 0x38;
pub(super) const SLOT_STRIDE: usize = 0x38;
pub(super) const NEXT_FIXED_PRIMITIVE_OFFSET: usize = 0x1708;

// The compact renderer multiplies the logical slot by 56 as
// `((slot << 3) - slot) << 3`. Keep that executable grammar tied to the
// capacity proof instead of duplicating an unconnected literal in each module.
pub(super) const RENDERER_SLOT_SCALE_SHIFT: u8 = 3;
pub(super) const RENDERER_SHIFTED_SLOT_STRIDE: usize =
    ((1 << RENDERER_SLOT_SCALE_SHIFT) - 1) << RENDERER_SLOT_SCALE_SHIFT;

const _: () = assert!(SLOT_STRIDE == RENDERER_SHIFTED_SLOT_STRIDE);
const _: () = assert!(FIRST_SLOT_OFFSET + LOGICAL_SLOT_CAPACITY * SLOT_STRIDE == 0x16f8);
const _: () = assert!(FIRST_SLOT_OFFSET + (LOGICAL_SLOT_CAPACITY + 1) * SLOT_STRIDE == 0x1730);
const _: () =
    assert!(FIRST_SLOT_OFFSET + LOGICAL_SLOT_CAPACITY * SLOT_STRIDE <= NEXT_FIXED_PRIMITIVE_OFFSET);
const _: () = assert!(
    FIRST_SLOT_OFFSET + (LOGICAL_SLOT_CAPACITY + 1) * SLOT_STRIDE > NEXT_FIXED_PRIMITIVE_OFFSET
);
