use super::*;

pub(super) const OVERLAY_RUNTIME_BASE: u32 = 0x800a_2000;
pub(super) const GET_TPAGE_TABLE_ADDRESS: u32 = 0x801f_6370;
pub(super) const GET_TPAGE_FUNCTION_OFFSET: i16 = 0x18;
pub(super) const GPU_HELPER_TABLE_ADDRESS: u32 = 0x801f_6360;
pub(super) const SET_DRAW_MODE_FUNCTION_OFFSET: i16 = 0x2c;
pub(super) const SET_DRAW_TPAGE_FUNCTION_OFFSET: i16 = 0x34;
pub(super) const ADD_PRIM_FUNCTION_OFFSET: i16 = 0x00;
pub(super) const CAT_PRIM_FUNCTION_OFFSET: i16 = 0x20;
pub(super) const SET_SEMI_TRANS_FUNCTION_OFFSET: i16 = 0x64;
pub(super) const PRIMITIVE_CONSTRUCTOR_OFFSETS: [i16; 2] = [0x68, 0x6c];
pub(super) const SET_SPRT_FUNCTION_OFFSET: i16 = 0x68;
pub(super) const ARGUMENT_SCAN_INSTRUCTION_LIMIT: usize = 96;
pub(super) const SHARED_PRODUCER_VRAM_X_START: u32 = 0x300;
pub(super) const SHARED_PRODUCER_VRAM_X_END: u32 = 0x3c0;
pub(super) const EIGHT_BIT_TPAGE_VRAM_WIDTH: u32 = 0x80;

#[derive(Clone, Copy)]
pub(super) enum SharedCallKind {
    PrimaryDescriptor,
    SecondaryDescriptorBankSelected,
    Direct { texture_x: u32 },
}

#[derive(Clone, Copy)]
pub(super) struct SharedCallSite {
    pub(super) call_offset: usize,
    pub(super) kind: SharedCallKind,
}

#[derive(Clone, Copy)]
pub(super) struct NonSharedCounts {
    pub(super) external_y_256: usize,
    pub(super) outside_producer_x: usize,
}

pub(super) struct DirectCensusProfile {
    pub(super) code_start: usize,
    pub(super) code_end: usize,
    pub(super) first_data_pointer: u32,
    pub(super) static_packet_arena_roots: &'static [StaticPacketArenaRoot],
    pub(super) get_tpage_calls: &'static [usize],
    pub(super) shared_calls: &'static [SharedCallSite],
    pub(super) eight_bit_calls: &'static [usize],
    pub(super) draw_mode_links: &'static [DrawModeLink],
    pub(super) expected_non_shared: NonSharedCounts,
    pub(super) non_stack_word_stores: &'static [usize],
    pub(super) packet_window_nonpacket_stores: &'static [usize],
}

#[derive(Clone, Copy)]
pub(super) struct StaticPacketArenaRoot {
    pub(super) writer_offset: usize,
    pub(super) address: u32,
}

#[derive(Clone, Copy)]
pub(super) struct DrawModeLink {
    pub(super) get_tpage_call: usize,
    pub(super) table_lui_delta: usize,
    pub(super) table_load_delta: usize,
    pub(super) draw_to_display_delta: usize,
    pub(super) texture_window_store_delta: usize,
    pub(super) function_load_delta: usize,
    pub(super) call_delta: usize,
    pub(super) result_delta: usize,
    pub(super) texture_window_origin: usize,
    pub(super) texture_window_register: Register,
}

pub(super) const BASICS_GET_TPAGE_CALLS: [usize; 28] = [
    0x2030, 0x21b8, 0x22f0, 0x2484, 0x42d4, 0x4884, 0x49f8, 0x4c60, 0x4ecc, 0x5054, 0x5198, 0x52c4,
    0x5400, 0x5630, 0x57f8, 0x598c, 0x5ab8, 0x5ef8, 0x6170, 0x63a8, 0x658c, 0x677c, 0x68d4, 0x6d2c,
    0x6e7c, 0x7014, 0x7284, 0x7408,
];

