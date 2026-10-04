//! Assembles every typed feature result into the finite physical-record set.

use std::path::Path;

use anyhow::{Result, ensure};

use crate::bonus_inventory::BONUS_INVENTORY_PATH;
use crate::bonus_menu::{BONUS_MENU_PATH, BonusMenuBuild};
use crate::character_select_graphics::CharacterSelectAtlasBuild;
use crate::diary_header::{DIARY_HEADER_PATH, DiaryHeaderBuild};
use crate::diary_scene::{DIARY_SCENE_PATH, DiarySceneBuild};
use crate::disc::rebuild::{DiscRecordContribution, DiscRecordSourceIdentity};
use crate::edit_runtime_text::{
    EditRuntimeTextBuild, OVERLAY_PATH as EDIT_RUNTIME_TEXT_OVERLAY_PATH,
    PASS_PATH as EDIT_RUNTIME_TEXT_PASS_PATH,
};
use crate::mode_descendant_graphics::ModeDescendantGraphicsBuild;
use crate::mode_select::{MODE_SELECT_MENU_PATH, ModeSelectRecordBuild};
use crate::options::{OPTIONS_INFO_PATH, OPTIONS_OVERLAY_PATH, OptionsRecordBuild};
use crate::pipeline::sha256_bytes;
use crate::practical_instruction_graphics::{
    PRACTICAL_INSTRUCTION_PATH, PracticalInstructionGraphicsBuild,
};
use crate::shop_ui::SHOP_UI_PATH;
use crate::source_disc::MAIN_EXECUTABLE_PATH;
use crate::title_menu::{TITLE_MENU_OVERLAY_PATH, TitleMenuRecordBuild};
use crate::title_notice::TitleNoticeBuild;

use super::bonus_inventory_composition::BonusInventorySurfaceBuild;
use super::development_menu_build::{DevelopmentMenuBuild, build_development_menu};
use super::development_title_overlay_build::{
    DevelopmentTitleOverlayBuild, build_development_title_overlay,
};
use super::dialogue_bundle::{
    DialogueBundleBuild, DialogueBundleMemberCandidate, build_dialogue_bundles,
};
use super::dialogue_font_build_model::DialogueFontBuild;
use super::dialogue_name_runtime_build::DialogueNameRuntimeBuild;
use super::main_executable_record::{MAIN_EXECUTABLE_RECORD_OWNER, StoredMainExecutableRecord};
use super::mode_descendant_composition::final_mode_descendant_records;
use super::name_entry::OVERLAY_PATH as NAME_ENTRY_OVERLAY_PATH;
use super::name_entry_composed_build::ComposedNameInputBuild;
use super::name_entry_font_build::NAME_ENTRY_FONT_PATH;
use super::script_source::MGAME_PATH;
use super::shop_surface_composition::ShopSurfaceBuild;

pub(super) struct DialogueDiscRecordSources<'a> {
    pub menu_atlas_plan: &'a crate::menu_atlas_plan::MenuAtlasPlan,
    pub staff_roll: Option<&'a crate::staff_roll::StaffRollBuild>,
    pub endings: Option<&'a crate::ending_subtitles::Build>,
    pub pocket_help: Option<&'a crate::pocket_help::Build>,
    pub shared_menu_atlas: &'a crate::menu_atlas::presentation::SharedMenuAtlasBuild,
    pub card_art: Option<&'a crate::card_art::CardArtBuild>,
    pub loading_art: Option<&'a crate::loading_art::LoadingArtBuild>,
    pub source_image: &'a Path,
    pub font: &'a DialogueFontBuild,
    pub name_entry: &'a ComposedNameInputBuild,
    pub battle_names: &'a super::battle_name_build::BattleNameBuild,
    pub diary_header: &'a DiaryHeaderBuild,
    pub diary_scene: &'a DiarySceneBuild,
    pub character_select: &'a CharacterSelectAtlasBuild,
    pub mode_descendants: &'a ModeDescendantGraphicsBuild,
    pub practical_instructions: &'a PracticalInstructionGraphicsBuild,
    pub mode_select: &'a ModeSelectRecordBuild,
    pub options: &'a OptionsRecordBuild,
    pub title_menu: &'a TitleMenuRecordBuild,
    pub title_notice: &'a TitleNoticeBuild,
    pub edit_runtime_text: &'a EditRuntimeTextBuild,
    pub main_executable: &'a StoredMainExecutableRecord,
    pub dialogue_name_runtime: &'a DialogueNameRuntimeBuild,
    pub bonus_menu: &'a BonusMenuBuild,
    pub bonus_inventory_surfaces: &'a BonusInventorySurfaceBuild,
    pub shop_surfaces: &'a ShopSurfaceBuild,
}

