use crate::character_select_graphics::episode_card_consumer::TITLE_MEMBER_IMAGE_HEIGHTS;
use crate::character_select_graphics::model::CharacterSelectFixedStripSegment;
use crate::character_select_graphics::texture_targets::SOLO_EPISODE_CARD_TIM_OFFSETS;
use crate::tim::Cell;

#[derive(Clone, Copy)]
pub(super) struct EpisodeCardSegmentSpec {
    pub(super) segment: CharacterSelectFixedStripSegment,
    pub(super) source_indexed_sha256: &'static str,
}

pub(super) struct EpisodeCardLineSpec {
    pub(super) logical_text_region_id: &'static str,
    pub(super) logical_width: usize,
    pub(super) logical_height: usize,
    pub(super) segments: &'static [EpisodeCardSegmentSpec],
}

pub(super) struct EpisodeCardSpec {
    pub(super) member_index: usize,
    pub(super) source_ui_id: &'static str,
    pub(super) source_text: &'static str,
    pub(super) tim_offset: usize,
    pub(super) image_height: usize,
    pub(super) lines: [&'static EpisodeCardLineSpec; 3],
}

const fn segment(
    physical_text_region_id: &'static str,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    logical_x: usize,
    source_indexed_sha256: &'static str,
) -> EpisodeCardSegmentSpec {
    EpisodeCardSegmentSpec {
        segment: CharacterSelectFixedStripSegment {
            physical_text_region_id,
            cell: Cell {
                x,
                y,
                width,
                height,
            },
            logical_origin: [logical_x, 0],
        },
        source_indexed_sha256,
    }
}

const CARD_1_HEADING_SEGMENTS: [EpisodeCardSegmentSpec; 1] = [segment(
    "solo-episode-card-1-heading",
    0,
    0,
    120,
    40,
    0,
    "c402fad25b97614fa922bf44cac58639eb7e0f55c0349308f37653da1bc7ab07",
)];
const CARD_1_TITLE_SEGMENTS: [EpisodeCardSegmentSpec; 2] = [
    segment(
        "solo-episode-card-1-title-first",
        0,
        40,
        240,
        40,
        0,
        "362dc9d2045c235b92fb2a16327ead3f24422b2b7ad1cbb64f88fdfcb5a39d06",
    ),
    segment(
        "solo-episode-card-1-title-second",
        0,
        80,
        80,
        40,
        240,
        "f635d34fca37089a435160e95d10d271a079d44a071b4fc9992bc8cb9857c690",
    ),
];
const CARD_1_LOCATION_SEGMENTS: [EpisodeCardSegmentSpec; 1] = [segment(
    "solo-episode-card-1-location",
    0,
    120,
    248,
    24,
    0,
    "7ee4a5a9cbb10ed6cad2ffe74dce2b5788795579caac0e5531f46899d25db263",
)];

const CARD_2_HEADING_SEGMENTS: [EpisodeCardSegmentSpec; 1] = [segment(
    "solo-episode-card-2-heading",
    0,
    0,
    120,
    40,
    0,
    "b5973848ac4f809ce0eb51ce82be2e9082d4371c2c16a850de8cc2db0d819b8b",
)];
const CARD_2_TITLE_SEGMENTS: [EpisodeCardSegmentSpec; 1] = [segment(
    "solo-episode-card-2-title",
    0,
    40,
    120,
    40,
    0,
    "92b1168f229d16f457dfc65619154286c7aa8f091b7f31ba242362a0ee613b56",
)];
const CARD_2_LOCATION_SEGMENTS: [EpisodeCardSegmentSpec; 1] = [segment(
    "solo-episode-card-2-location",
    0,
    80,
    248,
    24,
    0,
    "94b89b06ba332d612862d29ccaf1e567a8fc0a87f03f0311b5fb38001403f928",
)];

const CARD_3_HEADING_SEGMENTS: [EpisodeCardSegmentSpec; 1] = [segment(
    "solo-episode-card-3-heading",
    0,
    0,
    120,
    40,
    0,
    "7ce24b045657fec13fa70f989041046b7e89cff5e716f2b8d12ffd9b3006f98a",
)];
const CARD_3_TITLE_SEGMENTS: [EpisodeCardSegmentSpec; 1] = [segment(
    "solo-episode-card-3-title",
    0,
    40,
    240,
    40,
    0,
    "1fbf7a1770fc4ce89c19bb51d747e3d09918afe1f4775a9285bbdfe052ee332f",
)];
const CARD_3_LOCATION_SEGMENTS: [EpisodeCardSegmentSpec; 2] = [
    segment(
        "solo-episode-card-3-location-first",
        0,
        80,
        248,
        24,
        0,
        "817da4cfa98db0ba1cb345a2fa6599c3fcf5ba2fe5ecf7bc5a9771208a8e6a59",
    ),
    segment(
        "solo-episode-card-3-location-second",
        0,
        104,
        48,
        24,
        248,
        "476912296a060266c844b99b742330f05c9f01333b680c49c6b451f5443aa414",
    ),
];

const CARD_4_HEADING_SEGMENTS: [EpisodeCardSegmentSpec; 1] = [segment(
    "solo-episode-card-4-heading",
    0,
    0,
    120,
    40,
    0,
    "f336d44b00a86c8c32a5c205ba882d474ecacf7b0dff084cbab9a79480bc1381",
)];
const CARD_4_TITLE_SEGMENTS: [EpisodeCardSegmentSpec; 1] = [segment(
    "solo-episode-card-4-title",
    0,
    40,
    240,
    40,
    0,
    "4432e10de6f2caeef8ab92d804e7ae13aedb4934041e6d36834643f3f17aaa0c",
)];
const CARD_4_LOCATION_SEGMENTS: [EpisodeCardSegmentSpec; 1] = [segment(
    "solo-episode-card-4-location",
    0,
    80,
    248,
    24,
    0,
    "20d5ce6fc81963dd750e1702ba1f4bc599c1e82c67dd8a5a1bc2e0016276e5ac",
)];

const CARD_5_HEADING_SEGMENTS: [EpisodeCardSegmentSpec; 1] = [segment(
    "solo-episode-card-5-heading",
    0,
    0,
    120,
    40,
    0,
    "9dee44a5d83845c6920ca2948b83ae2edfe8c31192fc291bd74a35596e55e0c2",
)];
const CARD_5_TITLE_SEGMENTS: [EpisodeCardSegmentSpec; 1] = [segment(
    "solo-episode-card-5-title",
    0,
    40,
    200,
    40,
    0,
    "0abc90921f257d9ec6eda6a749548a901c2566ed0bf7783006562e2b789d1aac",
)];
const CARD_5_LOCATION_SEGMENTS: [EpisodeCardSegmentSpec; 2] = [
    segment(
        "solo-episode-card-5-location-first",
        0,
        80,
        248,
        24,
        0,
        "968ab72ea850675aa4dd9a247528f6514606a06856a9cdc4da4f1b1e60ba80ca",
    ),
    segment(
        "solo-episode-card-5-location-second",
        0,
        104,
        48,
        24,
        248,
        "d10e2dcbb0c49d57ab1d469869f1ed5dae37db07c1cf56012e039672a00e2728",
    ),
];

const CARD_6_HEADING_SEGMENTS: [EpisodeCardSegmentSpec; 1] = [segment(
    "solo-episode-card-6-heading",
    0,
    0,
    120,
    40,
    0,
    "f0b80298ddfcc004944331fd39cc91b0b4a1f4c647ce442290535f83d4ecc892",
)];
const CARD_6_TITLE_SEGMENTS: [EpisodeCardSegmentSpec; 2] = [
    segment(
        "solo-episode-card-6-title-first",
        0,
        40,
        240,
        40,
        0,
        "37722ce9faae40a522310af2dd1939d0c507eeb75e2d7a758f1c9321961004b3",
    ),
    segment(
        "solo-episode-card-6-title-second",
        0,
        80,
        80,
        40,
        240,
        "e5ba376d7684c5ec1b6ddebddd67efdf38769731f9745fb3218fe0a630119310",
    ),
];
const CARD_6_LOCATION_SEGMENTS: [EpisodeCardSegmentSpec; 2] = [
    segment(
        "solo-episode-card-6-location-first",
        0,
        120,
        248,
        24,
        0,
        "b41d8b3ec2ac9994b1b8b08a7e71761458b27579c8a7f26cf1ba37af13f0fc3e",
    ),
    segment(
        "solo-episode-card-6-location-second",
        0,
        144,
        48,
        24,
        248,
        "70e5a8e51c38bb68a6a40ac9b160719bd17f9171f6c3698f3129aa80031550ee",
    ),
];

const CARD_7_HEADING_SEGMENTS: [EpisodeCardSegmentSpec; 1] = [segment(
    "solo-episode-card-7-heading",
    0,
    0,
    120,
    40,
    0,
    "de52800a816ad84b27bdf9e8f8f65450c2eb9781b2121c5097295322c43e67e6",
)];
const CARD_7_TITLE_SEGMENTS: [EpisodeCardSegmentSpec; 1] = [segment(
    "solo-episode-card-7-title",
    0,
    40,
    200,
    40,
    0,
    "9f8ca815be1058059178e281599cb0f8c20eec46dfa317430d55d04a6be43106",
)];
const CARD_7_LOCATION_SEGMENTS: [EpisodeCardSegmentSpec; 2] = [
    segment(
        "solo-episode-card-7-location-first",
        0,
        80,
        248,
        24,
        0,
        "a328e5365af6eaa14ddbdd66a824ea112a461e5e8b007c95c63ffc589b7c947d",
    ),
    segment(
        "solo-episode-card-7-location-second",
        0,
        104,
        48,
        24,
        248,
        "65d45aa47a2af482926bd8a2bc85dff713d63064a67fade04d56ed53c99eb211",
    ),
];

const CARD_8_HEADING_SEGMENTS: [EpisodeCardSegmentSpec; 1] = [segment(
    "solo-episode-card-8-heading",
    0,
    0,
    120,
    40,
    0,
    "515724559eab120a8c4ee73740d95bd66fa37a5b3ca3867f737a79388211afed",
)];
const CARD_8_TITLE_SEGMENTS: [EpisodeCardSegmentSpec; 1] = [segment(
    "solo-episode-card-8-title",
    0,
    40,
    200,
    40,
    0,
    "cb243dd111e10fa45ba47530e3b855b1428b97576f89a0bc48f666a6a15a26e9",
)];
const CARD_8_LOCATION_SEGMENTS: [EpisodeCardSegmentSpec; 1] = [segment(
    "solo-episode-card-8-location",
    0,
    80,
    248,
    24,
    0,
    "016b128249cf62878d0abfec7314bb84f20c1ba5fcc04e46a7036a29621c9ac9",
)];

const CARD_1_HEADING: EpisodeCardLineSpec = EpisodeCardLineSpec {
    logical_text_region_id: "solo-episode-card-1-heading",
    logical_width: 120,
    logical_height: 40,
    segments: &CARD_1_HEADING_SEGMENTS,
};
const CARD_1_TITLE: EpisodeCardLineSpec = EpisodeCardLineSpec {
    logical_text_region_id: "solo-episode-card-1-title",
    logical_width: 320,
    logical_height: 40,
    segments: &CARD_1_TITLE_SEGMENTS,
};
const CARD_1_LOCATION: EpisodeCardLineSpec = EpisodeCardLineSpec {
    logical_text_region_id: "solo-episode-card-1-location",
    logical_width: 248,
    logical_height: 24,
    segments: &CARD_1_LOCATION_SEGMENTS,
};
const CARD_2_HEADING: EpisodeCardLineSpec = EpisodeCardLineSpec {
    logical_text_region_id: "solo-episode-card-2-heading",
    logical_width: 120,
    logical_height: 40,
    segments: &CARD_2_HEADING_SEGMENTS,
};
const CARD_2_TITLE: EpisodeCardLineSpec = EpisodeCardLineSpec {
    logical_text_region_id: "solo-episode-card-2-title",
    logical_width: 120,
    logical_height: 40,
    segments: &CARD_2_TITLE_SEGMENTS,
};
const CARD_2_LOCATION: EpisodeCardLineSpec = EpisodeCardLineSpec {
    logical_text_region_id: "solo-episode-card-2-location",
    logical_width: 248,
    logical_height: 24,
    segments: &CARD_2_LOCATION_SEGMENTS,
};
const CARD_3_HEADING: EpisodeCardLineSpec = EpisodeCardLineSpec {
    logical_text_region_id: "solo-episode-card-3-heading",
    logical_width: 120,
    logical_height: 40,
    segments: &CARD_3_HEADING_SEGMENTS,
};
const CARD_3_TITLE: EpisodeCardLineSpec = EpisodeCardLineSpec {
    logical_text_region_id: "solo-episode-card-3-title",
    logical_width: 240,
    logical_height: 40,
    segments: &CARD_3_TITLE_SEGMENTS,
};
const CARD_3_LOCATION: EpisodeCardLineSpec = EpisodeCardLineSpec {
    logical_text_region_id: "solo-episode-card-3-location",
    logical_width: 296,
    logical_height: 24,
    segments: &CARD_3_LOCATION_SEGMENTS,
};
const CARD_4_HEADING: EpisodeCardLineSpec = EpisodeCardLineSpec {
    logical_text_region_id: "solo-episode-card-4-heading",
    logical_width: 120,
    logical_height: 40,
    segments: &CARD_4_HEADING_SEGMENTS,
};
const CARD_4_TITLE: EpisodeCardLineSpec = EpisodeCardLineSpec {
    logical_text_region_id: "solo-episode-card-4-title",
    logical_width: 240,
    logical_height: 40,
    segments: &CARD_4_TITLE_SEGMENTS,
};
const CARD_4_LOCATION: EpisodeCardLineSpec = EpisodeCardLineSpec {
    logical_text_region_id: "solo-episode-card-4-location",
    logical_width: 248,
    logical_height: 24,
    segments: &CARD_4_LOCATION_SEGMENTS,
};
const CARD_5_HEADING: EpisodeCardLineSpec = EpisodeCardLineSpec {
    logical_text_region_id: "solo-episode-card-5-heading",
    logical_width: 120,
    logical_height: 40,
    segments: &CARD_5_HEADING_SEGMENTS,
};
const CARD_5_TITLE: EpisodeCardLineSpec = EpisodeCardLineSpec {
    logical_text_region_id: "solo-episode-card-5-title",
    logical_width: 200,
    logical_height: 40,
    segments: &CARD_5_TITLE_SEGMENTS,
};
const CARD_5_LOCATION: EpisodeCardLineSpec = EpisodeCardLineSpec {
    logical_text_region_id: "solo-episode-card-5-location",
    logical_width: 296,
    logical_height: 24,
    segments: &CARD_5_LOCATION_SEGMENTS,
};
const CARD_6_HEADING: EpisodeCardLineSpec = EpisodeCardLineSpec {
    logical_text_region_id: "solo-episode-card-6-heading",
    logical_width: 120,
    logical_height: 40,
    segments: &CARD_6_HEADING_SEGMENTS,
};
const CARD_6_TITLE: EpisodeCardLineSpec = EpisodeCardLineSpec {
    logical_text_region_id: "solo-episode-card-6-title",
    logical_width: 320,
    logical_height: 40,
    segments: &CARD_6_TITLE_SEGMENTS,
};
const CARD_6_LOCATION: EpisodeCardLineSpec = EpisodeCardLineSpec {
    logical_text_region_id: "solo-episode-card-6-location",
    logical_width: 296,
    logical_height: 24,
    segments: &CARD_6_LOCATION_SEGMENTS,
};
const CARD_7_HEADING: EpisodeCardLineSpec = EpisodeCardLineSpec {
    logical_text_region_id: "solo-episode-card-7-heading",
    logical_width: 120,
    logical_height: 40,
    segments: &CARD_7_HEADING_SEGMENTS,
};
const CARD_7_TITLE: EpisodeCardLineSpec = EpisodeCardLineSpec {
    logical_text_region_id: "solo-episode-card-7-title",
    logical_width: 200,
    logical_height: 40,
    segments: &CARD_7_TITLE_SEGMENTS,
};
const CARD_7_LOCATION: EpisodeCardLineSpec = EpisodeCardLineSpec {
    logical_text_region_id: "solo-episode-card-7-location",
    logical_width: 296,
    logical_height: 24,
    segments: &CARD_7_LOCATION_SEGMENTS,
};
const CARD_8_HEADING: EpisodeCardLineSpec = EpisodeCardLineSpec {
    logical_text_region_id: "solo-episode-card-8-heading",
    logical_width: 120,
    logical_height: 40,
    segments: &CARD_8_HEADING_SEGMENTS,
};
const CARD_8_TITLE: EpisodeCardLineSpec = EpisodeCardLineSpec {
    logical_text_region_id: "solo-episode-card-8-title",
    logical_width: 200,
    logical_height: 40,
    segments: &CARD_8_TITLE_SEGMENTS,
};
const CARD_8_LOCATION: EpisodeCardLineSpec = EpisodeCardLineSpec {
    logical_text_region_id: "solo-episode-card-8-location",
    logical_width: 248,
    logical_height: 24,
    segments: &CARD_8_LOCATION_SEGMENTS,
};

pub(super) const EPISODE_CARDS: [EpisodeCardSpec; 8] = [
    EpisodeCardSpec {
        member_index: 0,
        source_ui_id: "solo_episode_card_1",
        source_text: "第一話\n手がかりを持つ者\n於 太陽学園高等部屋上",
        tim_offset: SOLO_EPISODE_CARD_TIM_OFFSETS[0],
        image_height: TITLE_MEMBER_IMAGE_HEIGHTS[0],
        lines: [&CARD_1_HEADING, &CARD_1_TITLE, &CARD_1_LOCATION],
    },
    EpisodeCardSpec {
        member_index: 1,
        source_ui_id: "solo_episode_card_2",
        source_text: "第二話\n濡れ衣\n於 外道高校裏工事現場",
        tim_offset: SOLO_EPISODE_CARD_TIM_OFFSETS[1],
        image_height: TITLE_MEMBER_IMAGE_HEIGHTS[1],
        lines: [&CARD_2_HEADING, &CARD_2_TITLE, &CARD_2_LOCATION],
    },
    EpisodeCardSpec {
        member_index: 2,
        source_ui_id: "solo_episode_card_3",
        source_text: "第三話\n血のつながり\n於 太陽学園中等部グランド",
        tim_offset: SOLO_EPISODE_CARD_TIM_OFFSETS[2],
        image_height: TITLE_MEMBER_IMAGE_HEIGHTS[2],
        lines: [&CARD_3_HEADING, &CARD_3_TITLE, &CARD_3_LOCATION],
    },
    EpisodeCardSpec {
        member_index: 3,
        source_ui_id: "solo_episode_card_4",
        source_text: "第四話\n千切れた友情\n於 太陽学園高等部教室",
        tim_offset: SOLO_EPISODE_CARD_TIM_OFFSETS[3],
        image_height: TITLE_MEMBER_IMAGE_HEIGHTS[3],
        lines: [&CARD_4_HEADING, &CARD_4_TITLE, &CARD_4_LOCATION],
    },
    EpisodeCardSpec {
        member_index: 4,
        source_ui_id: "solo_episode_card_5",
        source_text: "第五話\n返らぬ答え\n於 パシフィックHS裏門前",
        tim_offset: SOLO_EPISODE_CARD_TIM_OFFSETS[4],
        image_height: TITLE_MEMBER_IMAGE_HEIGHTS[4],
        lines: [&CARD_5_HEADING, &CARD_5_TITLE, &CARD_5_LOCATION],
    },
    EpisodeCardSpec {
        member_index: 5,
        source_ui_id: "solo_episode_card_6",
        source_text: "第六話\nジャスティス学園\n於 ジャスティス学園正門前",
        tim_offset: SOLO_EPISODE_CARD_TIM_OFFSETS[5],
        image_height: TITLE_MEMBER_IMAGE_HEIGHTS[5],
        lines: [&CARD_6_HEADING, &CARD_6_TITLE, &CARD_6_LOCATION],
    },
    EpisodeCardSpec {
        member_index: 6,
        source_ui_id: "solo_episode_card_7",
        source_text: "第七話\n友情と血縁\n於 ジャスティス学園図書館",
        tim_offset: SOLO_EPISODE_CARD_TIM_OFFSETS[6],
        image_height: TITLE_MEMBER_IMAGE_HEIGHTS[6],
        lines: [&CARD_7_HEADING, &CARD_7_TITLE, &CARD_7_LOCATION],
    },
    EpisodeCardSpec {
        member_index: 7,
        source_ui_id: "solo_episode_card_8",
        source_text: "最終話\n兄か悪魔か\n於 幻影空間",
        tim_offset: SOLO_EPISODE_CARD_TIM_OFFSETS[7],
        image_height: TITLE_MEMBER_IMAGE_HEIGHTS[7],
        lines: [&CARD_8_HEADING, &CARD_8_TITLE, &CARD_8_LOCATION],
    },
];
