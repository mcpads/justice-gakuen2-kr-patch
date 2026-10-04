//! Exact semantic and texture layout of the nine timed OP01 story-intro lines.

use crate::character_select_graphics::model::CharacterSelectFixedStripSegment;
use crate::tim::Cell;

pub(super) const TOTAL_STORY_INTRO_LINES: usize = 9;

#[derive(Clone, Copy)]
pub(super) struct StoryIntroSegmentSpec {
    pub(super) segment: CharacterSelectFixedStripSegment,
    pub(super) source_indexed_sha256: &'static str,
}

#[derive(Clone, Copy)]
pub(super) struct StoryIntroLineSpec {
    pub(super) logical_text_region_id: &'static str,
    pub(super) logical_width: usize,
    pub(super) layout_index: usize,
    pub(super) segments: &'static [StoryIntroSegmentSpec],
}

pub(super) struct StoryIntroAssetSpec {
    pub(super) source_ui_id: &'static str,
    pub(super) source_text: &'static str,
    pub(super) lines: &'static [StoryIntroLineSpec],
}

const OPENING_LINE_1_SEGMENTS: [StoryIntroSegmentSpec; 1] = [segment(
    "solo-story-intro-line-1-head",
    0,
    0,
    256,
    0,
    "0a55cd4ae546012cebe63e4c516e3db7c466560bec731544d9f2bc33f82e3b9e",
)];

const OPENING_LINE_2_SEGMENTS: [StoryIntroSegmentSpec; 2] = [
    segment(
        "solo-story-intro-line-2-head",
        0,
        48,
        256,
        0,
        "7e0b883e5b807ed5016f1085213230175906754844c17a3eb0e58d1034e70a59",
    ),
    segment(
        "solo-story-intro-line-2-tail",
        0,
        72,
        80,
        256,
        "4b9eded70917ddb27e847d8c1808fe14f08c6c5706eac95718a2a54be551c42d",
    ),
];

const OPENING_LINE_3_SEGMENTS: [StoryIntroSegmentSpec; 2] = [
    segment(
        "solo-story-intro-line-3-head",
        0,
        96,
        256,
        0,
        "25f8730ca6685690fe4e1630465e5dc0edd39aa651646ddddc4b2f655ada1e18",
    ),
    segment(
        "solo-story-intro-line-3-tail",
        0,
        120,
        80,
        256,
        "8abf9f0f809ca9a80570b756361c9763860931834231b2fdf051a2e652e0b286",
    ),
];

const ADULTS_UNRELIABLE_SEGMENTS: [StoryIntroSegmentSpec; 2] = [
    segment(
        "solo-story-intro-adults-unreliable-head",
        0,
        144,
        256,
        0,
        "a20fd7852467b6a013e96c30819a221878c4241d40643da31a14133334913fab",
    ),
    segment(
        "solo-story-intro-adults-unreliable-tail",
        0,
        168,
        104,
        256,
        "3104c2f31992ce8dd9ead5280333f4ec486848f728c83549f1b44093a4689e9d",
    ),
];

const STUDENTS_RISE_SEGMENTS: [StoryIntroSegmentSpec; 2] = [
    segment(
        "solo-story-intro-students-rise-head",
        0,
        192,
        256,
        0,
        "c9ad00077ef1591577a281d350d33dc55f781a1e7e70ea0f7f0c1983f33cecba",
    ),
    segment(
        "solo-story-intro-students-rise-tail",
        0,
        216,
        160,
        256,
        "44e888017f41b3e84fed36367ac36de4786d2b76022ce913b5511860f10b114e",
    ),
];

const PROTECT_SCHOOL_SEGMENTS: [StoryIntroSegmentSpec; 2] = [
    segment(
        "solo-story-intro-protect-school-head",
        256,
        0,
        256,
        0,
        "a577e466d07b7f08b2aa3b4dbe4425373017efee025f5a2617cd0fd528136d18",
    ),
    segment(
        "solo-story-intro-protect-school-tail",
        256,
        24,
        48,
        256,
        "753d173be3dc4981cb14d4c841c289853f146930b31e630447dc4e70168b8ada",
    ),
];

const CONFRONT_EVIL_LINE_1_SEGMENTS: [StoryIntroSegmentSpec; 2] = [
    segment(
        "solo-story-intro-confront-evil-line-1-head",
        256,
        48,
        256,
        0,
        "0a75f35065bd116ab546a11db20a759067ebb2d2d6ccbfe0a421811cfe089b8b",
    ),
    segment(
        "solo-story-intro-confront-evil-line-1-tail",
        256,
        72,
        40,
        256,
        "772944f45eeeae477cabc7dc2b6ffec471af99a1b20c28d66ee1cea48159e37b",
    ),
];

const CONFRONT_EVIL_LINE_2_SEGMENTS: [StoryIntroSegmentSpec; 1] = [segment(
    "solo-story-intro-confront-evil-line-2",
    256,
    96,
    256,
    0,
    "c9d92710f4bf4075eb46caf3d92151e1dc28eb423cc36814f826d0ac94450317",
)];

