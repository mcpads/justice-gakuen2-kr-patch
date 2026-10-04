use std::path::Path;

use anyhow::{Context, Result, ensure};

use crate::bonus_inventory::BONUS_INVENTORY_PATH;
use crate::bonus_menu::BONUS_MENU_PATH;
use crate::character_select_graphics::decode_built_record_streams;
use crate::compression::decompress;
use crate::diary_header::DIARY_HEADER_PATH;
use crate::diary_scene::DIARY_SCENE_PATH;
use crate::disc::rebuild::{self, DiscRecordContribution, RecordRebuild};
use crate::edit_runtime_text::{
    OVERLAY_PATH as EDIT_RUNTIME_TEXT_OVERLAY_PATH, PASS_PATH as EDIT_RUNTIME_TEXT_PASS_PATH,
};
use crate::mode_select::MODE_SELECT_MENU_PATH;
use crate::options::{OPTIONS_INFO_PATH, OPTIONS_OVERLAY_PATH};
use crate::pipeline::sha256_bytes;
use crate::practical_instruction_graphics::PRACTICAL_INSTRUCTION_PATH;
use crate::shop_ui::SHOP_UI_PATH;
use crate::source_disc::MAIN_EXECUTABLE_PATH;
use crate::title_menu::TITLE_MENU_OVERLAY_PATH;
use crate::tzz::parse_tzz;

use super::{DialogueDiscComponentBuilds, DialogueDiscComposition};
use crate::dialogue_audit::dialogue_disc_build_model::{
    DialogueDiscAssetReadback, DialogueDiscBundleReadback, DialogueDiscRecordReadback,
    DiarySceneRuntimeBundleReadback,
};
use crate::dialogue_audit::mode_descendant_composition::{
    decode_mode_descendant_bytes, final_mode_descendant_records,
};
use crate::dialogue_audit::name_entry::OVERLAY_PATH as NAME_ENTRY_OVERLAY_PATH;
use crate::dialogue_audit::name_entry_font_build::NAME_ENTRY_FONT_PATH;
use crate::dialogue_audit::script_source::MGAME_PATH;

use crate::dialogue_audit::dialogue_disc_build_model::{
    DialogueDiscBundleMemberReadback, DiarySceneRuntimeMemberReadback,
};

pub(super) struct DialogueDiscReadbackInputs<'a> {
    pub temporary_bin: &'a Path,
    pub rebuilt: &'a [RecordRebuild],
    pub replacements: &'a [DiscRecordContribution<'a>],
    pub component_builds: DialogueDiscComponentBuilds<'a>,
    pub composition: DialogueDiscComposition<'a>,
}

pub(super) struct DialogueDiscReadbacks {
    pub dialogue_assets: Vec<DialogueDiscAssetReadback>,
    pub dialogue_bundles: Vec<DialogueDiscBundleReadback>,
    pub name_entry_font: DialogueDiscAssetReadback,
    pub battle_name: DialogueDiscAssetReadback,
    pub name_entry: DialogueDiscRecordReadback,
    pub diary_header: DialogueDiscAssetReadback,
    pub diary_scene: DialogueDiscRecordReadback,
    pub diary_scene_runtime_bundles: Vec<DiarySceneRuntimeBundleReadback>,
    pub diary_artwork_archives: Vec<DialogueDiscRecordReadback>,
    pub character_select_records: Vec<DialogueDiscAssetReadback>,
    pub character_select_overlays: Vec<DialogueDiscRecordReadback>,
    pub mode_descendant_records: Vec<DialogueDiscAssetReadback>,
    pub practical_instruction: DialogueDiscRecordReadback,
    pub practical_instruction_prompt_consumers: Vec<DialogueDiscRecordReadback>,
    pub menu: DialogueDiscAssetReadback,
    pub options_overlay: DialogueDiscRecordReadback,
    pub options_info: DialogueDiscAssetReadback,
    pub edit_runtime_text_overlay: DialogueDiscRecordReadback,
    pub edit_runtime_text_pass: DialogueDiscRecordReadback,
    pub title_menu_overlay: DialogueDiscAssetReadback,
    pub main_executable: DialogueDiscRecordReadback,
    pub dialogue_name_runtime: DialogueDiscRecordReadback,
    pub bonus_menu: DialogueDiscAssetReadback,
    pub bonus_inventory: DialogueDiscAssetReadback,
    pub bonus_inventory_overlay: DialogueDiscRecordReadback,
    pub shop_ui: DialogueDiscAssetReadback,
    pub shop_overlay: DialogueDiscRecordReadback,
}

