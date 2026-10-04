use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Result, ensure};

use super::super::model::ModeDescendantFontRole;
use super::descriptor::PracticalExamConsumer;

#[derive(Debug, Clone)]
pub(in crate::mode_descendant_graphics) struct PracticalExamTranslation {
    pub(super) id: String,
    pub(super) source_text: String,
    pub(super) korean_text: String,
    pub(super) font_role: ModeDescendantFontRole,
}

impl PracticalExamTranslation {
    pub(in crate::mode_descendant_graphics) fn new(
        id: String,
        source_text: String,
        korean_text: String,
        font_role: ModeDescendantFontRole,
    ) -> Self {
        Self {
            id,
            source_text,
            korean_text,
            font_role,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PracticalExamPlacement {
    DynamicStrip,
    GuardedHintComposite,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PracticalExamDescriptorBinding {
    pub(super) consumer: PracticalExamConsumer,
    pub(super) descriptor_index: usize,
    pub(super) descriptor_offset: usize,
    pub(super) descriptor_capacity: usize,
    pub(super) source_descriptor_sha256: &'static str,
}

pub(super) struct PracticalExamCatalogEntry {
    pub(super) id: &'static str,
    pub(super) source_text: &'static str,
    pub(super) font_role: ModeDescendantFontRole,
    pub(super) placement: PracticalExamPlacement,
    pub(super) bindings: &'static [PracticalExamDescriptorBinding],
}

const fn basics_binding(
    descriptor_index: usize,
    descriptor_offset: usize,
    descriptor_capacity: usize,
    source_descriptor_sha256: &'static str,
) -> PracticalExamDescriptorBinding {
    PracticalExamDescriptorBinding {
        consumer: PracticalExamConsumer::BasicsReview,
        descriptor_index,
        descriptor_offset,
        descriptor_capacity,
        source_descriptor_sha256,
    }
}

const BASICS_TITLE: [PracticalExamDescriptorBinding; 1] = [PracticalExamDescriptorBinding {
    consumer: PracticalExamConsumer::BasicsReview,
    descriptor_index: 0,
    descriptor_offset: 0x00a4,
    descriptor_capacity: 8,
    source_descriptor_sha256: "3a03cc167c108f7504eb5fbf7182afbb0d567469768a8366af2b5c572025effb",
}];

const EXAM_1999_TITLE: [PracticalExamDescriptorBinding; 1] = [PracticalExamDescriptorBinding {
    consumer: PracticalExamConsumer::Exam1999,
    descriptor_index: 0,
    descriptor_offset: 0x0158,
    descriptor_capacity: 12,
    source_descriptor_sha256: "e184d7d5d749fd417e1b963d20adfdf22c293b6f371db69ab9992a3d46b9b762",
}];

const EXAM_START: [PracticalExamDescriptorBinding; 1] = [PracticalExamDescriptorBinding {
    consumer: PracticalExamConsumer::Exam1999,
    descriptor_index: 1,
    descriptor_offset: 0x0164,
    descriptor_capacity: 24,
    source_descriptor_sha256: "dac1a97fdfecb2bad52906c9098f93fc1831e6aff6905920242b5842c9393b4d",
}];

const RESULTS: [PracticalExamDescriptorBinding; 2] = [
    PracticalExamDescriptorBinding {
        consumer: PracticalExamConsumer::BasicsReview,
        descriptor_index: 46,
        descriptor_offset: 0x03e8,
        descriptor_capacity: 24,
        source_descriptor_sha256: "b298286116fcef3d342ae6dfbe61af586bc0f953ad30d7b15c11ecdd48ec8891",
    },
    PracticalExamDescriptorBinding {
        consumer: PracticalExamConsumer::Exam1999,
        descriptor_index: 2,
        descriptor_offset: 0x017c,
        descriptor_capacity: 24,
        source_descriptor_sha256: "b298286116fcef3d342ae6dfbe61af586bc0f953ad30d7b15c11ecdd48ec8891",
    },
];

const BASICS_REVIEW: [PracticalExamDescriptorBinding; 1] = [PracticalExamDescriptorBinding {
    consumer: PracticalExamConsumer::BasicsReview,
    descriptor_index: 47,
    descriptor_offset: 0x0400,
    descriptor_capacity: 24,
    source_descriptor_sha256: "f60fd0108c403526fb74539161a0759b1a61752ab47c78f7b923c881fb36a298",
}];

const CONTROL_HINTS: [PracticalExamDescriptorBinding; 2] = [
    PracticalExamDescriptorBinding {
        consumer: PracticalExamConsumer::BasicsReview,
        descriptor_index: 48,
        descriptor_offset: 0x0418,
        descriptor_capacity: 8,
        source_descriptor_sha256: "9e85948064f6b7e285158ba76307f7dfaec084914b5cf66b6c3643bee4f4eb67",
    },
    PracticalExamDescriptorBinding {
        consumer: PracticalExamConsumer::Exam1999,
        descriptor_index: 3,
        descriptor_offset: 0x0194,
        descriptor_capacity: 8,
        source_descriptor_sha256: "9e85948064f6b7e285158ba76307f7dfaec084914b5cf66b6c3643bee4f4eb67",
    },
];

const EXAM_PROMPT: [PracticalExamDescriptorBinding; 2] = [
    PracticalExamDescriptorBinding {
        consumer: PracticalExamConsumer::BasicsReview,
        descriptor_index: 1,
        descriptor_offset: 0x00ac,
        descriptor_capacity: 28,
        source_descriptor_sha256: "55fd5b252795dee6ddeff46b96600cc8aa277f06ab3142a109187c5bd03dadbd",
    },
    PracticalExamDescriptorBinding {
        consumer: PracticalExamConsumer::Exam1999,
        descriptor_index: 4,
        descriptor_offset: 0x019c,
        descriptor_capacity: 28,
        source_descriptor_sha256: "55fd5b252795dee6ddeff46b96600cc8aa277f06ab3142a109187c5bd03dadbd",
    },
];

const FIRST_TERM: [PracticalExamDescriptorBinding; 1] = [PracticalExamDescriptorBinding {
    consumer: PracticalExamConsumer::Exam1999,
    descriptor_index: 5,
    descriptor_offset: 0x01b8,
    descriptor_capacity: 32,
    source_descriptor_sha256: "783ba2453b1849d67b377913100ad1c999c5cb995bac6cee2b6f2ab434ef3878",
}];

const SECOND_TERM: [PracticalExamDescriptorBinding; 1] = [PracticalExamDescriptorBinding {
    consumer: PracticalExamConsumer::Exam1999,
    descriptor_index: 6,
    descriptor_offset: 0x01d8,
    descriptor_capacity: 32,
    source_descriptor_sha256: "360274a9ff119df10053d78e225fe23f1f8a082357480e214bd569fb983b8df7",
}];

const SCHOOL_YEAR: [PracticalExamDescriptorBinding; 1] = [PracticalExamDescriptorBinding {
    consumer: PracticalExamConsumer::Exam1999,
    descriptor_index: 7,
    descriptor_offset: 0x01f8,
    descriptor_capacity: 24,
    source_descriptor_sha256: "73cf255deb6d951ccf2c8dc11d2258e5547a61438af20b3e79c4ef2ff96cc23b",
}];

pub(super) const PRACTICAL_EXAM_CATALOG: [PracticalExamCatalogEntry; 48] = [
    PracticalExamCatalogEntry {
        id: "practical_basics_title",
        source_text: "試験に出る実技",
        font_role: ModeDescendantFontRole::PracticalTitle,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &BASICS_TITLE,
    },
    PracticalExamCatalogEntry {
        id: "practical_exam_1999_title",
        source_text: "試験に出る実技’99",
        font_role: ModeDescendantFontRole::PracticalTitle,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &EXAM_1999_TITLE,
    },
    PracticalExamCatalogEntry {
        id: "practical_exam_start",
        source_text: "試験を行う",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &EXAM_START,
    },
    PracticalExamCatalogEntry {
        id: "practical_results",
        source_text: "成績表を見る",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &RESULTS,
    },
    PracticalExamCatalogEntry {
        id: "practical_basics_review",
        source_text: "基礎のおさらい",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &BASICS_REVIEW,
    },
    PracticalExamCatalogEntry {
        id: "practical_control_hints",
        source_text: "○－決定\n×－戻る",
        font_role: ModeDescendantFontRole::PracticalHint,
        placement: PracticalExamPlacement::GuardedHintComposite,
        bindings: &CONTROL_HINTS,
    },
    PracticalExamCatalogEntry {
        id: "practical_exam_prompt",
        source_text: "どの試験に向けて勉強しますか?",
        font_role: ModeDescendantFontRole::PracticalPrompt,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &EXAM_PROMPT,
    },
    PracticalExamCatalogEntry {
        id: "practical_first_term_exam",
        source_text: "1学期期末試験",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &FIRST_TERM,
    },
    PracticalExamCatalogEntry {
        id: "practical_second_term_exam",
        source_text: "2学期期末試験",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &SECOND_TERM,
    },
    PracticalExamCatalogEntry {
        id: "practical_school_year_exam",
        source_text: "学年末試験",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &SCHOOL_YEAR,
    },
    PracticalExamCatalogEntry {
        id: "practical_basics_test_1",
        source_text: "試験▼1",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            2,
            0x00c8,
            16,
            "62094ef2a166d167d25a6c897ada0cd2a72fef25b7feb70f7876f811dc1c09fc",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_basics_test_2",
        source_text: "試験▼2",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            3,
            0x00d8,
            16,
            "1594c3a6aa5274e9988cbccff02541f4e92d87240b9d3aec41db04f2efacfd1c",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_basics_test_3",
        source_text: "試験▼3",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            4,
            0x00e8,
            16,
            "477ce7576986929dbb41db564790daca7332af75352b7aa19e816efe23b9041e",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_basics_test_4",
        source_text: "試験▼4",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            5,
            0x00f8,
            16,
            "cfe3a62d6d23fb9816d398f28d9bc565140fa974bced7fb38f5c3a69ceceb177",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_basics_test_5",
        source_text: "試験▼5",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            6,
            0x0108,
            16,
            "77dd151d76906a857fd97e80a5f9ebedd30f50a8d90dc086f8d4427e4669ab2d",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_basics_test_6",
        source_text: "試験▼6",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            7,
            0x0118,
            16,
            "eb490b04e6f47b9ed47b3a32e4603c8f23e51b48ab7f121a8a6abe5a4f2060a6",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_basics_subject_prompt",
        source_text: "どの科目を重点的に行いますか?",
        font_role: ModeDescendantFontRole::PracticalPrompt,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            8,
            0x0128,
            24,
            "065251231a47b1642dd09a2dfde5ff9fc5562f363c51e9968c222833795b0806",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_basics_subject_label",
        source_text: "科目",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            9,
            0x0140,
            8,
            "d69259a6491b50137d6cc7645d77832babab9f9683664f8ec213760cb45bd377",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_movement_basics",
        source_text: "<移動の基礎>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            16,
            0x0178,
            16,
            "fb3a4b8a2bec8328474b6121620a77583d8d04e4a215aa75293413bdd9875bbb",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_gravity_theory",
        source_text: "<重力理論>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            17,
            0x0188,
            16,
            "45be7c00de663f861116b72e2ffead7e8ca6352a9d5fec8ac5f4cd8063365e44",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_arm_strength_law",
        source_text: "<腕力の法則>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            18,
            0x0198,
            24,
            "1e0da8d58f8e7ad2103ba80a1f0385365cca2569c292b7861109d97f3861e864",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_kicking_theory",
        source_text: "<足技理論>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            19,
            0x01b0,
            16,
            "d63d11179896450b0950df5c7b6f1e5a79a0d9a708cc633fbba6665b3ebdccd1",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_basics_summary",
        source_text: "<基礎の総括>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            20,
            0x01c0,
            16,
            "48a3711e1c6bfadcd86f985db9207485dab33c24e6c45f6166b85061675ff82f",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_step_striking",
        source_text: "<ステップ打法>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            21,
            0x01d0,
            24,
            "8800ceb8e98d715c3ef0d7be2639b50ce071315672bff39e944bc6af46bb2156",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_three_dimensional_axis",
        source_text: "<３次元軸>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            22,
            0x01e8,
            16,
            "bfa3cfa60ff54485ef3da22d7ff860bb0c1fbf4c0bb15e018c9a2ddb4bb79449",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_applied_gravity",
        source_text: "<重力理論の応用>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            23,
            0x01f8,
            24,
            "d6624c0b14fde7dfb7d1922cb2ade9fd00c032e11239522913d2d24a32169d87",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_throw_action",
        source_text: "<投げ作用>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            24,
            0x0210,
            16,
            "5f9492038c90afbe0251204f69ded26d07483c985733b5460007b9291d0d1439",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_special_actions_summary",
        source_text: "<特殊行動総括>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            25,
            0x0220,
            24,
            "433071c1f041d48325d64ca9bb6c06c971d3ceaf15b84d2fafa233a2e9ffe400",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_breakfall_principle",
        source_text: "<受け身の原理>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            26,
            0x0238,
            16,
            "b8b79997d950d1fe0750b9771e1e8ec1e66f885f54b782dc161b5c6c83aa912e",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_four_way_rising_theorem",
        source_text: "<４方起きあがりの定理>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            27,
            0x0248,
            24,
            "0cf461a721b688418d85e003dd2886bdaf96457ddecd01fe1243fe420ea63d7d",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_love_and_friendship",
        source_text: "<愛と友情とは何ぞや?>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            28,
            0x0260,
            24,
            "bdb35eface80be68b2cc228cf653bfee145d9228ba723bbcee6c9d36d66f1e36",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_hot_blooded_combo",
        source_text: "<熱血コンボの世界>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            29,
            0x0278,
            16,
            "ea1ebbf96053a17932813bcd2f3be2f7f926db1504f370de68f4973e0a2e6a4b",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_attack_defense_reading",
        source_text: "<攻守の見極め>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            30,
            0x0288,
            16,
            "21012601abef8457da1ba880e67e13868cf31b9d00a2b69d2468876fe39112cd",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_air_burst_phenomenon",
        source_text: "<エアバースト現象>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            31,
            0x0298,
            24,
            "6b5192ffbfb2e8a8ca526e27628cabffc8e3205ee36af11aa63fa2682b9a217a",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_applied_four_way_rising",
        source_text: "<４方起きあがりの応用>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            32,
            0x02b0,
            16,
            "146e69c70de27a1f6fd1e61b514c7d50f92202a00301dba163e0b474bc88263e",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_guts_counter",
        source_text: "<炸裂!根性カウンター>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            33,
            0x02c0,
            16,
            "ae10cd01bd998c0cdd71c5a707300c6f3f0b0693fe1f7765166d7c723f157539",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_complete_combustion",
        source_text: "<完全燃焼法>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            34,
            0x02d0,
            24,
            "0b651d06b49b37d66a630cd7ea0c988c41c89f5da71ceaf46e4f0102561b6a8a",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_growth_process",
        source_text: "<成長過程の確認>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            35,
            0x02e8,
            16,
            "771586bc6d0d5c539f32cbe0bbbfaa5f8b0478d4231fe86c415951503b7b8cd1",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_evasive_movement",
        source_text: "<回り込みの展開>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            36,
            0x02f8,
            24,
            "9fb97cb397335cb4e1fb54e6aa3fdff8cf65f960b1d6c9674286d18d2dfc30d0",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_throw_escape",
        source_text: "<投げ外し解法論>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            37,
            0x0310,
            16,
            "f73d4ac4c89fe22a367c261f55c040f8d458d2608a5d60f5d1f2465e6bbf4e3a",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_attack_cancellation",
        source_text: "<攻撃相殺法>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            38,
            0x0320,
            16,
            "4c1274d96c3d7d386c9552d39dfe4412ff9c92a2f5b19cfcffe9f0108338c62a",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_infinite_guts_counter",
        source_text: "<根性カウンター無限論>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            39,
            0x0330,
            24,
            "63758439e29a9ffc551bde0828651836e194e5b3a0e43780a19bed98ffadd393",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_one_hit_to_victory",
        source_text: "<勝利の道も一発から>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            40,
            0x0348,
            16,
            "08009156cc37ead7df62009710a5de66653a77a3f90cf957eb892be1c1ed6675",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_review_test_1",
        source_text: "<試験1の確認>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            41,
            0x0358,
            28,
            "98bb0f4767fea59395ef156f9ad54de6bb65f7c486ba6845d4d19d595fbfda04",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_review_tests_2_3",
        source_text: "<試験2と3の確認>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            42,
            0x0374,
            36,
            "e0bdb2d2ea7f7d71c65070f8182fc51d97095fe7b9c7b021a2e1d08f7062ceec",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_review_tests_4_5",
        source_text: "<試験4と5の確認>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            43,
            0x0398,
            36,
            "7c1fa773baac622dd006c12acf81fc01bf310124d3157418f859242db4db99c5",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_review_all_tests",
        source_text: "<全試験の確認>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            44,
            0x03bc,
            28,
            "9343c1b11d18a4d90502fc9cb9f07c898c25bcc044d2605143d3de20354bf646",
        )],
    },
    PracticalExamCatalogEntry {
        id: "practical_subject_hayato_self_review",
        source_text: "<隼人自身で確認>",
        font_role: ModeDescendantFontRole::PracticalMenuLabel,
        placement: PracticalExamPlacement::DynamicStrip,
        bindings: &[basics_binding(
            45,
            0x03d8,
            16,
            "9484b262ed6fe42b6562f7cfaae78732377e15721c95445499f8327473495f4b",
        )],
    },
];

pub(super) fn bind_translations(
    translations: &[PracticalExamTranslation],
) -> Result<
    Vec<(
        &'static PracticalExamCatalogEntry,
        &PracticalExamTranslation,
    )>,
> {
    ensure!(
        translations.len() == PRACTICAL_EXAM_CATALOG.len(),
        "practical-exam shared UI requires all {} semantic translations",
        PRACTICAL_EXAM_CATALOG.len()
    );
    let mut by_id = BTreeMap::new();
    for translation in translations {
        ensure!(
            by_id.insert(translation.id.as_str(), translation).is_none(),
            "duplicate practical-exam translation {}",
            translation.id
        );
        ensure!(
            !translation.korean_text.trim().is_empty(),
            "practical-exam translation {} is empty",
            translation.id
        );
    }

    let mut bound = Vec::with_capacity(PRACTICAL_EXAM_CATALOG.len());
    let mut bound_ids = BTreeSet::new();
    for catalog in &PRACTICAL_EXAM_CATALOG {
        let translation = by_id
            .get(catalog.id)
            .copied()
            .ok_or_else(|| anyhow::anyhow!("missing practical-exam translation {}", catalog.id))?;
        ensure!(
            translation.source_text == catalog.source_text,
            "practical-exam translation {} source identity changed",
            catalog.id
        );
        ensure!(
            translation.font_role == catalog.font_role,
            "practical-exam translation {} selects the wrong font role",
            catalog.id
        );
        if catalog.placement == PracticalExamPlacement::GuardedHintComposite {
            ensure!(
                translation.korean_text.lines().count() == 2,
                "practical-exam control hints require exactly two Korean lines"
            );
        } else {
            ensure!(
                translation.korean_text.lines().count() == 1,
                "practical-exam strip {} must be one line",
                catalog.id
            );
        }
        bound_ids.insert(catalog.id);
        bound.push((catalog, translation));
    }
    ensure!(
        by_id.keys().all(|id| bound_ids.contains(*id)),
        "practical-exam translations contain an uncataloged id"
    );
    Ok(bound)
}