pub(super) struct DialogueDiscRecordAssembly {
    records: Vec<OwnedDiscRecord>,
    pub dialogue_bundles: Vec<DialogueBundleBuild>,
    pub development_menu: DevelopmentMenuBuild,
    pub development_title_overlay: DevelopmentTitleOverlayBuild,
}

impl DialogueDiscRecordAssembly {
    pub(super) fn contributions(&self) -> Vec<DiscRecordContribution<'_>> {
        self.records
            .iter()
            .map(OwnedDiscRecord::contribution)
            .collect()
    }
}

pub(super) struct OwnedDiscRecord {
    owner: String,
    path: String,
    data: Vec<u8>,
    source: DiscRecordSourceIdentity,
}

impl OwnedDiscRecord {
    pub(super) fn new(
        owner: impl Into<String>,
        path: impl Into<String>,
        data: Vec<u8>,
        source: DiscRecordSourceIdentity,
    ) -> Self {
        Self {
            owner: owner.into(),
            path: path.into(),
            data,
            source,
        }
    }

    pub(super) fn contribution(&self) -> DiscRecordContribution<'_> {
        DiscRecordContribution {
            owner: self.owner.clone(),
            path: &self.path,
            data: &self.data,
            source: self.source.clone(),
        }
    }
}

pub(super) fn assemble_dialogue_disc_records(
    sources: DialogueDiscRecordSources<'_>,
) -> Result<DialogueDiscRecordAssembly> {
    let DialogueDiscRecordSources {
        menu_atlas_plan,
        staff_roll,
        endings,
        pocket_help,
        shared_menu_atlas,
        loading_art,
        card_art,
        source_image,
        font,
        name_entry,
        battle_names,
        diary_header,
        diary_scene,
        character_select,
        mode_descendants,
        practical_instructions,
        mode_select,
        options,
        title_menu,
        title_notice,
        edit_runtime_text,
        main_executable,
        dialogue_name_runtime,
        bonus_menu,
        bonus_inventory_surfaces,
        shop_surfaces,
    } = sources;
    let font_report = &font.report;
    let name_entry_report = &name_entry.report;

    ensure!(
        font.records.len() == font_report.assets.len(),
        "dialogue font typed result and report populations differ"
    );
    for (record, asset) in font.records.iter().zip(&font_report.assets) {
        ensure!(
            record.source_path == asset.source_path
                && sha256_bytes(&record.stored) == asset.font_installed_stored_sha256,
            "dialogue font typed result differs from its report"
        );
    }
    let bundle_candidates = font_report
        .assets
        .iter()
        .zip(&font.records)
        .map(|(asset, record)| {
            ensure!(
                record.source_path == asset.source_path,
                "dialogue font replacement path changed before BZZ composition"
            );
            Ok(DialogueBundleMemberCandidate {
                source_path: &asset.source_path,
                data: &record.stored,
                source_stored_sha256: &asset.source_stored_sha256,
                candidate_stored_sha256: &asset.font_installed_stored_sha256,
                candidate_decoded_sha256: &asset.font_installed_decoded_sha256,
                compression_roundtrip_verified: asset.compression_roundtrip_verified,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let dialogue_bundles = build_dialogue_bundles(source_image, &bundle_candidates)?;
    let development_menu = build_development_menu(
        mode_select,
        options,
        title_menu,
        title_notice,
        shared_menu_atlas,
    )?;
    let development_title_overlay = build_development_title_overlay(
        title_menu,
        title_notice,
        name_entry,
        &font.name_glyph_materialization,
        menu_atlas_plan,
    )?;

    let mut records = font
        .records
        .iter()
        .zip(&font_report.assets)
        .map(|(record, asset)| {
            Ok(OwnedDiscRecord::new(
                "dialogue font asset builder",
                record.source_path.clone(),
                record.stored.clone(),
                DiscRecordSourceIdentity::new(
                    asset.original_stored_byte_count,
                    asset.source_stored_sha256.clone(),
                )?,
            ))
        })
        .collect::<Result<Vec<_>>>()?;
    for bundle in &dialogue_bundles {
        records.push(OwnedDiscRecord::new(
            bundle.owner.clone(),
            bundle.path.clone(),
            bundle.data.clone(),
            DiscRecordSourceIdentity::new(bundle.source_byte_count, bundle.source_sha256.clone())?,
        ));
    }

    ensure!(
        sha256_bytes(&name_entry.font_stored) == name_entry_report.font.patched_stored_sha256,
        "name-entry font typed result differs from its report"
    );
    records.push(OwnedDiscRecord::new(
        "name-entry font builder",
        NAME_ENTRY_FONT_PATH,
        name_entry.font_stored.clone(),
        DiscRecordSourceIdentity::new(
            name_entry_report.font.source_record_size,
            name_entry_report.font.source_stored_sha256.clone(),
        )?,
    ));
    ensure!(
        sha256_bytes(&name_entry.overlay) == name_entry_report.patched_overlay_sha256,
        "name-entry overlay typed result differs from its report"
    );
    records.push(OwnedDiscRecord::new(
        "composed name-entry overlay builder",
        NAME_ENTRY_OVERLAY_PATH,
        name_entry.overlay.clone(),
        DiscRecordSourceIdentity::new(
            name_entry_report.source_overlay_size,
            name_entry_report.source_overlay_sha256.clone(),
        )?,
    ));
    records.push(OwnedDiscRecord::new(
        "diary-header record builder",
        DIARY_HEADER_PATH,
        diary_header.stored.clone(),
        DiscRecordSourceIdentity::new(
            diary_header.report.source_record_size,
            diary_header.report.source_stored_sha256.clone(),
        )?,
    ));
    records.push(OwnedDiscRecord::new(
        "diary-scene archive builder",
        DIARY_SCENE_PATH,
        diary_scene.archive.clone(),
        DiscRecordSourceIdentity::new(
            diary_scene.report.source_archive_size,
            diary_scene.report.source_archive_sha256.clone(),
        )?,
    ));
    for bundle in &diary_scene.runtime_bundles {
        records.push(OwnedDiscRecord::new(
            "diary-scene runtime-bundle compositor",
            bundle.path.clone(),
            bundle.data.clone(),
            DiscRecordSourceIdentity::new(
                bundle.report.source_size,
                bundle.report.source_sha256.clone(),
            )?,
        ));
    }

    for portrait in &diary_scene.artwork_archives {
        records.push(OwnedDiscRecord::new(
            "diary artwork archive builder",
            portrait.report.path.clone(),
            portrait.data.clone(),
            DiscRecordSourceIdentity::new(
                portrait.report.source_size,
                portrait.report.source_sha256.clone(),
            )?,
        ));
    }

    ensure!(
        character_select.records.len() == character_select.report.records.len()
            && character_select.auxiliary_records.len()
                == character_select.report.auxiliary_records.len()
            && character_select.overlays.len() == character_select.report.overlays.len(),
        "character-select build output count changed before disc composition"
    );
    for (record, data) in character_select
        .report
        .records
        .iter()
        .zip(&character_select.records)
    {
        ensure!(
            sha256_bytes(data) == record.patched_stored_sha256,
            "{} changed after the character-select build",
            record.source_path
        );
        records.push(OwnedDiscRecord::new(
            "character-select record builder",
            record.source_path.clone(),
            data.clone(),
            DiscRecordSourceIdentity::new(
                record.source_record_size,
                record.source_stored_sha256.clone(),
            )?,
        ));
    }
    for (record, data) in character_select
        .report
        .auxiliary_records
        .iter()
        .zip(&character_select.auxiliary_records)
    {
        ensure!(
            sha256_bytes(data) == record.patched_stored_sha256,
            "{} changed after the character-select auxiliary build",
            record.source_path
        );
        records.push(OwnedDiscRecord::new(
            "character-select auxiliary-record builder",
            record.source_path.clone(),
            data.clone(),
            DiscRecordSourceIdentity::new(
                record.source_record_size,
                record.source_stored_sha256.clone(),
            )?,
        ));
    }
    for (overlay, data) in character_select
        .report
        .overlays
        .iter()
        .zip(&character_select.overlays)
    {
        ensure!(
            sha256_bytes(data) == overlay.patched_sha256,
            "{} changed after the character-select build",
            overlay.source_path
        );
        records.push(OwnedDiscRecord::new(
            "character-select overlay builder",
            overlay.source_path.clone(),
            data.clone(),
            DiscRecordSourceIdentity::new(overlay.source_size, overlay.source_sha256.clone())?,
        ));
    }
    for record in
        final_mode_descendant_records(mode_descendants, edit_runtime_text, practical_instructions)?
    {
        ensure!(
            sha256_bytes(&record.stored) == record.stored_sha256,
            "{} changed after its final composed build",
            record.report.source_path
        );
        records.push(OwnedDiscRecord::new(
            "mode-descendant final-record compositor",
            record.report.source_path.clone(),
            record.stored,
            DiscRecordSourceIdentity::new(
                record.report.source_record_size,
                record.report.source_stored_sha256.clone(),
            )?,
        ));
    }

    ensure!(
        practical_instructions.report.source_record_path == PRACTICAL_INSTRUCTION_PATH
            && practical_instructions.record.len()
                == practical_instructions.report.source_record_size
            && sha256_bytes(&practical_instructions.record)
                == practical_instructions.report.patched_record_sha256,
        "practical-instruction build output differs from its report"
    );
    records.push(OwnedDiscRecord::new(
        "practical-instruction graphics builder",
        PRACTICAL_INSTRUCTION_PATH,
        practical_instructions.record.clone(),
        DiscRecordSourceIdentity::new(
            practical_instructions.report.source_record_size,
            practical_instructions.report.source_record_sha256.clone(),
        )?,
    ));
    ensure!(
        practical_instructions.prompt_consumers.len() == 2,
        "practical-instruction gameplay-prompt consumer denominator changed"
    );
    for consumer in &practical_instructions.prompt_consumers {
        ensure!(
            consumer.record.len() == consumer.source_record_size
                && sha256_bytes(&consumer.record) == consumer.patched_record_sha256,
            "{} practical gameplay-prompt output differs from its report",
            consumer.path
        );
        records.push(OwnedDiscRecord::new(
            "practical gameplay-prompt table builder",
            consumer.path.clone(),
            consumer.record.clone(),
            DiscRecordSourceIdentity::new(
                consumer.source_record_size,
                consumer.source_record_sha256.clone(),
            )?,
        ));
    }

    records.push(OwnedDiscRecord::new(
        "development menu compositor",
        MODE_SELECT_MENU_PATH,
        development_menu.stored.clone(),
        development_menu.source.clone(),
    ));
    records.push(OwnedDiscRecord::new(
        "options overlay builder",
        OPTIONS_OVERLAY_PATH,
        options.overlay.clone(),
        DiscRecordSourceIdentity::new(
            options.report.source_overlay_size,
            options.report.source_overlay_sha256.clone(),
        )?,
    ));
    records.push(OwnedDiscRecord::new(
        "options information builder",
        OPTIONS_INFO_PATH,
        options.optinfo_stored.clone(),
        DiscRecordSourceIdentity::new(
            options.report.original_optinfo_stored_size,
            options.report.source_optinfo_stored_sha256.clone(),
        )?,
    ));
    records.push(OwnedDiscRecord::new(
        "EDIT runtime-text compositor",
        EDIT_RUNTIME_TEXT_OVERLAY_PATH,
        edit_runtime_text.overlay.clone(),
        DiscRecordSourceIdentity::new(
            edit_runtime_text.report.source_overlay_size,
            edit_runtime_text.report.source_overlay_sha256.clone(),
        )?,
    ));
    ensure!(
        edit_runtime_text.pass.len() == edit_runtime_text.report.source_pass_size
            && sha256_bytes(&edit_runtime_text.pass) == edit_runtime_text.report.output_pass_sha256,
        "EDIT runtime-text PASS typed result differs from its report"
    );
    records.push(OwnedDiscRecord::new(
        "EDIT runtime-text compositor",
        EDIT_RUNTIME_TEXT_PASS_PATH,
        edit_runtime_text.pass.clone(),
        DiscRecordSourceIdentity::new(
            edit_runtime_text.report.source_pass_size,
            edit_runtime_text.report.source_pass_sha256.clone(),
        )?,
    ));
    records.push(OwnedDiscRecord::new(
        "development title-overlay compositor",
        TITLE_MENU_OVERLAY_PATH,
        development_title_overlay.stored.clone(),
        development_title_overlay.source.clone(),
    ));
    records.push(OwnedDiscRecord::new(
        MAIN_EXECUTABLE_RECORD_OWNER,
        MAIN_EXECUTABLE_PATH,
        main_executable.data.clone(),
        main_executable.source.clone(),
    ));
    records.push(OwnedDiscRecord::new(
        "dialogue-name runtime compositor",
        MGAME_PATH,
        dialogue_name_runtime.bytes.clone(),
        DiscRecordSourceIdentity::new(
            dialogue_name_runtime.report.source_size,
            dialogue_name_runtime.report.source_sha256.clone(),
        )?,
    ));
    records.push(OwnedDiscRecord::new(
        "bonus-menu record builder",
        BONUS_MENU_PATH,
        bonus_menu.stored.clone(),
        DiscRecordSourceIdentity::new(
            bonus_menu.report.source_record_size,
            bonus_menu.report.source_stored_sha256.clone(),
        )?,
    ));
    records.push(OwnedDiscRecord::new(
        "bonus-inventory surface compositor",
        BONUS_INVENTORY_PATH,
        bonus_inventory_surfaces.inventory_stored.clone(),
        bonus_inventory_surfaces.inventory_source.clone(),
    ));
    records.push(OwnedDiscRecord::new(
        "bonus-inventory surface compositor",
        bonus_inventory_surfaces.overlay_path.clone(),
        bonus_inventory_surfaces.overlay.clone(),
        bonus_inventory_surfaces.overlay_source.clone(),
    ));
    records.push(OwnedDiscRecord::new(
        "shop surface compositor",
        SHOP_UI_PATH,
        shop_surfaces.shop_ui_stored.clone(),
        shop_surfaces.shop_ui_source.clone(),
    ));
    records.push(OwnedDiscRecord::new(
        "shop surface compositor",
        shop_surfaces.overlay_path.clone(),
        shop_surfaces.overlay.clone(),
        shop_surfaces.overlay_source.clone(),
    ));

    if let Some(loading) = loading_art {
        records.push(OwnedDiscRecord::new(
            "loading illustration and message builder",
            crate::loading_art::PATH,
            loading.record.clone(),
            DiscRecordSourceIdentity::new(loading.record.len(), loading.source_sha256.clone())?,
        ));
    }

    records.push(OwnedDiscRecord::new(
        "shared menu atlas reload",
        shared_menu_atlas.reload_path.clone(),
        shared_menu_atlas.reload_stored.clone(),
        shared_menu_atlas.reload_source.clone(),
    ));

    if let Some(endings) = endings {
        for record in &endings.records {
            records.push(OwnedDiscRecord::new(
                "ending subtitles",
                &record.path,
                record.output.clone(),
                DiscRecordSourceIdentity::new(record.output.len(), record.source_sha256.clone())?,
            ));
        }
    }
    if let Some(help) = pocket_help {
        records.push(OwnedDiscRecord::new(
            "PocketStation help and bank",
            "DAT2/PDATIM.BIZ",
            help.output.clone(),
            DiscRecordSourceIdentity::new(help.output.len(), help.source_sha256.clone())?,
        ));
    }
    if let Some(staff) = staff_roll {
        for record in &staff.records {
            records.push(OwnedDiscRecord::new(
                "staff-roll credits",
                record.path,
                record.output.clone(),
                DiscRecordSourceIdentity::new(record.output.len(), record.source_sha256.clone())?,
            ));
        }
    }
    if let Some(cards) = card_art {
        records.push(OwnedDiscRecord::new(
            "card letter artwork builder",
            crate::card_art::PATH,
            cards.record.clone(),
            DiscRecordSourceIdentity::new(cards.record.len(), cards.source_sha256.clone())?,
        ));
    }

    records.push(OwnedDiscRecord::new(
        "battle name runtime",
        super::battle_name_build::PATH,
        battle_names.stored.clone(),
        battle_names.source.clone(),
    ));

    Ok(DialogueDiscRecordAssembly {
        records,
        dialogue_bundles,
        development_menu,
        development_title_overlay,
    })
}