pub(super) const EXAM_1999_GET_TPAGE_CALLS: [usize; 34] = [
    0x194c, 0x1ad4, 0x1c0c, 0x39fc, 0x3fac, 0x4120, 0x4388, 0x458c, 0x4714, 0x485c, 0x498c, 0x4ac4,
    0x4d10, 0x4ed4, 0x5098, 0x522c, 0x5358, 0x5500, 0x5760, 0x5998, 0x5b8c, 0x5dc0, 0x5f68, 0x60ac,
    0x621c, 0x63b8, 0x6790, 0x6910, 0x6bb4, 0x6dc4, 0x6fb4, 0x7144, 0x72c8, 0x74d4,
];

pub(super) const BASICS_SHARED_CALLS: [SharedCallSite; 4] = [
    SharedCallSite {
        call_offset: 0x2030,
        kind: SharedCallKind::PrimaryDescriptor,
    },
    SharedCallSite {
        call_offset: 0x42d4,
        kind: SharedCallKind::SecondaryDescriptorBankSelected,
    },
    SharedCallSite {
        call_offset: 0x7014,
        kind: SharedCallKind::Direct { texture_x: 0x380 },
    },
    SharedCallSite {
        call_offset: 0x7284,
        kind: SharedCallKind::Direct { texture_x: 0x300 },
    },
];

pub(super) const EXAM_1999_SHARED_CALLS: [SharedCallSite; 7] = [
    SharedCallSite {
        call_offset: 0x194c,
        kind: SharedCallKind::PrimaryDescriptor,
    },
    SharedCallSite {
        call_offset: 0x39fc,
        kind: SharedCallKind::SecondaryDescriptorBankSelected,
    },
    SharedCallSite {
        call_offset: 0x6910,
        kind: SharedCallKind::Direct { texture_x: 0x300 },
    },
    SharedCallSite {
        call_offset: 0x6bb4,
        kind: SharedCallKind::Direct { texture_x: 0x300 },
    },
    SharedCallSite {
        call_offset: 0x6dc4,
        kind: SharedCallKind::Direct { texture_x: 0x300 },
    },
    SharedCallSite {
        call_offset: 0x6fb4,
        kind: SharedCallKind::Direct { texture_x: 0x300 },
    },
    SharedCallSite {
        call_offset: 0x72c8,
        kind: SharedCallKind::Direct { texture_x: 0x380 },
    },
];

pub(super) const BASICS_EIGHT_BIT_CALLS: [usize; 2] = [0x6d2c, 0x6e7c];
pub(super) const EXAM_1999_EIGHT_BIT_CALLS: [usize; 2] = [0x6790, 0x7144];

pub(super) const BASICS_DRAW_MODE_LINKS: [DrawModeLink; 2] = [
    DrawModeLink {
        get_tpage_call: 0x5054,
        table_lui_delta: 0x14,
        table_load_delta: 0x18,
        draw_to_display_delta: 0x10,
        texture_window_store_delta: 0x20,
        function_load_delta: 0x24,
        call_delta: 0x2c,
        result_delta: 0x30,
        texture_window_origin: 0x5070,
        texture_window_register: Register::S1,
    },
    DrawModeLink {
        get_tpage_call: 0x5198,
        table_lui_delta: 0x10,
        table_load_delta: 0x14,
        draw_to_display_delta: 0x18,
        texture_window_store_delta: 0x1c,
        function_load_delta: 0x20,
        call_delta: 0x28,
        result_delta: 0x2c,
        texture_window_origin: 0x5070,
        texture_window_register: Register::S1,
    },
];

pub(super) const EXAM_1999_DRAW_MODE_LINKS: [DrawModeLink; 2] = [
    DrawModeLink {
        get_tpage_call: 0x4714,
        table_lui_delta: 0x14,
        table_load_delta: 0x18,
        draw_to_display_delta: 0x10,
        texture_window_store_delta: 0x20,
        function_load_delta: 0x24,
        call_delta: 0x2c,
        result_delta: 0x30,
        texture_window_origin: 0x4730,
        texture_window_register: Register::S2,
    },
    DrawModeLink {
        get_tpage_call: 0x485c,
        table_lui_delta: 0x10,
        table_load_delta: 0x14,
        draw_to_display_delta: 0x18,
        texture_window_store_delta: 0x1c,
        function_load_delta: 0x20,
        call_delta: 0x28,
        result_delta: 0x2c,
        texture_window_origin: 0x4730,
        texture_window_register: Register::S2,
    },
];