pub(super) fn read_back_dialogue_disc(
    inputs: DialogueDiscReadbackInputs<'_>,
) -> Result<DialogueDiscReadbacks> {
    let DialogueDiscReadbackInputs {
        temporary_bin,
        rebuilt,
        replacements,
        component_builds,
        composition,
    } = inputs;
    let DialogueDiscComponentBuilds {
        font: font_build,
        name_entry: name_entry_build,
        diary_header: diary_header_build,
        diary_scene: diary_scene_build,
        character_select: character_select_build,
        mode_descendants: mode_descendant_build,
        practical_instructions: practical_instruction_build,
        mode_select: _,
        options: options_build,
        title_menu: _,
        title_notice: _,
        edit_runtime_text: edit_runtime_text_build,
        bonus_menu: bonus_menu_build,
        bonus_inventory: _,
        bonus_inventory_action_labels: _,
        bonus_confirmation: _,
        bonus_inventory_stock_label: _,
        bonus_inventory_card_acquisition: _,
        bonus_inventory_memory_card_swap: _,
        bonus_j_bank_return_label: _,
        bonus_page_indicator: _,
        shop_ui: _,
        bonus_shop_exit_confirmation: _,
        bonus_shop_text: _,
    } = component_builds;
    let font_report = &font_build.report;
    let name_entry_report = &name_entry_build.report;
    let DialogueDiscComposition {
        dialogue_bundles,
        development_menu,
        development_title_overlay,
        main_executable,
        dialogue_name_runtime,
        bonus_inventory_surfaces,
        shop_surfaces,
    } = composition;
    ensure!(
        font_build.records.len() == font_report.assets.len(),
        "dialogue font typed result and report populations differ during readback"
    );
    let mut dialogue_assets = Vec::with_capacity(font_report.assets.len());
    for (index, (asset, record)) in font_report
        .assets
        .iter()
        .zip(&font_build.records)
        .enumerate()
    {
        let rebuilt_record = rebuilt
            .get(index)
            .context("dialogue replacement record disappeared")?;
        ensure!(
            replacements[index].path == asset.source_path
                && record.source_path == asset.source_path,
            "dialogue replacement order changed"
        );
        let (_, readback) = rebuild::read_record(temporary_bin, &asset.source_path)?;
        let expected_stored_sha256 = sha256_bytes(&record.stored);
        ensure!(
            expected_stored_sha256 == asset.font_installed_stored_sha256,
            "dialogue typed record changed before readback"
        );
        let readback_stored_sha256 = sha256_bytes(&readback);
        ensure!(
            readback == record.stored,
            "dialogue record readback changed bytes"
        );
        let decoded = decompress(&readback, true)?;
        let readback_decoded_sha256 = sha256_bytes(&decoded);
        ensure!(
            readback_decoded_sha256 == asset.font_installed_decoded_sha256,
            "dialogue record readback decoded to unexpected bytes"
        );
        dialogue_assets.push(DialogueDiscAssetReadback {
            path: asset.source_path.clone(),
            extent_lba: rebuilt_record.record.extent_lba,
            sector_count: rebuilt_record.target_lbas.len(),
            expected_stored_sha256,
            readback_stored_sha256,
            expected_decoded_sha256: asset.font_installed_decoded_sha256.clone(),
            readback_decoded_sha256,
            readback_verified: true,
        });
    }
    let bundle_start_index = font_report.assets.len();
    let mut dialogue_bundle_readbacks = Vec::with_capacity(dialogue_bundles.len());
    for (bundle_offset, bundle) in dialogue_bundles.iter().enumerate() {
        let replacement_index = bundle_start_index + bundle_offset;
        let rebuilt_record = rebuilt
            .get(replacement_index)
            .context("dialogue bundle replacement record disappeared")?;
        ensure!(
            replacements
                .get(replacement_index)
                .is_some_and(|replacement| replacement.path == bundle.path),
            "dialogue bundle replacement order changed"
        );
        let (_, readback) = rebuild::read_record(temporary_bin, &bundle.path)?;
        ensure!(
            readback == bundle.data,
            "dialogue bundle readback changed bytes for {}",
            bundle.path
        );
        let readback_sha256 = sha256_bytes(&readback);
        ensure!(
            readback_sha256 == bundle.rebuilt_sha256,
            "dialogue bundle readback hash changed for {}",
            bundle.path
        );
        ensure!(
            bundle.owner == "dialogue bundle compositor"
                && bundle.purpose == "compose rebuilt dialogue images into one BZZ record",
            "dialogue bundle ownership changed for {}",
            bundle.path
        );
        let mut members = Vec::with_capacity(bundle.members.len());
        let mut changed_byte_count = 0usize;
        for member in &bundle.members {
            let end = member.bundle_offset + member.byte_count;
            let readback_member = readback
                .get(member.bundle_offset..end)
                .context("dialogue bundle member disappeared during readback")?;
            let readback_replacement_sha256 = sha256_bytes(readback_member);
            ensure!(
                readback_replacement_sha256 == member.replacement_sha256,
                "dialogue bundle member readback changed for {}",
                member.source_path
            );
            if member.changed_byte_count == 0 {
                ensure!(
                    member.owner.is_none() && member.lineage_id.is_none(),
                    "unchanged dialogue bundle member acquired producer lineage for {}",
                    member.source_path
                );
            } else {
                ensure!(
                    member.input_present && member.owner.is_some() && member.lineage_id.is_some(),
                    "changed dialogue bundle member lost producer lineage for {}",
                    member.source_path
                );
            }
            changed_byte_count += member.changed_byte_count;
            members.push(DialogueDiscBundleMemberReadback {
                source_path: member.source_path.clone(),
                bundle_offset: member.bundle_offset,
                byte_count: member.byte_count,
                source_sha256: member.source_sha256.clone(),
                expected_replacement_sha256: member.replacement_sha256.clone(),
                readback_replacement_sha256,
                readback_verified: true,
            });
        }
        ensure!(
            changed_byte_count == bundle.changed_byte_count,
            "dialogue bundle member diff count changed for {}",
            bundle.path
        );
        dialogue_bundle_readbacks.push(DialogueDiscBundleReadback {
            path: bundle.path.clone(),
            extent_lba: rebuilt_record.record.extent_lba,
            sector_count: rebuilt_record.target_lbas.len(),
            source_sha256: bundle.source_sha256.clone(),
            expected_rebuilt_sha256: bundle.rebuilt_sha256.clone(),
            readback_sha256,
            readback_verified: true,
            members,
        });
    }

    let name_entry_font_index = bundle_start_index + dialogue_bundles.len();
    let name_entry_font_record = rebuilt
        .get(name_entry_font_index)
        .context("name-entry font replacement record disappeared")?;
    ensure!(
        replacements
            .get(name_entry_font_index)
            .is_some_and(|replacement| replacement.path == NAME_ENTRY_FONT_PATH),
        "name-entry font replacement order changed"
    );
    let (_, name_entry_font_readback) = rebuild::read_record(temporary_bin, NAME_ENTRY_FONT_PATH)?;
    ensure!(
        name_entry_font_readback == name_entry_build.font_stored,
        "name-entry font record readback changed bytes"
    );
    let name_entry_font_decoded = decompress(&name_entry_font_readback, true)?;
    let name_entry_font = DialogueDiscAssetReadback {
        path: NAME_ENTRY_FONT_PATH.to_string(),
        extent_lba: name_entry_font_record.record.extent_lba,
        sector_count: name_entry_font_record.target_lbas.len(),
        expected_stored_sha256: name_entry_report.font.patched_stored_sha256.clone(),
        readback_stored_sha256: sha256_bytes(&name_entry_font_readback),
        expected_decoded_sha256: name_entry_report.font.patched_decoded_sha256.clone(),
        readback_decoded_sha256: sha256_bytes(&name_entry_font_decoded),
        readback_verified: true,
    };
    ensure!(
        name_entry_font.readback_decoded_sha256 == name_entry_font.expected_decoded_sha256,
        "name-entry font record decoded to unexpected bytes"
    );

    let name_entry_index = name_entry_font_index + 1;
    let name_entry_record = rebuilt
        .get(name_entry_index)
        .context("name-entry replacement record disappeared")?;
    ensure!(
        replacements
            .get(name_entry_index)
            .is_some_and(|replacement| replacement.path == NAME_ENTRY_OVERLAY_PATH),
        "name-entry replacement order changed"
    );
    let (_, name_entry_readback) = rebuild::read_record(temporary_bin, NAME_ENTRY_OVERLAY_PATH)?;
    ensure!(
        name_entry_readback == name_entry_build.overlay,
        "name-entry record readback changed bytes"
    );
    let name_entry = DialogueDiscRecordReadback {
        path: NAME_ENTRY_OVERLAY_PATH.to_string(),
        extent_lba: name_entry_record.record.extent_lba,
        sector_count: name_entry_record.target_lbas.len(),
        expected_sha256: name_entry_report.patched_overlay_sha256.clone(),
        readback_sha256: sha256_bytes(&name_entry_readback),
        readback_verified: true,
    };

    let diary_header_index = name_entry_index + 1;
    let diary_header_record = rebuilt
        .get(diary_header_index)
        .context("diary header replacement record disappeared")?;
    ensure!(
        replacements
            .get(diary_header_index)
            .is_some_and(|replacement| replacement.path == DIARY_HEADER_PATH),
        "diary header replacement order changed"
    );
    let (_, diary_header_readback) = rebuild::read_record(temporary_bin, DIARY_HEADER_PATH)?;
    ensure!(
        diary_header_readback == diary_header_build.stored,
        "diary header record readback changed bytes"
    );
    let diary_header_decoded = decompress(&diary_header_readback, true)?;
    let diary_header = DialogueDiscAssetReadback {
        path: DIARY_HEADER_PATH.to_string(),
        extent_lba: diary_header_record.record.extent_lba,
        sector_count: diary_header_record.target_lbas.len(),
        expected_stored_sha256: diary_header_build.report.patched_stored_sha256.clone(),
        readback_stored_sha256: sha256_bytes(&diary_header_readback),
        expected_decoded_sha256: diary_header_build.report.patched_decoded_sha256.clone(),
        readback_decoded_sha256: sha256_bytes(&diary_header_decoded),
        readback_verified: true,
    };
    ensure!(
        diary_header.readback_decoded_sha256 == diary_header.expected_decoded_sha256,
        "diary header record decoded to unexpected bytes"
    );

    let diary_scene_index = diary_header_index + 1;
    let diary_scene_record = rebuilt
        .get(diary_scene_index)
        .context("diary scene replacement record disappeared")?;
    ensure!(
        replacements
            .get(diary_scene_index)
            .is_some_and(|replacement| replacement.path == DIARY_SCENE_PATH),
        "diary scene replacement order changed"
    );
    let (_, diary_scene_readback) = rebuild::read_record(temporary_bin, DIARY_SCENE_PATH)?;
    ensure!(
        diary_scene_readback == diary_scene_build.archive,
        "diary scene record readback changed bytes"
    );
    let diary_scene = DialogueDiscRecordReadback {
        path: DIARY_SCENE_PATH.to_string(),
        extent_lba: diary_scene_record.record.extent_lba,
        sector_count: diary_scene_record.target_lbas.len(),
        expected_sha256: diary_scene_build.report.patched_archive_sha256.clone(),
        readback_sha256: sha256_bytes(&diary_scene_readback),
        readback_verified: true,
    };

    let diary_scene_runtime_bundle_start_index = diary_scene_index + 1;
    let mut diary_scene_runtime_bundles =
        Vec::with_capacity(diary_scene_build.runtime_bundles.len());
    for (bundle_offset, bundle) in diary_scene_build.runtime_bundles.iter().enumerate() {
        let replacement_index = diary_scene_runtime_bundle_start_index + bundle_offset;
        let rebuilt_record = rebuilt
            .get(replacement_index)
            .context("diary scene runtime bundle replacement record disappeared")?;
        ensure!(
            replacements
                .get(replacement_index)
                .is_some_and(|replacement| replacement.path == bundle.path),
            "diary scene runtime bundle replacement order changed"
        );
        let (_, readback) = rebuild::read_record(temporary_bin, &bundle.path)?;
        ensure!(
            readback == bundle.data,
            "diary scene runtime bundle readback changed bytes for {}",
            bundle.path
        );
        let readback_sha256 = sha256_bytes(&readback);
        ensure!(
            readback_sha256 == bundle.report.patched_sha256,
            "diary scene runtime bundle readback hash changed for {}",
            bundle.path
        );
        let mut members = Vec::with_capacity(bundle.report.members.len());
        for member_report in &bundle.report.members {
            let end = member_report.offset + member_report.source_compressed_size;
            let member = readback.get(member_report.offset..end).with_context(|| {
                format!(
                    "diary scene runtime member {} disappeared from {}",
                    member_report.index, bundle.path
                )
            })?;
            let readback_member_sha256 = sha256_bytes(member);
            ensure!(
                readback_member_sha256 == member_report.patched_compressed_sha256,
                "diary scene runtime member {} readback changed in {}",
                member_report.index,
                bundle.path,
            );
            members.push(DiarySceneRuntimeMemberReadback {
                index: member_report.index,
                offset: member_report.offset,
                byte_count: member_report.source_compressed_size,
                expected_sha256: member_report.patched_compressed_sha256.clone(),
                readback_sha256: readback_member_sha256,
                readback_verified: true,
            });
        }
        diary_scene_runtime_bundles.push(DiarySceneRuntimeBundleReadback {
            path: bundle.path.clone(),
            extent_lba: rebuilt_record.record.extent_lba,
            sector_count: rebuilt_record.target_lbas.len(),
            source_sha256: bundle.report.source_sha256.clone(),
            expected_rebuilt_sha256: bundle.report.patched_sha256.clone(),
            readback_sha256,
            members,
            members_read_back: true,
            readback_verified: true,
        });
    }

    let portrait_start_index =
        diary_scene_runtime_bundle_start_index + diary_scene_build.runtime_bundles.len();
    let mut diary_artwork_archives = Vec::new();
    for (offset, archive) in diary_scene_build.artwork_archives.iter().enumerate() {
        let index = portrait_start_index + offset;
        let record = rebuilt
            .get(index)
            .context("portrait replacement disappeared")?;
        ensure!(
            replacements
                .get(index)
                .is_some_and(|r| r.path == archive.report.path),
            "portrait replacement order changed"
        );
        let (_, bytes) = rebuild::read_record(temporary_bin, &archive.report.path)?;
        ensure!(
            bytes == archive.data && sha256_bytes(&bytes) == archive.report.patched_sha256,
            "portrait archive readback changed"
        );
        diary_artwork_archives.push(DialogueDiscRecordReadback {
            path: archive.report.path.clone(),
            extent_lba: record.record.extent_lba,
            sector_count: record.target_lbas.len(),
            expected_sha256: archive.report.patched_sha256.clone(),
            readback_sha256: sha256_bytes(&bytes),
            readback_verified: true,
        });
    }
    let character_select_record_start_index =
        portrait_start_index + diary_scene_build.artwork_archives.len();
    ensure!(
        character_select_build.records.len() == character_select_build.report.records.len()
            && character_select_build.auxiliary_records.len()
                == character_select_build.report.auxiliary_records.len()
            && character_select_build.overlays.len()
                == character_select_build.report.overlays.len(),
        "character-select build output count changed during readback"
    );
    let mut character_select_records = Vec::with_capacity(
        character_select_build.report.records.len()
            + character_select_build.report.auxiliary_records.len(),
    );
    for (record_offset, (report, expected)) in character_select_build
        .report
        .records
        .iter()
        .zip(&character_select_build.records)
        .enumerate()
    {
        let replacement_index = character_select_record_start_index + record_offset;
        let rebuilt_record = rebuilt
            .get(replacement_index)
            .context("character-select SELP replacement record disappeared")?;
        ensure!(
            replacements
                .get(replacement_index)
                .is_some_and(|replacement| replacement.path == report.source_path),
            "character-select SELP replacement order changed"
        );
        let (_, readback) = rebuild::read_record(temporary_bin, &report.source_path)?;
        ensure!(
            &readback == expected && sha256_bytes(&readback) == report.patched_stored_sha256,
            "{} character-select SELP readback changed bytes",
            report.source_path
        );
        let decoded = decode_built_record_streams(
            &readback,
            report
                .compressed_streams
                .iter()
                .map(|stream| (stream.encoded_stream_offset, stream.encoded_stream_capacity)),
        )?;
        let readback_decoded_sha256 = sha256_bytes(&decoded);
        ensure!(
            readback_decoded_sha256 == report.patched_decoded_sha256,
            "{} character-select SELP readback decoded to unexpected bytes",
            report.source_path
        );
        character_select_records.push(DialogueDiscAssetReadback {
            path: report.source_path.clone(),
            extent_lba: rebuilt_record.record.extent_lba,
            sector_count: rebuilt_record.target_lbas.len(),
            expected_stored_sha256: report.patched_stored_sha256.clone(),
            readback_stored_sha256: sha256_bytes(&readback),
            expected_decoded_sha256: report.patched_decoded_sha256.clone(),
            readback_decoded_sha256,
            readback_verified: true,
        });
    }

    let character_select_auxiliary_start_index =
        character_select_record_start_index + character_select_build.report.records.len();
    for (record_offset, (report, expected)) in character_select_build
        .report
        .auxiliary_records
        .iter()
        .zip(&character_select_build.auxiliary_records)
        .enumerate()
    {
        let replacement_index = character_select_auxiliary_start_index + record_offset;
        let rebuilt_record = rebuilt
            .get(replacement_index)
            .context("character-select auxiliary replacement record disappeared")?;
        ensure!(
            replacements
                .get(replacement_index)
                .is_some_and(|replacement| replacement.path == report.source_path),
            "character-select auxiliary replacement order changed"
        );
        let (_, readback) = rebuild::read_record(temporary_bin, &report.source_path)?;
        ensure!(
            &readback == expected && sha256_bytes(&readback) == report.patched_stored_sha256,
            "{} character-select auxiliary readback changed bytes",
            report.source_path
        );
        let decoded = decode_built_record_streams(
            &readback,
            report
                .compressed_streams
                .iter()
                .map(|stream| (stream.encoded_stream_offset, stream.encoded_stream_capacity)),
        )?;
        let readback_decoded_sha256 = sha256_bytes(&decoded);
        ensure!(
            readback_decoded_sha256 == report.patched_decoded_sha256,
            "{} character-select auxiliary readback decoded to unexpected bytes",
            report.source_path
        );
        character_select_records.push(DialogueDiscAssetReadback {
            path: report.source_path.clone(),
            extent_lba: rebuilt_record.record.extent_lba,
            sector_count: rebuilt_record.target_lbas.len(),
            expected_stored_sha256: report.patched_stored_sha256.clone(),
            readback_stored_sha256: sha256_bytes(&readback),
            expected_decoded_sha256: report.patched_decoded_sha256.clone(),
            readback_decoded_sha256,
            readback_verified: true,
        });
    }

    let character_select_overlay_start_index = character_select_auxiliary_start_index
        + character_select_build.report.auxiliary_records.len();
    let mut character_select_overlays =
        Vec::with_capacity(character_select_build.report.overlays.len());
    for (overlay_offset, (report, expected)) in character_select_build
        .report
        .overlays
        .iter()
        .zip(&character_select_build.overlays)
        .enumerate()
    {
        let replacement_index = character_select_overlay_start_index + overlay_offset;
        let rebuilt_record = rebuilt
            .get(replacement_index)
            .context("character-select PLSEL replacement record disappeared")?;
        ensure!(
            replacements
                .get(replacement_index)
                .is_some_and(|replacement| replacement.path == report.source_path),
            "character-select PLSEL replacement order changed"
        );
        let (_, readback) = rebuild::read_record(temporary_bin, &report.source_path)?;
        ensure!(
            &readback == expected && sha256_bytes(&readback) == report.patched_sha256,
            "{} character-select PLSEL readback changed bytes",
            report.source_path
        );
        character_select_overlays.push(DialogueDiscRecordReadback {
            path: report.source_path.clone(),
            extent_lba: rebuilt_record.record.extent_lba,
            sector_count: rebuilt_record.target_lbas.len(),
            expected_sha256: report.patched_sha256.clone(),
            readback_sha256: sha256_bytes(&readback),
            readback_verified: true,
        });
    }

    let mode_descendant_start_index =
        character_select_overlay_start_index + character_select_build.report.overlays.len();
    let final_mode_descendants = final_mode_descendant_records(
        mode_descendant_build,
        edit_runtime_text_build,
        practical_instruction_build,
    )?;
    ensure!(
        final_mode_descendants.len() == mode_descendant_build.report.records.len(),
        "mode-descendant readback source count changed"
    );
    let mut mode_descendant_records = Vec::with_capacity(final_mode_descendants.len());
    for (record_offset, final_record) in final_mode_descendants.iter().enumerate() {
        let replacement_index = mode_descendant_start_index + record_offset;
        let expected = &final_record.stored;
        let expected_path = final_record.report.source_path.as_str();
        ensure!(
            replacements
                .get(replacement_index)
                .is_some_and(|replacement| replacement.path == expected_path),
            "mode-descendant replacement order changed"
        );
        let rebuilt_record = rebuilt
            .get(replacement_index)
            .context("mode-descendant replacement record disappeared")?;
        let (_, readback) = rebuild::read_record(temporary_bin, expected_path)?;
        let expected_stored_sha256 = &final_record.stored_sha256;
        let expected_decoded_sha256 = &final_record.decoded_sha256;
        ensure!(
            readback.as_slice() == expected.as_slice()
                && sha256_bytes(&readback) == *expected_stored_sha256,
            "{expected_path} mode-descendant readback changed bytes"
        );
        let decoded = decode_mode_descendant_bytes(final_record.report.storage_kind, &readback)?;
        let readback_decoded_sha256 = sha256_bytes(&decoded);
        ensure!(
            readback_decoded_sha256 == *expected_decoded_sha256,
            "{expected_path} mode-descendant readback decoded to unexpected bytes"
        );
        mode_descendant_records.push(DialogueDiscAssetReadback {
            path: expected_path.to_owned(),
            extent_lba: rebuilt_record.record.extent_lba,
            sector_count: rebuilt_record.target_lbas.len(),
            expected_stored_sha256: expected_stored_sha256.to_string(),
            readback_stored_sha256: sha256_bytes(&readback),
            expected_decoded_sha256: expected_decoded_sha256.to_string(),
            readback_decoded_sha256,
            readback_verified: true,
        });
    }

    let practical_instruction_index =
        mode_descendant_start_index + mode_descendant_build.report.records.len();
    let practical_instruction_record = rebuilt
        .get(practical_instruction_index)
        .context("practical-instruction replacement record disappeared")?;
    ensure!(
        replacements
            .get(practical_instruction_index)
            .is_some_and(|replacement| replacement.path == PRACTICAL_INSTRUCTION_PATH),
        "practical-instruction replacement order changed"
    );
    let (_, practical_instruction_readback) =
        rebuild::read_record(temporary_bin, PRACTICAL_INSTRUCTION_PATH)?;
    ensure!(
        practical_instruction_readback == practical_instruction_build.record
            && sha256_bytes(&practical_instruction_readback)
                == practical_instruction_build.report.patched_record_sha256,
        "practical-instruction record readback changed bytes"
    );
    let practical_members = parse_tzz(&practical_instruction_readback)?;
    ensure!(
        practical_members.len() == practical_instruction_build.report.members.len(),
        "practical-instruction readback member denominator changed"
    );
    for (member, expected) in practical_members
        .iter()
        .zip(&practical_instruction_build.report.members)
    {
        ensure!(
            member.index == expected.member_index
                && sha256_bytes(&practical_instruction_readback[member.compressed_range()])
                    == expected.patched_compressed_sha256
                && sha256_bytes(&decompress(
                    &practical_instruction_readback[member.compressed_range()],
                    true,
                )?) == expected.patched_decoded_sha256,
            "practical-instruction member {} failed disc readback",
            member.index
        );
    }
    let practical_instruction = DialogueDiscRecordReadback {
        path: PRACTICAL_INSTRUCTION_PATH.to_string(),
        extent_lba: practical_instruction_record.record.extent_lba,
        sector_count: practical_instruction_record.target_lbas.len(),
        expected_sha256: practical_instruction_build
            .report
            .patched_record_sha256
            .clone(),
        readback_sha256: sha256_bytes(&practical_instruction_readback),
        readback_verified: true,
    };

    let practical_prompt_consumer_start_index = practical_instruction_index + 1;
    let mut practical_instruction_prompt_consumers =
        Vec::with_capacity(practical_instruction_build.prompt_consumers.len());
    for (consumer_offset, expected) in practical_instruction_build
        .prompt_consumers
        .iter()
        .enumerate()
    {
        let replacement_index = practical_prompt_consumer_start_index + consumer_offset;
        let rebuilt_record = rebuilt
            .get(replacement_index)
            .context("practical gameplay-prompt consumer replacement disappeared")?;
        ensure!(
            replacements
                .get(replacement_index)
                .is_some_and(|replacement| replacement.path == expected.path),
            "practical gameplay-prompt consumer replacement order changed"
        );
        let (_, readback) = rebuild::read_record(temporary_bin, &expected.path)?;
        ensure!(
            readback == expected.record
                && sha256_bytes(&readback) == expected.patched_record_sha256,
            "{} practical gameplay-prompt consumer readback changed bytes",
            expected.path
        );
        practical_instruction_prompt_consumers.push(DialogueDiscRecordReadback {
            path: expected.path.clone(),
            extent_lba: rebuilt_record.record.extent_lba,
            sector_count: rebuilt_record.target_lbas.len(),
            expected_sha256: expected.patched_record_sha256.clone(),
            readback_sha256: sha256_bytes(&readback),
            readback_verified: true,
        });
    }

    let menu_index =
        practical_prompt_consumer_start_index + practical_instruction_build.prompt_consumers.len();
    let menu_record = rebuilt
        .get(menu_index)
        .context("development MENU replacement record disappeared")?;
    ensure!(
        replacements
            .get(menu_index)
            .is_some_and(|replacement| replacement.path == MODE_SELECT_MENU_PATH),
        "development MENU replacement order changed"
    );
    let (_, menu_readback) = rebuild::read_record(temporary_bin, MODE_SELECT_MENU_PATH)?;
    ensure!(
        menu_readback == development_menu.stored,
        "development MENU record readback changed bytes"
    );
    let menu_decoded = decompress(&menu_readback, true)?;
    let menu = DialogueDiscAssetReadback {
        path: MODE_SELECT_MENU_PATH.to_string(),
        extent_lba: menu_record.record.extent_lba,
        sector_count: menu_record.target_lbas.len(),
        expected_stored_sha256: sha256_bytes(&development_menu.stored),
        readback_stored_sha256: sha256_bytes(&menu_readback),
        expected_decoded_sha256: sha256_bytes(&development_menu.decoded),
        readback_decoded_sha256: sha256_bytes(&menu_decoded),
        readback_verified: true,
    };
    ensure!(
        menu.readback_decoded_sha256 == menu.expected_decoded_sha256,
        "development MENU record decoded to unexpected bytes"
    );

    let options_overlay_index = menu_index + 1;
    let options_overlay_record = rebuilt
        .get(options_overlay_index)
        .context("options overlay replacement record disappeared")?;
    ensure!(
        replacements
            .get(options_overlay_index)
            .is_some_and(|replacement| replacement.path == OPTIONS_OVERLAY_PATH),
        "options overlay replacement order changed"
    );
    let (_, options_overlay_readback) = rebuild::read_record(temporary_bin, OPTIONS_OVERLAY_PATH)?;
    ensure!(
        options_overlay_readback == options_build.overlay,
        "options overlay record readback changed bytes"
    );
    let options_overlay = DialogueDiscRecordReadback {
        path: OPTIONS_OVERLAY_PATH.to_string(),
        extent_lba: options_overlay_record.record.extent_lba,
        sector_count: options_overlay_record.target_lbas.len(),
        expected_sha256: sha256_bytes(&options_build.overlay),
        readback_sha256: sha256_bytes(&options_overlay_readback),
        readback_verified: true,
    };

    let options_info_index = options_overlay_index + 1;
    let options_info_record = rebuilt
        .get(options_info_index)
        .context("options info replacement record disappeared")?;
    ensure!(
        replacements
            .get(options_info_index)
            .is_some_and(|replacement| replacement.path == OPTIONS_INFO_PATH),
        "options info replacement order changed"
    );
    let (_, options_info_readback) = rebuild::read_record(temporary_bin, OPTIONS_INFO_PATH)?;
    ensure!(
        options_info_readback == options_build.optinfo_stored,
        "options info record readback changed bytes"
    );
    let options_info_decoded = decompress(&options_info_readback, true)?;
    let options_info = DialogueDiscAssetReadback {
        path: OPTIONS_INFO_PATH.to_string(),
        extent_lba: options_info_record.record.extent_lba,
        sector_count: options_info_record.target_lbas.len(),
        expected_stored_sha256: sha256_bytes(&options_build.optinfo_stored),
        readback_stored_sha256: sha256_bytes(&options_info_readback),
        expected_decoded_sha256: sha256_bytes(&options_build.optinfo_decoded),
        readback_decoded_sha256: sha256_bytes(&options_info_decoded),
        readback_verified: true,
    };
    ensure!(
        options_info.readback_decoded_sha256 == options_info.expected_decoded_sha256,
        "options info record decoded to unexpected bytes"
    );

    let edit_runtime_text_overlay_index = options_info_index + 1;
    let edit_runtime_text_overlay_record = rebuilt
        .get(edit_runtime_text_overlay_index)
        .context("EDIT runtime text overlay replacement record disappeared")?;
    ensure!(
        replacements
            .get(edit_runtime_text_overlay_index)
            .is_some_and(|replacement| replacement.path == EDIT_RUNTIME_TEXT_OVERLAY_PATH),
        "EDIT runtime text overlay replacement order changed"
    );
    let (_, edit_runtime_text_overlay_readback) =
        rebuild::read_record(temporary_bin, EDIT_RUNTIME_TEXT_OVERLAY_PATH)?;
    ensure!(
        edit_runtime_text_overlay_readback == edit_runtime_text_build.overlay,
        "EDIT runtime text overlay readback changed bytes"
    );
    let edit_runtime_text_overlay = DialogueDiscRecordReadback {
        path: EDIT_RUNTIME_TEXT_OVERLAY_PATH.to_string(),
        extent_lba: edit_runtime_text_overlay_record.record.extent_lba,
        sector_count: edit_runtime_text_overlay_record.target_lbas.len(),
        expected_sha256: edit_runtime_text_build.report.output_overlay_sha256.clone(),
        readback_sha256: sha256_bytes(&edit_runtime_text_overlay_readback),
        readback_verified: true,
    };

    let edit_runtime_text_pass_index = edit_runtime_text_overlay_index + 1;
    let edit_runtime_text_pass_record = rebuilt
        .get(edit_runtime_text_pass_index)
        .context("EDIT runtime text PASS replacement record disappeared")?;
    ensure!(
        replacements
            .get(edit_runtime_text_pass_index)
            .is_some_and(|replacement| replacement.path == EDIT_RUNTIME_TEXT_PASS_PATH),
        "EDIT runtime text PASS replacement order changed"
    );
    let (_, edit_runtime_text_pass_readback) =
        rebuild::read_record(temporary_bin, EDIT_RUNTIME_TEXT_PASS_PATH)?;
    ensure!(
        edit_runtime_text_pass_readback == edit_runtime_text_build.pass,
        "EDIT runtime text PASS readback changed bytes"
    );
    let edit_runtime_text_pass = DialogueDiscRecordReadback {
        path: EDIT_RUNTIME_TEXT_PASS_PATH.to_string(),
        extent_lba: edit_runtime_text_pass_record.record.extent_lba,
        sector_count: edit_runtime_text_pass_record.target_lbas.len(),
        expected_sha256: edit_runtime_text_build.report.output_pass_sha256.clone(),
        readback_sha256: sha256_bytes(&edit_runtime_text_pass_readback),
        readback_verified: true,
    };

    let title_menu_overlay_index = edit_runtime_text_pass_index + 1;
    let title_menu_overlay_record = rebuilt
        .get(title_menu_overlay_index)
        .context("title-adjacent menu overlay replacement record disappeared")?;
    ensure!(
        replacements
            .get(title_menu_overlay_index)
            .is_some_and(|replacement| replacement.path == TITLE_MENU_OVERLAY_PATH),
        "title-adjacent menu overlay replacement order changed"
    );
    let (_, title_menu_overlay_readback) =
        rebuild::read_record(temporary_bin, TITLE_MENU_OVERLAY_PATH)?;
    ensure!(
        title_menu_overlay_readback == development_title_overlay.stored,
        "composed title overlay readback changed bytes"
    );
    let title_menu_overlay_decoded = decompress(&title_menu_overlay_readback, true)?;
    ensure!(
        title_menu_overlay_decoded == development_title_overlay.decoded,
        "composed title overlay decoded to unexpected bytes"
    );
    let title_menu_overlay = DialogueDiscAssetReadback {
        path: TITLE_MENU_OVERLAY_PATH.to_string(),
        extent_lba: title_menu_overlay_record.record.extent_lba,
        sector_count: title_menu_overlay_record.target_lbas.len(),
        expected_stored_sha256: sha256_bytes(&development_title_overlay.stored),
        readback_stored_sha256: sha256_bytes(&title_menu_overlay_readback),
        expected_decoded_sha256: sha256_bytes(&development_title_overlay.decoded),
        readback_decoded_sha256: sha256_bytes(&title_menu_overlay_decoded),
        readback_verified: true,
    };

    let main_executable_index = title_menu_overlay_index + 1;
    let main_executable_record = rebuilt
        .get(main_executable_index)
        .context("composed main executable replacement record disappeared")?;
    ensure!(
        replacements
            .get(main_executable_index)
            .is_some_and(|replacement| replacement.path == MAIN_EXECUTABLE_PATH),
        "composed main executable replacement order changed"
    );
    let (_, main_executable_readback) = rebuild::read_record(temporary_bin, MAIN_EXECUTABLE_PATH)?;
    ensure!(
        main_executable_readback == main_executable.data,
        "composed main executable readback changed bytes"
    );
    let main_executable_readback = DialogueDiscRecordReadback {
        path: MAIN_EXECUTABLE_PATH.to_string(),
        extent_lba: main_executable_record.record.extent_lba,
        sector_count: main_executable_record.target_lbas.len(),
        expected_sha256: main_executable.stored_sha256.clone(),
        readback_sha256: sha256_bytes(&main_executable_readback),
        readback_verified: true,
    };

    let dialogue_name_runtime_index = main_executable_index + 1;
    let dialogue_name_runtime_record = rebuilt
        .get(dialogue_name_runtime_index)
        .context("shared-name MGAME replacement record disappeared")?;
    ensure!(
        replacements
            .get(dialogue_name_runtime_index)
            .is_some_and(|replacement| replacement.path == MGAME_PATH),
        "shared-name MGAME replacement order changed"
    );
    let (_, dialogue_name_runtime_readback) = rebuild::read_record(temporary_bin, MGAME_PATH)?;
    ensure!(
        dialogue_name_runtime_readback == dialogue_name_runtime.bytes,
        "shared-name MGAME record readback changed bytes"
    );
    let dialogue_name_runtime_readback = DialogueDiscRecordReadback {
        path: MGAME_PATH.to_string(),
        extent_lba: dialogue_name_runtime_record.record.extent_lba,
        sector_count: dialogue_name_runtime_record.target_lbas.len(),
        expected_sha256: dialogue_name_runtime.report.patched_sha256.clone(),
        readback_sha256: sha256_bytes(&dialogue_name_runtime_readback),
        readback_verified: true,
    };

    let bonus_menu_index = dialogue_name_runtime_index + 1;
    let bonus_menu_record = rebuilt
        .get(bonus_menu_index)
        .context("bonus main-menu replacement record disappeared")?;
    ensure!(
        replacements
            .get(bonus_menu_index)
            .is_some_and(|replacement| replacement.path == BONUS_MENU_PATH),
        "bonus main-menu replacement order changed"
    );
    let (_, bonus_menu_readback) = rebuild::read_record(temporary_bin, BONUS_MENU_PATH)?;
    ensure!(
        bonus_menu_readback == bonus_menu_build.stored,
        "bonus main-menu record readback changed bytes"
    );
    let bonus_menu_decoded = decompress(&bonus_menu_readback, true)?;
    let bonus_menu = DialogueDiscAssetReadback {
        path: BONUS_MENU_PATH.to_string(),
        extent_lba: bonus_menu_record.record.extent_lba,
        sector_count: bonus_menu_record.target_lbas.len(),
        expected_stored_sha256: bonus_menu_build.report.patched_stored_sha256.clone(),
        readback_stored_sha256: sha256_bytes(&bonus_menu_readback),
        expected_decoded_sha256: bonus_menu_build.report.patched_decoded_sha256.clone(),
        readback_decoded_sha256: sha256_bytes(&bonus_menu_decoded),
        readback_verified: true,
    };
    ensure!(
        bonus_menu.readback_decoded_sha256 == bonus_menu.expected_decoded_sha256,
        "bonus main-menu record decoded to unexpected bytes"
    );

    let bonus_inventory_index = bonus_menu_index + 1;
    let bonus_inventory_record = rebuilt
        .get(bonus_inventory_index)
        .context("bonus-inventory replacement record disappeared")?;
    ensure!(
        replacements
            .get(bonus_inventory_index)
            .is_some_and(|replacement| replacement.path == BONUS_INVENTORY_PATH),
        "bonus-inventory replacement order changed"
    );
    let (_, bonus_inventory_readback) = rebuild::read_record(temporary_bin, BONUS_INVENTORY_PATH)?;
    ensure!(
        bonus_inventory_readback == bonus_inventory_surfaces.inventory_stored,
        "composed bonus-inventory record readback changed bytes"
    );
    let bonus_inventory_decoded = decompress(&bonus_inventory_readback, true)?;
    ensure!(
        bonus_inventory_decoded == bonus_inventory_surfaces.inventory_decoded,
        "composed bonus-inventory record decoded to unexpected bytes"
    );
    let bonus_inventory = DialogueDiscAssetReadback {
        path: BONUS_INVENTORY_PATH.to_string(),
        extent_lba: bonus_inventory_record.record.extent_lba,
        sector_count: bonus_inventory_record.target_lbas.len(),
        expected_stored_sha256: sha256_bytes(&bonus_inventory_surfaces.inventory_stored),
        readback_stored_sha256: sha256_bytes(&bonus_inventory_readback),
        expected_decoded_sha256: sha256_bytes(&bonus_inventory_surfaces.inventory_decoded),
        readback_decoded_sha256: sha256_bytes(&bonus_inventory_decoded),
        readback_verified: true,
    };
    ensure!(
        bonus_inventory.readback_decoded_sha256 == bonus_inventory.expected_decoded_sha256,
        "bonus-inventory record decoded to unexpected bytes"
    );

    let bonus_inventory_overlay_index = bonus_inventory_index + 1;
    let bonus_inventory_overlay_record = rebuilt
        .get(bonus_inventory_overlay_index)
        .context("bonus-inventory overlay replacement record disappeared")?;
    ensure!(
        replacements
            .get(bonus_inventory_overlay_index)
            .is_some_and(|replacement| {
                replacement.path == bonus_inventory_surfaces.overlay_path.as_str()
            }),
        "bonus-inventory overlay replacement order changed"
    );
    let (_, bonus_inventory_overlay_readback) =
        rebuild::read_record(temporary_bin, &bonus_inventory_surfaces.overlay_path)?;
    ensure!(
        bonus_inventory_overlay_readback == bonus_inventory_surfaces.overlay,
        "composed bonus-inventory overlay readback changed bytes"
    );
    let bonus_inventory_overlay = DialogueDiscRecordReadback {
        path: bonus_inventory_surfaces.overlay_path.clone(),
        extent_lba: bonus_inventory_overlay_record.record.extent_lba,
        sector_count: bonus_inventory_overlay_record.target_lbas.len(),
        expected_sha256: sha256_bytes(&bonus_inventory_surfaces.overlay),
        readback_sha256: sha256_bytes(&bonus_inventory_overlay_readback),
        readback_verified: true,
    };

    let shop_ui_index = bonus_inventory_overlay_index + 1;
    let shop_ui_record = rebuilt
        .get(shop_ui_index)
        .context("shop fixed-UI replacement record disappeared")?;
    ensure!(
        replacements
            .get(shop_ui_index)
            .is_some_and(|replacement| replacement.path == SHOP_UI_PATH),
        "shop fixed-UI replacement order changed"
    );
    let (_, shop_ui_readback) = rebuild::read_record(temporary_bin, SHOP_UI_PATH)?;
    ensure!(
        shop_ui_readback == shop_surfaces.shop_ui_stored,
        "composed shop UI record readback changed bytes"
    );
    let shop_ui_decoded = decompress(&shop_ui_readback, true)?;
    ensure!(
        shop_ui_decoded == shop_surfaces.shop_ui_decoded,
        "composed shop UI record decoded to unexpected bytes"
    );
    let shop_ui = DialogueDiscAssetReadback {
        path: SHOP_UI_PATH.to_string(),
        extent_lba: shop_ui_record.record.extent_lba,
        sector_count: shop_ui_record.target_lbas.len(),
        expected_stored_sha256: sha256_bytes(&shop_surfaces.shop_ui_stored),
        readback_stored_sha256: sha256_bytes(&shop_ui_readback),
        expected_decoded_sha256: sha256_bytes(&shop_surfaces.shop_ui_decoded),
        readback_decoded_sha256: sha256_bytes(&shop_ui_decoded),
        readback_verified: true,
    };
    ensure!(
        shop_ui.readback_decoded_sha256 == shop_ui.expected_decoded_sha256,
        "composed shop UI record decoded to unexpected bytes"
    );

    let shop_overlay_index = shop_ui_index + 1;
    let shop_overlay_record = rebuilt
        .get(shop_overlay_index)
        .context("shop exit-confirmation overlay replacement record disappeared")?;
    ensure!(
        replacements
            .get(shop_overlay_index)
            .is_some_and(|replacement| replacement.path == shop_surfaces.overlay_path.as_str()),
        "shop exit-confirmation overlay replacement order changed"
    );
    let (_, shop_overlay_readback) =
        rebuild::read_record(temporary_bin, &shop_surfaces.overlay_path)?;
    ensure!(
        shop_overlay_readback == shop_surfaces.overlay,
        "composed shop exit-confirmation overlay readback changed bytes"
    );
    let shop_overlay = DialogueDiscRecordReadback {
        path: shop_surfaces.overlay_path.clone(),
        extent_lba: shop_overlay_record.record.extent_lba,
        sector_count: shop_overlay_record.target_lbas.len(),
        expected_sha256: sha256_bytes(&shop_surfaces.overlay),
        readback_sha256: sha256_bytes(&shop_overlay_readback),
        readback_verified: true,
    };

    let battle_path = super::super::battle_name_build::PATH;
    let battle_index = replacements
        .iter()
        .position(|r| r.path == battle_path)
        .context("battle name supplier missing from final record set")?;
    let expected = &replacements[battle_index];
    let record = rebuilt
        .get(battle_index)
        .context("battle name rebuilt record missing")?;
    let (_, bytes) = rebuild::read_record(temporary_bin, battle_path)?;
    ensure!(
        bytes == expected.data,
        "battle name supplier readback differs"
    );
    let decoded = decompress(&bytes, true)?;
    let expected_decoded = decompress(expected.data, true)?;
    ensure!(
        decoded == expected_decoded,
        "battle name supplier decoded readback differs"
    );
    let battle_name = DialogueDiscAssetReadback {
        path: battle_path.into(),
        extent_lba: record.record.extent_lba,
        sector_count: record.target_lbas.len(),
        expected_stored_sha256: sha256_bytes(expected.data),
        readback_stored_sha256: sha256_bytes(&bytes),
        expected_decoded_sha256: sha256_bytes(&expected_decoded),
        readback_decoded_sha256: sha256_bytes(&decoded),
        readback_verified: true,
    };
    Ok(DialogueDiscReadbacks {
        battle_name,
        dialogue_assets,
        dialogue_bundles: dialogue_bundle_readbacks,
        name_entry_font,
        name_entry,
        diary_header,
        diary_scene,
        diary_scene_runtime_bundles,
        diary_artwork_archives,
        character_select_records,
        character_select_overlays,
        mode_descendant_records,
        practical_instruction,
        practical_instruction_prompt_consumers,
        menu,
        options_overlay,
        options_info,
        edit_runtime_text_overlay,
        edit_runtime_text_pass,
        title_menu_overlay,
        main_executable: main_executable_readback,
        dialogue_name_runtime: dialogue_name_runtime_readback,
        bonus_menu,
        bonus_inventory,
        bonus_inventory_overlay,
        shop_ui,
        shop_overlay,
    })
}
