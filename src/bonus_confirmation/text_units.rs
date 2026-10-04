use super::model::ConfirmationTextUnit;

pub(super) const TEXT_UNIT_SPECS: [ConfirmationTextSpec; 6] = [
    ConfirmationTextSpec {
        id: "exit_prompt",
        source_text: "終了しますか？",
        korean_text: "종료할까요?",
        record_offset: 0x167c,
        record_length: 24,
        source_sha256: "f21ccab237f104267fa5c7b5d7ecd32a799558815ebbe341d00a006a2e382721",
    },
    ConfirmationTextSpec {
        id: "selected_card_prompt",
        source_text: "このカードでよろしいですか？",
        korean_text: "이 카드로 할까요?",
        record_offset: 0x1694,
        record_length: 44,
        source_sha256: "d7bc880847bea6d085173f30390e2eda6349fcfe0be75ffb7462c1ffcfe51592",
    },
    ConfirmationTextSpec {
        id: "memory_card_destination",
        source_text: "このメモリーカードに",
        korean_text: "이 메모리 카드에",
        record_offset: 0x0fe4,
        record_length: 32,
        source_sha256: "58057bc1293f69ff5344316215f5f7860fe252a10529c6b78a44656a96cf82ee",
    },
    ConfirmationTextSpec {
        id: "memory_card_copy_prompt",
        source_text: "コピーしてよろしいですか？",
        korean_text: "복사할까요?",
        record_offset: 0x1004,
        record_length: 40,
        source_sha256: "ca161f7745553c8acbc49a8a893f8e3958279f5496db9ffa363fbf6d052f5da9",
    },
    ConfirmationTextSpec {
        id: "shared_yes",
        source_text: "はい",
        korean_text: "예",
        record_offset: 0x16c0,
        record_length: 6,
        source_sha256: "f98516fdb6293503638d4c02d84a252b6233fc96e8480ba5e7a2aa11194e460b",
    },
    ConfirmationTextSpec {
        id: "shared_no",
        source_text: "いいえ",
        korean_text: "아니요",
        record_offset: 0x16c6,
        record_length: 9,
        source_sha256: "9e0eaea691b11c67b4b1d7ba777b6987283f7734487522d535a4340ad2365bfb",
    },
];

#[derive(Clone, Copy)]
pub(super) struct ConfirmationTextSpec {
    pub(super) id: &'static str,
    pub(super) source_text: &'static str,
    pub(super) korean_text: &'static str,
    pub(super) record_offset: usize,
    pub(super) record_length: usize,
    pub(super) source_sha256: &'static str,
}

pub(super) fn authored_text<'a>(
    units: &'a [ConfirmationTextUnit],
    id: &str,
) -> anyhow::Result<&'a str> {
    let unit = units
        .iter()
        .find(|unit| unit.id == id)
        .ok_or_else(|| anyhow::anyhow!("bonus confirmation lacks authored unit {id}"))?;
    unit.korean_text
        .as_deref()
        .ok_or_else(|| anyhow::anyhow!("authored bonus confirmation unit {id} lost Korean text"))
}
