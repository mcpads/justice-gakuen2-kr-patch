use anyhow::{Result, ensure};

use crate::bonus_inventory::{BONUS_INVENTORY_PATH, BonusInventoryBuild};
use crate::bonus_inventory_source::BonusInventorySource;
use crate::compression::decompress;
use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::disc::rebuild;
use crate::menu_compression::compress_menu_with_source_limits;
use crate::paired_decoded_record_plan::PairedDecodedRecordPlan;
use crate::pipeline::sha256_bytes;

pub(super) struct BonusInventorySurfaceBuild {
    pub inventory_source: rebuild::DiscRecordSourceIdentity,
    pub inventory_stored: Vec<u8>,
    pub inventory_decoded: Vec<u8>,
    pub overlay_source: rebuild::DiscRecordSourceIdentity,
    pub overlay_path: String,
    pub overlay: Vec<u8>,
}

pub(super) struct BonusInventorySurfaceComposition {
    inventory_source: rebuild::DiscRecordSourceIdentity,
    source_inventory_stored: Vec<u8>,
    overlay_source: rebuild::DiscRecordSourceIdentity,
    overlay_path: String,
    records: PairedDecodedRecordPlan,
}

pub(super) fn load_bonus_inventory_surface_composition(
    source: &BonusInventorySource,
    fixed_inventory: &BonusInventoryBuild,
    overlay_path: &str,
    expected_overlay_sha256: &str,
) -> Result<BonusInventorySurfaceComposition> {
    ensure!(
        fixed_inventory.report.source_path == BONUS_INVENTORY_PATH,
        "bonus-inventory fixed UI uses another source record"
    );
    let source_inventory_stored = source.inventory_stored.clone();
    ensure!(
        source_inventory_stored.len() == fixed_inventory.report.source_record_size
            && sha256_bytes(&source_inventory_stored)
                == fixed_inventory.report.source_stored_sha256,
        "bonus-inventory composition source record identity changed"
    );
    let source_inventory_decoded = decompress(&source_inventory_stored, false)?;
    ensure!(
        sha256_bytes(&source_inventory_decoded) == fixed_inventory.report.source_decoded_sha256,
        "bonus-inventory composition source decoded identity changed"
    );
    ensure!(
        sha256_bytes(&fixed_inventory.decoded) == fixed_inventory.report.patched_decoded_sha256,
        "bonus-inventory fixed decoded bytes changed before composition"
    );

    let overlay = source.overlay.clone();
    ensure!(
        sha256_bytes(&overlay) == expected_overlay_sha256,
        "bonus overlay composition source identity changed"
    );
    let mut composition = BonusInventorySurfaceComposition::from_verified_sources(
        source_inventory_stored,
        fixed_inventory.decoded.clone(),
        fixed_inventory.decoded_write_claims.clone(),
        overlay_path.to_string(),
        overlay,
    )?;
    let mut category_overlay = source.overlay.clone();
    composition.records.register_second_candidate(
        "bonus device messages",
        fixed_inventory.device_text.overlay.clone(),
        DecodedDataClaim::from_ranges(
            "bonus-device-messages",
            "translate PocketStation and J-BANK pointer records",
            fixed_inventory.device_text.overlay_ranges.clone(),
        ),
    )?;
    composition.records.register_second_candidate(
        "bonus inventory item labels",
        fixed_inventory.item_labels.overlay.clone(),
        DecodedDataClaim::from_ranges(
            "bonus-item-labels",
            "render compact movie and poster names from their pointer table",
            fixed_inventory.item_labels.overlay_ranges.clone(),
        ),
    )?;
    let ranges = crate::bonus_inventory::apply_category_sprite_geometry(
        &mut category_overlay,
        fixed_inventory,
    )?;
    if !ranges.is_empty() {
        composition.records.register_second_candidate(
            "bonus inventory category geometry",
            category_overlay,
            DecodedDataClaim::from_ranges(
                "bonus-category-geometry",
                "sample authored category rectangles",
                ranges,
            ),
        )?;
    }
    Ok(composition)
}

impl BonusInventorySurfaceComposition {
    fn from_verified_sources(
        source_inventory_stored: Vec<u8>,
        inventory_decoded: Vec<u8>,
        fixed_inventory_claims: Vec<DecodedDataClaim>,
        overlay_path: String,
        overlay: Vec<u8>,
    ) -> Result<Self> {
        let inventory_source =
            rebuild::DiscRecordSourceIdentity::from_bytes(&source_inventory_stored);
        let overlay_source = rebuild::DiscRecordSourceIdentity::from_bytes(&overlay);
        let source_inventory_decoded = decompress(&source_inventory_stored, false)?;
        let mut records = PairedDecodedRecordPlan::new(
            BONUS_INVENTORY_PATH,
            source_inventory_decoded,
            overlay_path.clone(),
            overlay,
        )?;
        records.register_first_candidate(
            "bonus inventory fixed UI",
            inventory_decoded,
            fixed_inventory_claims,
        )?;
        Ok(Self {
            inventory_source,
            source_inventory_stored,
            overlay_source,
            overlay_path,
            records,
        })
    }

    pub(super) fn apply_surface_writer<InventoryWriter, OverlayWriter>(
        &mut self,
        owner: &str,
        inventory_writer: InventoryWriter,
        overlay_writer: OverlayWriter,
    ) -> Result<()>
    where
        InventoryWriter: FnOnce(&mut [u8]) -> Result<Vec<[usize; 2]>>,
        OverlayWriter: FnOnce(&mut [u8]) -> Result<Vec<[usize; 2]>>,
    {
        self.records
            .register_writer(owner, inventory_writer, overlay_writer)
    }

    pub(super) fn finish(self) -> Result<BonusInventorySurfaceBuild> {
        let records = self.records.compose()?;
        let (reencoded, _, _) =
            compress_menu_with_source_limits(&records.first, &self.source_inventory_stored)?;
        ensure!(
            decompress(&reencoded, false)? == records.first,
            "composed KOUBAI1 compression roundtrip changed decoded bytes"
        );
        ensure!(
            reencoded.len() <= self.source_inventory_stored.len(),
            "composed KOUBAI1 exceeds its source record"
        );
        let mut inventory_stored = reencoded;
        inventory_stored.resize(self.source_inventory_stored.len(), 0);
        ensure!(
            decompress(&inventory_stored, true)? == records.first,
            "padded composed KOUBAI1 changed decoded bytes"
        );
        Ok(BonusInventorySurfaceBuild {
            inventory_source: self.inventory_source,
            inventory_stored,
            inventory_decoded: records.first,
            overlay_source: self.overlay_source,
            overlay_path: self.overlay_path,
            overlay: records.second,
        })
    }
}

#[cfg(test)]
#[path = "bonus_inventory_composition_tests.rs"]
mod tests;
