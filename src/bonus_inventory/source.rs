pub(super) use crate::bonus_inventory_source::{
    BonusInventorySource, load_bonus_inventory_source as load_source,
};
use crate::source_disc::profile::BONUS_INVENTORY_RECORD;

pub const BONUS_INVENTORY_PATH: &str = BONUS_INVENTORY_RECORD.path;
pub(super) const SOURCE_STORED_SHA256: &str = BONUS_INVENTORY_RECORD.stored_sha256;
pub(super) const SOURCE_DECODED_SHA256: &str = BONUS_INVENTORY_RECORD.decoded_sha256;
pub(super) const FIXED_UI_TIM_OFFSET: usize = crate::bonus_inventory_source::FIXED_UI_TIM_OFFSET;
