pub(super) use crate::bonus_inventory_source::{
    BonusInventorySource as BonusInventoryCardAcquisitionSource,
    load_bonus_inventory_source as load_source,
};
use crate::source_disc::profile::{
    BONUS_INVENTORY_GLYPH_TIM, BONUS_INVENTORY_OVERLAY_RECORD, BONUS_INVENTORY_RECORD,
};

pub const BONUS_INVENTORY_CARD_ACQUISITION_INVENTORY_PATH: &str = BONUS_INVENTORY_RECORD.path;
pub const BONUS_INVENTORY_CARD_ACQUISITION_OVERLAY_PATH: &str = BONUS_INVENTORY_OVERLAY_RECORD.path;
pub(super) const INVENTORY_SOURCE_STORED_SHA256: &str = BONUS_INVENTORY_RECORD.stored_sha256;
pub(super) const INVENTORY_SOURCE_DECODED_SHA256: &str = BONUS_INVENTORY_RECORD.decoded_sha256;
pub(super) const OVERLAY_SOURCE_SHA256: &str = BONUS_INVENTORY_OVERLAY_RECORD.sha256;
pub(super) const GLYPH_TIM_OFFSET: usize = BONUS_INVENTORY_GLYPH_TIM.offset;
pub(super) const GLYPH_TIM_SIZE: usize = BONUS_INVENTORY_GLYPH_TIM.size;
