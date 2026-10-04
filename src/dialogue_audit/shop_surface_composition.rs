use anyhow::{Result, ensure};

use crate::bonus_shop_source::BonusShopSource;
use crate::compression::decompress;
use crate::decoded_record_write_plan::DecodedDataClaim;
use crate::disc::rebuild;
use crate::menu_compression::compress_menu_with_source_limits;
use crate::paired_decoded_record_plan::PairedDecodedRecordPlan;
use crate::pipeline::sha256_bytes;
use crate::shop_ui::{SHOP_UI_PATH, ShopUiBuild};

pub(super) struct ShopSurfaceBuild {
    pub shop_ui_source: rebuild::DiscRecordSourceIdentity,
    pub shop_ui_stored: Vec<u8>,
    pub shop_ui_decoded: Vec<u8>,
    pub overlay_source: rebuild::DiscRecordSourceIdentity,
    pub overlay_path: String,
    pub overlay: Vec<u8>,
}

pub(super) struct ShopSurfaceComposition {
    shop_ui_source: rebuild::DiscRecordSourceIdentity,
    source_shop_ui_stored: Vec<u8>,
    overlay_source: rebuild::DiscRecordSourceIdentity,
    overlay_path: String,
    records: PairedDecodedRecordPlan,
}

pub(super) fn load_shop_surface_composition(
    source: &BonusShopSource,
    fixed_ui: &ShopUiBuild,
    overlay_path: &str,
    expected_overlay_sha256: &str,
) -> Result<ShopSurfaceComposition> {
    ensure!(
        fixed_ui.report.source_path == SHOP_UI_PATH,
        "shop fixed UI uses another source record"
    );
    let source_shop_ui_stored = source.shop_ui_stored.clone();
    ensure!(
        source_shop_ui_stored.len() == fixed_ui.report.source_record_size
            && sha256_bytes(&source_shop_ui_stored) == fixed_ui.report.source_stored_sha256,
        "shop composition source record identity changed"
    );
    let source_shop_ui_decoded = decompress(&source_shop_ui_stored, false)?;
    ensure!(
        sha256_bytes(&source_shop_ui_decoded) == fixed_ui.report.source_decoded_sha256,
        "shop composition source decoded identity changed"
    );
    ensure!(
        sha256_bytes(&fixed_ui.decoded) == fixed_ui.report.patched_decoded_sha256,
        "shop fixed decoded bytes changed before composition"
    );

    let overlay = source.overlay.clone();
    ensure!(
        sha256_bytes(&overlay) == expected_overlay_sha256,
        "shop overlay composition source identity changed"
    );
    ShopSurfaceComposition::from_verified_sources(
        source_shop_ui_stored,
        fixed_ui.decoded.clone(),
        fixed_ui.decoded_write_claims.clone(),
        overlay_path.to_string(),
        overlay,
    )
}

impl ShopSurfaceComposition {
    fn from_verified_sources(
        source_shop_ui_stored: Vec<u8>,
        shop_ui_decoded: Vec<u8>,
        fixed_ui_claims: Vec<DecodedDataClaim>,
        overlay_path: String,
        overlay: Vec<u8>,
    ) -> Result<Self> {
        let shop_ui_source = rebuild::DiscRecordSourceIdentity::from_bytes(&source_shop_ui_stored);
        let overlay_source = rebuild::DiscRecordSourceIdentity::from_bytes(&overlay);
        let source_shop_ui_decoded = decompress(&source_shop_ui_stored, false)?;
        let mut records = PairedDecodedRecordPlan::new(
            SHOP_UI_PATH,
            source_shop_ui_decoded,
            overlay_path.clone(),
            overlay,
        )?;
        records.register_first_candidate("shop fixed UI", shop_ui_decoded, fixed_ui_claims)?;
        Ok(Self {
            shop_ui_source,
            source_shop_ui_stored,
            overlay_source,
            overlay_path,
            records,
        })
    }

    pub(super) fn apply_surface_writer<ShopUiWriter, OverlayWriter>(
        &mut self,
        owner: &str,
        shop_ui_writer: ShopUiWriter,
        overlay_writer: OverlayWriter,
    ) -> Result<()>
    where
        ShopUiWriter: FnOnce(&mut [u8]) -> Result<Vec<[usize; 2]>>,
        OverlayWriter: FnOnce(&mut [u8]) -> Result<Vec<[usize; 2]>>,
    {
        self.records
            .register_writer(owner, shop_ui_writer, overlay_writer)
    }

    pub(super) fn finish(self) -> Result<ShopSurfaceBuild> {
        let records = self.records.compose()?;
        let (reencoded, _, _) =
            compress_menu_with_source_limits(&records.first, &self.source_shop_ui_stored)?;
        ensure!(
            decompress(&reencoded, false)? == records.first,
            "composed KOUBAI.TIZ compression roundtrip changed decoded bytes"
        );
        ensure!(
            reencoded.len() <= self.source_shop_ui_stored.len(),
            "composed KOUBAI.TIZ exceeds its source record"
        );
        let mut shop_ui_stored = reencoded;
        shop_ui_stored.resize(self.source_shop_ui_stored.len(), 0);
        ensure!(
            decompress(&shop_ui_stored, true)? == records.first,
            "padded composed KOUBAI.TIZ changed decoded bytes"
        );
        Ok(ShopSurfaceBuild {
            shop_ui_source: self.shop_ui_source,
            shop_ui_stored,
            shop_ui_decoded: records.first,
            overlay_source: self.overlay_source,
            overlay_path: self.overlay_path,
            overlay: records.second,
        })
    }
}

#[cfg(test)]
#[path = "shop_surface_composition_tests.rs"]
mod tests;
