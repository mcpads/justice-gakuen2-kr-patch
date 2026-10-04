#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct DialogueControlSpec {
    pub(super) code: u16,
    pub(super) semantic_name: &'static str,
    pub(super) argument_word_count: usize,
    pub(super) meaning: &'static str,
    pub(super) renderer_evidence: &'static str,
}

pub(super) const DIALOGUE_CONTROL_SPECS: &[DialogueControlSpec] = &[
    DialogueControlSpec {
        code: 0x061e,
        semantic_name: "name_buffer_padding",
        argument_word_count: 0,
        meaning: "blank padding; one column in main-window dialogue, skipped by small-font drawing",
        renderer_evidence: "0x800b7d64..0x800b7d6c advances the main-window column; 0x800b7af4..0x800b7b14 counts copied padding; 0x800af67c and 0x800af9f4 skip it in small-font drawing",
    },
    DialogueControlSpec {
        code: 0x2000,
        semantic_name: "protagonist_family_name",
        argument_word_count: 0,
        meaning: "copy the protagonist family-name buffer at 0x801f1866",
        renderer_evidence: "0x800af718 selects 0x801f1866",
    },
    DialogueControlSpec {
        code: 0x2001,
        semantic_name: "protagonist_given_name",
        argument_word_count: 0,
        meaning: "copy the protagonist given-name buffer at 0x801f1876",
        renderer_evidence: "0x800af728 selects 0x801f1876",
    },
    DialogueControlSpec {
        code: 0x2002,
        semantic_name: "protagonist_nickname",
        argument_word_count: 0,
        meaning: "copy the protagonist nickname buffer at 0x801f1896",
        renderer_evidence: "0x800af73c selects 0x801f1896",
    },
    DialogueControlSpec {
        code: 0x2003,
        semantic_name: "relationship_name_plain",
        argument_word_count: 1,
        meaning: "select family, given, or nickname by the indexed relationship tier, with no honorific",
        renderer_evidence: "0x800afb88 case 0; relationship byte 0x801f1958[index]",
    },
    DialogueControlSpec {
        code: 0x2004,
        semantic_name: "relationship_name_kun_kanji",
        argument_word_count: 1,
        meaning: "select family or given name plus 君, or the nickname alone, by relationship tier",
        renderer_evidence: "0x800afb88 case 1; selected MGK insertion string +0x14",
    },
    DialogueControlSpec {
        code: 0x2005,
        semantic_name: "relationship_name_kun_katakana",
        argument_word_count: 1,
        meaning: "select family or given name plus クン, or the nickname alone, by relationship tier",
        renderer_evidence: "0x800afb88 case 2; selected MGK insertion string +0x18",
    },
    DialogueControlSpec {
        code: 0x2006,
        semantic_name: "relationship_family_name_kun_katakana",
        argument_word_count: 1,
        meaning: "select family name plus クン, given name alone, or nickname alone by relationship tier",
        renderer_evidence: "0x800afb88 case 3",
    },
    DialogueControlSpec {
        code: 0x2007,
        semantic_name: "relationship_name_san",
        argument_word_count: 1,
        meaning: "select family or given name plus さん, or the nickname alone, by relationship tier",
        renderer_evidence: "0x800afb88 case 4; selected MGK insertion string +0x1c",
    },
    DialogueControlSpec {
        code: 0x2008,
        semantic_name: "relationship_name_san_or_chan",
        argument_word_count: 1,
        meaning: "select family name plus さん, given name plus ちゃん, or nickname alone by relationship tier",
        renderer_evidence: "0x800afb88 case 5; selected MGK insertion strings +0x1c and +0x20",
    },
    DialogueControlSpec {
        code: 0x2009,
        semantic_name: "current_school_name",
        argument_word_count: 1,
        meaning: "copy the school-name insertion selected by state 0x801f185a; the source argument word is consumed but ignored",
        renderer_evidence: "0x800af70c consumes the argument; 0x800afb88 case 6 selects insertion string +0x24",
    },
    DialogueControlSpec {
        code: 0x200a,
        semantic_name: "current_month",
        argument_word_count: 0,
        meaning: "format state byte 0x801f1855 as decimal",
        renderer_evidence: "0x800af760",
    },
    DialogueControlSpec {
        code: 0x200b,
        semantic_name: "current_day",
        argument_word_count: 0,
        meaning: "format state byte 0x801f1856 as decimal",
        renderer_evidence: "0x800af7a4",
    },
    DialogueControlSpec {
        code: 0x200c,
        semantic_name: "comparison_month",
        argument_word_count: 0,
        meaning: "format comparison-date month state byte 0x801f1858 as decimal",
        renderer_evidence: "0x800af7e8; 0x800ad8dc compares it with current month",
    },
    DialogueControlSpec {
        code: 0x200d,
        semantic_name: "comparison_day",
        argument_word_count: 0,
        meaning: "format comparison-date day state byte 0x801f1859 as decimal",
        renderer_evidence: "0x800af82c; 0x800ad8f8 compares it with current day",
    },
    DialogueControlSpec {
        code: 0x3000,
        semantic_name: "line_break",
        argument_word_count: 0,
        meaning: "advance to the next rendered dialogue line",
        renderer_evidence: "0x800af6dc branches to the line-advance path",
    },
    DialogueControlSpec {
        code: 0x3001,
        semantic_name: "message_end",
        argument_word_count: 0,
        meaning: "terminate the message stream",
        renderer_evidence: "0x800af634 and 0x800af978 terminate traversal",
    },
    DialogueControlSpec {
        code: 0x3002,
        semantic_name: "renderer_mode",
        argument_word_count: 1,
        meaning: "protected renderer-mode update with its raw argument preserved",
        renderer_evidence: "0x800af6f8 and 0x800af704 consume one argument word",
    },
    DialogueControlSpec {
        code: 0x3003,
        semantic_name: "palette_style",
        argument_word_count: 1,
        meaning: "protected palette or style update with its raw argument preserved",
        renderer_evidence: "0x800af8a8 through 0x800af8e0 consume one argument word",
    },
];

pub(super) fn dialogue_control_spec(code: u16) -> Option<&'static DialogueControlSpec> {
    DIALOGUE_CONTROL_SPECS.iter().find(|spec| spec.code == code)
}

pub(super) fn dialogue_control_spec_by_name(
    semantic_name: &str,
) -> Option<&'static DialogueControlSpec> {
    DIALOGUE_CONTROL_SPECS
        .iter()
        .find(|spec| spec.semantic_name == semantic_name)
}