const TARGET_JUSTICE_ACADEMY_SEGMENTS: [StoryIntroSegmentSpec; 2] = [
    segment(
        "solo-story-intro-target-justice-academy-head",
        256,
        144,
        256,
        0,
        "2bde329fd4d8bdabdfab384243159afb8a280c1cf5ad53d8174aad17e02b8e8f",
    ),
    segment(
        "solo-story-intro-target-justice-academy-tail",
        256,
        168,
        32,
        256,
        "09aa738cc76b4e476aa99b199790b8e50decc4454fb11f0af8d6dba41ec98ceb",
    ),
];

const OPENING_LINES: [StoryIntroLineSpec; 3] = [
    line("solo-story-intro-line-1", 256, 0, &OPENING_LINE_1_SEGMENTS),
    line("solo-story-intro-line-2", 336, 1, &OPENING_LINE_2_SEGMENTS),
    line("solo-story-intro-line-3", 336, 2, &OPENING_LINE_3_SEGMENTS),
];
const ADULTS_UNRELIABLE_LINES: [StoryIntroLineSpec; 1] = [line(
    "solo-story-intro-adults-unreliable",
    360,
    3,
    &ADULTS_UNRELIABLE_SEGMENTS,
)];
const STUDENTS_RISE_LINES: [StoryIntroLineSpec; 1] = [line(
    "solo-story-intro-students-rise",
    416,
    4,
    &STUDENTS_RISE_SEGMENTS,
)];
const PROTECT_SCHOOL_LINES: [StoryIntroLineSpec; 1] = [line(
    "solo-story-intro-protect-school",
    304,
    5,
    &PROTECT_SCHOOL_SEGMENTS,
)];
const CONFRONT_EVIL_LINES: [StoryIntroLineSpec; 2] = [
    line(
        "solo-story-intro-confront-evil-line-1",
        296,
        6,
        &CONFRONT_EVIL_LINE_1_SEGMENTS,
    ),
    line(
        "solo-story-intro-confront-evil-line-2",
        256,
        7,
        &CONFRONT_EVIL_LINE_2_SEGMENTS,
    ),
];
const TARGET_JUSTICE_ACADEMY_LINES: [StoryIntroLineSpec; 1] = [line(
    "solo-story-intro-target-justice-academy",
    288,
    8,
    &TARGET_JUSTICE_ACADEMY_SEGMENTS,
)];

pub(super) static STORY_INTRO_ASSETS: [StoryIntroAssetSpec; 6] = [
    StoryIntroAssetSpec {
        source_ui_id: "solo_story_intro",
        source_text: "２０世紀末。\n全国の優秀な高校生が何者かに誘拐、\nあるいは襲われる事件が多発した。",
        lines: &OPENING_LINES,
    },
    StoryIntroAssetSpec {
        source_ui_id: "solo_story_intro_adults_unreliable",
        source_text: "警察も大人も、もうあてには出来ない！",
        lines: &ADULTS_UNRELIABLE_LINES,
    },
    StoryIntroAssetSpec {
        source_ui_id: "solo_story_intro_students_rise",
        source_text: "こんな中、ついに生徒達自身が立ち上がった。",
        lines: &STUDENTS_RISE_LINES,
    },
    StoryIntroAssetSpec {
        source_ui_id: "solo_story_intro_protect_school",
        source_text: "「オレ達の学校は、俺達が守る！」",
        lines: &PROTECT_SCHOOL_LINES,
    },
    StoryIntroAssetSpec {
        source_ui_id: "solo_story_intro_confront_evil",
        source_text: "生徒達はそれぞれの思いを胸に\n巨悪に立ち向かっていく。",
        lines: &CONFRONT_EVIL_LINES,
    },
    StoryIntroAssetSpec {
        source_ui_id: "solo_story_intro_target_justice_academy",
        source_text: "「目指すはジャスティス学園！」",
        lines: &TARGET_JUSTICE_ACADEMY_LINES,
    },
];

const fn segment(
    physical_text_region_id: &'static str,
    x: usize,
    y: usize,
    width: usize,
    logical_x: usize,
    source_indexed_sha256: &'static str,
) -> StoryIntroSegmentSpec {
    StoryIntroSegmentSpec {
        segment: CharacterSelectFixedStripSegment {
            physical_text_region_id,
            cell: Cell {
                x,
                y,
                width,
                height: 24,
            },
            logical_origin: [logical_x, 0],
        },
        source_indexed_sha256,
    }
}

const fn line(
    logical_text_region_id: &'static str,
    logical_width: usize,
    layout_index: usize,
    segments: &'static [StoryIntroSegmentSpec],
) -> StoryIntroLineSpec {
    StoryIntroLineSpec {
        logical_text_region_id,
        logical_width,
        layout_index,
        segments,
    }
}