// Every non-stack word store in each pinned executable span.  Raw AddPrim
// expansion necessarily writes an ordering-table link and a packet tag through
// a word store, regardless of which displacement or base alias the compiler
// chooses.  Keeping this full denominator (rather than packet+0/+4 guesses)
// lets the typed grammars below reject any uncatalogued direct insertion path.
pub(super) const BASICS_NON_STACK_WORD_STORES: [usize; 10] = [
    0x0e18, 0x0e28, 0x0fa4, 0x0fb4, 0x2d38, 0x2dac, 0x2ff4, 0x3954, 0x43e4, 0x6320,
];
pub(super) const EXAM_1999_NON_STACK_WORD_STORES: [usize; 21] = [
    0x0ac8, 0x0ad8, 0x0c54, 0x0c64, 0x2200, 0x220c, 0x2458, 0x24c8, 0x2e70, 0x2e78, 0x2e80, 0x2e88,
    0x2ea0, 0x2eb4, 0x2ec0, 0x3b0c, 0x5910, 0x68d4, 0x6b68, 0x6d54, 0x6f44,
];
pub(super) const BASICS_PACKET_WINDOW_NONPACKET_STORES: [usize; 3] = [0x2374, 0x2508, 0x4908];
pub(super) const EXAM_1999_PACKET_WINDOW_NONPACKET_STORES: [usize; 2] = [0x1c90, 0x4030];
pub(super) const BASICS_STATIC_PACKET_ARENA_ROOTS: [StaticPacketArenaRoot; 1] =
    [StaticPacketArenaRoot {
        writer_offset: 0x6c20,
        address: 0x8018_7000,
    }];
pub(super) const EXAM_1999_STATIC_PACKET_ARENA_ROOTS: [StaticPacketArenaRoot; 2] = [
    StaticPacketArenaRoot {
        writer_offset: 0x5f18,
        address: 0x800a_97b0,
    },
    StaticPacketArenaRoot {
        writer_offset: 0x6684,
        address: 0x8018_7000,
    },
];

pub(super) fn profile(consumer: PracticalExamConsumer) -> DirectCensusProfile {
    match consumer {
        PracticalExamConsumer::BasicsReview => DirectCensusProfile {
            code_start: 0x0df4,
            code_end: 0x760c,
            first_data_pointer: 0x800a_2e88,
            static_packet_arena_roots: &BASICS_STATIC_PACKET_ARENA_ROOTS,
            get_tpage_calls: &BASICS_GET_TPAGE_CALLS,
            shared_calls: &BASICS_SHARED_CALLS,
            eight_bit_calls: &BASICS_EIGHT_BIT_CALLS,
            draw_mode_links: &BASICS_DRAW_MODE_LINKS,
            expected_non_shared: NonSharedCounts {
                external_y_256: 16,
                outside_producer_x: 6,
            },
            non_stack_word_stores: &BASICS_NON_STACK_WORD_STORES,
            packet_window_nonpacket_stores: &BASICS_PACKET_WINDOW_NONPACKET_STORES,
        },
        PracticalExamConsumer::Exam1999 => DirectCensusProfile {
            code_start: 0x0aa4,
            code_end: 0x76d8,
            first_data_pointer: 0x800a_2b38,
            static_packet_arena_roots: &EXAM_1999_STATIC_PACKET_ARENA_ROOTS,
            get_tpage_calls: &EXAM_1999_GET_TPAGE_CALLS,
            shared_calls: &EXAM_1999_SHARED_CALLS,
            eight_bit_calls: &EXAM_1999_EIGHT_BIT_CALLS,
            draw_mode_links: &EXAM_1999_DRAW_MODE_LINKS,
            expected_non_shared: NonSharedCounts {
                external_y_256: 18,
                outside_producer_x: 7,
            },
            non_stack_word_stores: &EXAM_1999_NON_STACK_WORD_STORES,
            packet_window_nonpacket_stores: &EXAM_1999_PACKET_WINDOW_NONPACKET_STORES,
        },
    }
}
