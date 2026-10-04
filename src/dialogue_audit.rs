mod prepared_dialogue;
pub use prepared_dialogue::prepare_dialogue_inputs;
#[path = "dialogue_audit/atlas.rs"]
mod atlas;
#[path = "dialogue_audit/audit.rs"]
mod audit;
mod backup_slot_text;
#[path = "dialogue_audit/battle_name_build.rs"]
mod battle_name_build;
#[path = "dialogue_audit/bonus_inventory_composition.rs"]
mod bonus_inventory_composition;
#[path = "dialogue_audit/bonus_inventory_development_build.rs"]
mod bonus_inventory_development_build;
#[path = "dialogue_audit/codebook.rs"]
pub(crate) mod codebook;
#[path = "dialogue_audit/codebook_model.rs"]
mod codebook_model;
#[path = "dialogue_audit/codebook_review_application.rs"]
mod codebook_review_application;
#[path = "dialogue_audit/component_workers.rs"]
mod component_workers;
#[path = "dialogue_audit/corpus.rs"]
mod corpus;
#[path = "dialogue_audit/corpus_codec.rs"]
mod corpus_codec;
#[path = "dialogue_audit/corpus_digest.rs"]
mod corpus_digest;
#[path = "dialogue_audit/corpus_model.rs"]
mod corpus_model;
#[path = "dialogue_audit/corpus_writer.rs"]
mod corpus_writer;
#[path = "dialogue_audit/development_menu_build.rs"]
mod development_menu_build;
#[path = "dialogue_audit/development_title_overlay_build.rs"]
mod development_title_overlay_build;
#[path = "dialogue_audit/dialogue_build_capacity.rs"]
mod dialogue_build_capacity;
#[path = "dialogue_audit/dialogue_build_capacity_model.rs"]
mod dialogue_build_capacity_model;
#[path = "dialogue_audit/dialogue_bundle.rs"]
mod dialogue_bundle;
#[path = "dialogue_audit/dialogue_code_allocation.rs"]
mod dialogue_code_allocation;
#[path = "dialogue_audit/dialogue_code_allocation_model.rs"]
mod dialogue_code_allocation_model;
#[path = "dialogue_audit/dialogue_consumer_union.rs"]
mod dialogue_consumer_union;
#[path = "dialogue_audit/dialogue_consumer_union_model.rs"]
mod dialogue_consumer_union_model;
#[path = "dialogue_audit/dialogue_development_input.rs"]
mod dialogue_development_input;
#[path = "dialogue_audit/dialogue_development_runtime.rs"]
mod dialogue_development_runtime;
#[path = "dialogue_audit/dialogue_development_runtime_model.rs"]
mod dialogue_development_runtime_model;
#[path = "dialogue_audit/dialogue_disc_build.rs"]
mod dialogue_disc_build;
#[path = "dialogue_audit/dialogue_disc_build_model.rs"]
mod dialogue_disc_build_model;
#[path = "dialogue_audit/dialogue_disc_finalization.rs"]
mod dialogue_disc_finalization;
#[path = "dialogue_audit/dialogue_disc_outputs.rs"]
mod dialogue_disc_outputs;
#[path = "dialogue_audit/dialogue_disc_records.rs"]
mod dialogue_disc_records;
#[path = "dialogue_audit/dialogue_fixed_code_consumers.rs"]
mod dialogue_fixed_code_consumers;
#[path = "dialogue_audit/dialogue_fixed_code_consumers_model.rs"]
mod dialogue_fixed_code_consumers_model;
#[path = "dialogue_audit/dialogue_font.rs"]
mod dialogue_font;
#[path = "dialogue_audit/dialogue_font_allocation.rs"]
mod dialogue_font_allocation;
#[path = "dialogue_audit/dialogue_font_build.rs"]
mod dialogue_font_build;
#[path = "dialogue_audit/dialogue_font_build_model.rs"]
mod dialogue_font_build_model;
#[path = "dialogue_audit/dialogue_font_conflict_attribution.rs"]
mod dialogue_font_conflict_attribution;
#[path = "dialogue_audit/dialogue_font_conflict_writer.rs"]
mod dialogue_font_conflict_writer;
#[path = "dialogue_audit/dialogue_font_conflicts.rs"]
mod dialogue_font_conflicts;
#[path = "dialogue_audit/dialogue_font_conflicts_model.rs"]
mod dialogue_font_conflicts_model;
#[path = "dialogue_audit/dialogue_message_build.rs"]
mod dialogue_message_build;
#[path = "dialogue_audit/dialogue_message_build_model.rs"]
mod dialogue_message_build_model;
#[path = "dialogue_audit/dialogue_message_encoding.rs"]
mod dialogue_message_encoding;
#[path = "dialogue_audit/dialogue_message_layout.rs"]
mod dialogue_message_layout;
mod dialogue_message_plan;
mod dialogue_message_plan_model;
#[path = "dialogue_audit/dialogue_message_rebuild.rs"]
mod dialogue_message_rebuild;
#[path = "dialogue_audit/dialogue_name_runtime_build.rs"]
mod dialogue_name_runtime_build;
#[path = "dialogue_audit/dialogue_runtime_entry_hook.rs"]
mod dialogue_runtime_entry_hook;
#[path = "dialogue_audit/dialogue_translation_assets.rs"]
mod dialogue_translation_assets;
#[path = "dialogue_audit/dialogue_translation_assets_model.rs"]
mod dialogue_translation_assets_model;
#[path = "dialogue_audit/dialogue_translation_input.rs"]
mod dialogue_translation_input;
#[path = "dialogue_audit/diary_save_slot_marker_flow.rs"]
mod diary_save_slot_marker_flow;
#[path = "dialogue_audit/format.rs"]
mod format;
#[path = "dialogue_audit/glossary.rs"]
mod glossary;
#[path = "dialogue_audit/glossary_model.rs"]
mod glossary_model;
#[path = "dialogue_audit/layout_measurement.rs"]
mod layout_measurement;
#[path = "dialogue_audit/layout_model.rs"]
mod layout_model;
#[path = "dialogue_audit/main_executable_record.rs"]
mod main_executable_record;
#[path = "dialogue_audit/menu_scene_components.rs"]
mod menu_scene_components;
#[path = "dialogue_audit/mode_descendant_composition.rs"]
mod mode_descendant_composition;
#[path = "dialogue_audit/model.rs"]
mod model;
#[path = "dialogue_audit/name_companion_reference_audit.rs"]
mod name_companion_reference_audit;
#[path = "dialogue_audit/name_companion_reference_model.rs"]
mod name_companion_reference_model;
#[path = "dialogue_audit/name_entry.rs"]
mod name_entry;
#[path = "dialogue_audit/name_entry_application_form.rs"]
mod name_entry_application_form;
#[path = "dialogue_audit/name_entry_build.rs"]
mod name_entry_build;
#[path = "dialogue_audit/name_entry_build_model.rs"]
mod name_entry_build_model;
#[path = "dialogue_audit/name_entry_candidate_controls.rs"]
mod name_entry_candidate_controls;
#[path = "dialogue_audit/name_entry_choice_descriptors.rs"]
mod name_entry_choice_descriptors;
#[path = "dialogue_audit/name_entry_composed_build.rs"]
mod name_entry_composed_build;
#[path = "dialogue_audit/name_entry_confirmation_hook.rs"]
mod name_entry_confirmation_hook;
#[path = "dialogue_audit/name_entry_defaults.rs"]
mod name_entry_defaults;
#[path = "dialogue_audit/name_entry_delete_handler.rs"]
mod name_entry_delete_handler;
#[path = "dialogue_audit/name_entry_favorite_word_choices.rs"]
mod name_entry_favorite_word_choices;
#[path = "dialogue_audit/name_entry_fixed_graphics_build.rs"]
mod name_entry_fixed_graphics_build;
#[path = "dialogue_audit/name_entry_fixed_graphics_model.rs"]
mod name_entry_fixed_graphics_model;
#[path = "dialogue_audit/name_entry_font_build.rs"]
mod name_entry_font_build;
#[path = "dialogue_audit/name_runtime_components.rs"]
mod name_runtime_components;
#[path = "dialogue_audit/quiz_answer_layout.rs"]
mod quiz_answer_layout;
pub(crate) use name_entry_font_build::{
    FONT_TIM_OFFSET as NAME_ENTRY_FONT_TIM_OFFSET, NAME_ENTRY_FONT_PATH,
    SOURCE_DECODED_SIZE as NAME_ENTRY_FONT_DECODED_SIZE,
    SOURCE_STORED_SHA256 as NAME_ENTRY_FONT_STORED_SHA256,
};
#[path = "dialogue_audit/name_entry_face_features.rs"]
mod name_entry_face_features;
#[path = "dialogue_audit/name_entry_font_build_model.rs"]
mod name_entry_font_build_model;
#[path = "dialogue_audit/name_entry_input.rs"]
mod name_entry_input;
#[path = "dialogue_audit/name_entry_keyboard_build.rs"]
mod name_entry_keyboard_build;
#[path = "dialogue_audit/name_entry_model.rs"]
mod name_entry_model;
#[path = "dialogue_audit/name_entry_nickname_companion.rs"]
mod name_entry_nickname_companion;
#[path = "dialogue_audit/name_entry_nickname_hangul_page.rs"]
mod name_entry_nickname_hangul_page;
#[path = "dialogue_audit/name_entry_overlay_compositor.rs"]
mod name_entry_overlay_compositor;
#[path = "dialogue_audit/name_entry_record_compositor.rs"]
mod name_entry_record_compositor;
#[path = "dialogue_audit/name_entry_record_transport.rs"]
mod name_entry_record_transport;
#[path = "dialogue_audit/name_entry_redisplay_hook.rs"]
mod name_entry_redisplay_hook;
#[path = "dialogue_audit/name_entry_redisplay_runtime.rs"]
mod name_entry_redisplay_runtime;
#[path = "dialogue_audit/name_entry_renderer.rs"]
mod name_entry_renderer;
#[path = "dialogue_audit/name_entry_runtime_code.rs"]
mod name_entry_runtime_code;
#[path = "dialogue_audit/name_entry_save_transport.rs"]
mod name_entry_save_transport;
#[path = "dialogue_audit/name_entry_school_choices.rs"]
mod name_entry_school_choices;
#[path = "dialogue_audit/name_entry_selection_hook.rs"]
mod name_entry_selection_hook;
#[path = "dialogue_audit/name_entry_sex_choices.rs"]
mod name_entry_sex_choices;
#[path = "dialogue_audit/name_entry_slot_navigation_guard.rs"]
mod name_entry_slot_navigation_guard;
#[path = "dialogue_audit/name_entry_subject_choices.rs"]
mod name_entry_subject_choices;
#[path = "dialogue_audit/parser.rs"]
mod parser;
#[path = "dialogue_audit/preview.rs"]
mod preview;
#[path = "dialogue_audit/review.rs"]
mod review;
#[path = "dialogue_audit/review_context.rs"]
mod review_context;
#[path = "dialogue_audit/review_model.rs"]
mod review_model;
#[path = "dialogue_audit/runtime_insertion_messages.rs"]
mod runtime_insertion_messages;
#[path = "dialogue_audit/runtime_insertions.rs"]
mod runtime_insertions;
#[path = "dialogue_audit/script.rs"]
mod script;
#[path = "dialogue_audit/script_message_contract.rs"]
mod script_message_contract;
#[path = "dialogue_audit/script_model.rs"]
mod script_model;
#[path = "dialogue_audit/script_opcode.rs"]
mod script_opcode;
#[path = "dialogue_audit/script_runtime.rs"]
mod script_runtime;
#[path = "dialogue_audit/script_source.rs"]
mod script_source;
#[path = "dialogue_audit/script_topology.rs"]
mod script_topology;
#[path = "dialogue_audit/selector_address_flow.rs"]
mod selector_address_flow;
#[path = "dialogue_audit/selector_address_flow_model.rs"]
mod selector_address_flow_model;
#[path = "dialogue_audit/selector_consumer_spec.rs"]
mod selector_consumer_spec;
#[path = "dialogue_audit/selector_consumers_model.rs"]
mod selector_consumers_model;
#[path = "dialogue_audit/selector_translation_audit.rs"]
mod selector_translation_audit;
#[path = "dialogue_audit/selector_translation_migration.rs"]
mod selector_translation_migration;
#[path = "dialogue_audit/selector_translation_model.rs"]
mod selector_translation_model;
#[path = "dialogue_audit/selector_translation_source.rs"]
mod selector_translation_source;
#[path = "dialogue_audit/selector_translation_validation.rs"]
mod selector_translation_validation;
#[path = "dialogue_audit/selector_translation_workspace.rs"]
mod selector_translation_workspace;
#[path = "dialogue_audit/selector_translation_writer.rs"]
mod selector_translation_writer;
#[path = "dialogue_audit/shared_name_runtime_build.rs"]
mod shared_name_runtime_build;
#[path = "dialogue_audit/shared_name_runtime_build_model.rs"]
mod shared_name_runtime_build_model;
#[path = "dialogue_audit/shop_development_build.rs"]
mod shop_development_build;
#[path = "dialogue_audit/shop_surface_composition.rs"]
mod shop_surface_composition;
#[path = "dialogue_audit/sources.rs"]
mod sources;
#[path = "dialogue_audit/summary.rs"]
mod summary;
#[path = "dialogue_audit/tokens.rs"]
mod tokens;
#[path = "dialogue_audit/translation.rs"]
mod translation;
#[path = "dialogue_audit/translation_decision_hash.rs"]
mod translation_decision_hash;
#[path = "dialogue_audit/translation_model.rs"]
mod translation_model;
#[path = "dialogue_audit/translation_validation.rs"]
mod translation_validation;
#[path = "dialogue_audit/translation_workspace.rs"]
mod translation_workspace;
#[path = "dialogue_audit/translation_workspace_asset_index.rs"]
mod translation_workspace_asset_index;
#[path = "dialogue_audit/translation_workspace_audit.rs"]
mod translation_workspace_audit;
#[path = "dialogue_audit/translation_workspace_io.rs"]
mod translation_workspace_io;
#[path = "dialogue_audit/translation_workspace_layout.rs"]
mod translation_workspace_layout;
#[path = "dialogue_audit/translation_workspace_migration.rs"]
mod translation_workspace_migration;
#[path = "dialogue_audit/translation_workspace_model.rs"]
mod translation_workspace_model;
#[path = "dialogue_audit/translation_workspace_primary_scripts.rs"]
mod translation_workspace_primary_scripts;
#[path = "dialogue_audit/translation_workspace_source.rs"]
mod translation_workspace_source;
#[path = "dialogue_audit/translation_workspace_validation.rs"]
mod translation_workspace_validation;
#[path = "dialogue_audit/translation_workspace_writer.rs"]
mod translation_workspace_writer;

#[cfg(test)]
#[path = "dialogue_audit/atlas_tests.rs"]
mod atlas_tests;
#[cfg(test)]
#[path = "dialogue_audit/codebook_tests.rs"]
mod codebook_tests;
#[cfg(test)]
#[path = "dialogue_audit/corpus_digest_tests.rs"]
mod corpus_digest_tests;
#[cfg(test)]
#[path = "dialogue_audit/corpus_tests.rs"]
mod corpus_tests;
#[cfg(test)]
#[path = "dialogue_audit/dialogue_bundle_tests.rs"]
mod dialogue_bundle_tests;
#[cfg(test)]
#[path = "dialogue_audit/dialogue_code_allocation_tests.rs"]
mod dialogue_code_allocation_tests;
#[cfg(test)]
#[path = "dialogue_audit/dialogue_consumer_union_tests.rs"]
mod dialogue_consumer_union_tests;
#[cfg(test)]
#[path = "dialogue_audit/dialogue_development_runtime_tests.rs"]
mod dialogue_development_runtime_tests;
#[cfg(test)]
#[path = "dialogue_audit/dialogue_disc_finalization_tests.rs"]
mod dialogue_disc_finalization_tests;
#[cfg(test)]
#[path = "dialogue_audit/dialogue_disc_outputs_tests.rs"]
mod dialogue_disc_outputs_tests;
#[cfg(test)]
#[path = "dialogue_audit/dialogue_disc_records_tests.rs"]
mod dialogue_disc_records_tests;
#[cfg(test)]
#[path = "dialogue_audit/dialogue_fixed_code_consumers_tests.rs"]
mod dialogue_fixed_code_consumers_tests;
#[cfg(test)]
#[path = "dialogue_audit/dialogue_font_allocation_tests.rs"]
mod dialogue_font_allocation_tests;
#[cfg(test)]
#[path = "dialogue_audit/dialogue_font_build_tests.rs"]
mod dialogue_font_build_tests;
#[cfg(test)]
#[path = "dialogue_audit/dialogue_font_conflict_writer_tests.rs"]
mod dialogue_font_conflict_writer_tests;
#[cfg(test)]
#[path = "dialogue_audit/dialogue_font_conflicts_tests.rs"]
mod dialogue_font_conflicts_tests;
#[cfg(test)]
#[path = "dialogue_audit/dialogue_font_tests.rs"]
mod dialogue_font_tests;
#[cfg(test)]
#[path = "dialogue_audit/dialogue_message_build_tests.rs"]
mod dialogue_message_build_tests;
#[cfg(test)]
#[path = "dialogue_audit/dialogue_message_encoding_tests.rs"]
mod dialogue_message_encoding_tests;
#[cfg(test)]
#[path = "dialogue_audit/dialogue_message_rebuild_tests.rs"]
mod dialogue_message_rebuild_tests;
#[cfg(test)]
#[path = "dialogue_audit/dialogue_name_runtime_build_tests.rs"]
mod dialogue_name_runtime_build_tests;
#[cfg(test)]
#[path = "dialogue_audit/diary_save_slot_marker_flow_tests.rs"]
mod diary_save_slot_marker_flow_tests;
#[cfg(test)]
#[path = "dialogue_audit/glossary_tests.rs"]
mod glossary_tests;
#[cfg(test)]
#[path = "dialogue_audit/layout_tests.rs"]
mod layout_tests;
#[cfg(test)]
#[path = "dialogue_audit/name_companion_reference_audit_tests.rs"]
mod name_companion_reference_audit_tests;
#[cfg(test)]
#[path = "dialogue_audit/name_entry_build_tests.rs"]
mod name_entry_build_tests;
#[cfg(test)]
#[path = "dialogue_audit/name_entry_choice_descriptors_tests.rs"]
mod name_entry_choice_descriptors_tests;
#[cfg(test)]
#[path = "dialogue_audit/name_entry_confirmation_hook_tests.rs"]
mod name_entry_confirmation_hook_tests;
#[cfg(test)]
#[path = "dialogue_audit/name_entry_delete_handler_tests.rs"]
mod name_entry_delete_handler_tests;
#[cfg(test)]
#[path = "dialogue_audit/name_entry_fixed_graphics_build_tests.rs"]
mod name_entry_fixed_graphics_build_tests;
#[cfg(test)]
#[path = "dialogue_audit/name_entry_font_build_tests.rs"]
mod name_entry_font_build_tests;
#[cfg(test)]
#[path = "dialogue_audit/name_entry_input_tests.rs"]
mod name_entry_input_tests;
#[cfg(test)]
#[path = "dialogue_audit/name_entry_keyboard_build_tests.rs"]
mod name_entry_keyboard_build_tests;
#[cfg(test)]
#[path = "dialogue_audit/name_entry_nickname_companion_tests.rs"]
mod name_entry_nickname_companion_tests;
#[cfg(test)]
#[path = "dialogue_audit/name_entry_nickname_hangul_page_tests.rs"]
mod name_entry_nickname_hangul_page_tests;
#[cfg(test)]
#[path = "dialogue_audit/name_entry_record_transport_tests.rs"]
mod name_entry_record_transport_tests;
#[cfg(test)]
#[path = "dialogue_audit/name_entry_redisplay_hook_tests.rs"]
mod name_entry_redisplay_hook_tests;
#[cfg(test)]
#[path = "dialogue_audit/name_entry_redisplay_runtime_tests.rs"]
mod name_entry_redisplay_runtime_tests;
#[cfg(test)]
#[path = "dialogue_audit/name_entry_renderer_tests.rs"]
mod name_entry_renderer_tests;
#[cfg(test)]
#[path = "dialogue_audit/name_entry_runtime_code_tests.rs"]
mod name_entry_runtime_code_tests;
#[cfg(test)]
#[path = "dialogue_audit/name_entry_save_transport_tests.rs"]
mod name_entry_save_transport_tests;
#[cfg(test)]
#[path = "dialogue_audit/name_entry_selection_hook_tests.rs"]
mod name_entry_selection_hook_tests;
#[cfg(test)]
#[path = "dialogue_audit/name_entry_sex_choices_tests.rs"]
mod name_entry_sex_choices_tests;
#[cfg(test)]
#[path = "dialogue_audit/name_entry_slot_navigation_guard_tests.rs"]
mod name_entry_slot_navigation_guard_tests;
#[cfg(test)]
#[path = "dialogue_audit/name_entry_test_fixture.rs"]
mod name_entry_test_fixture;
#[cfg(test)]
#[path = "dialogue_audit/name_entry_tests.rs"]
mod name_entry_tests;
#[cfg(test)]
#[path = "dialogue_audit/parser_tests.rs"]
mod parser_tests;
#[cfg(test)]
#[path = "dialogue_audit/preview_tests.rs"]
mod preview_tests;
#[cfg(test)]
#[path = "dialogue_audit/review_tests.rs"]
mod review_tests;
#[cfg(test)]
#[path = "dialogue_audit/runtime_insertion_messages_tests.rs"]
mod runtime_insertion_messages_tests;
#[cfg(test)]
#[path = "dialogue_audit/runtime_insertions_tests.rs"]
mod runtime_insertions_tests;
#[cfg(test)]
#[path = "dialogue_audit/script_message_contract_tests.rs"]
mod script_message_contract_tests;
#[cfg(test)]
#[path = "dialogue_audit/script_tests.rs"]
mod script_tests;
#[cfg(test)]
#[path = "dialogue_audit/selector_address_flow_tests.rs"]
mod selector_address_flow_tests;
#[cfg(test)]
#[path = "dialogue_audit/selector_translation_migration_tests.rs"]
mod selector_translation_migration_tests;
#[cfg(test)]
#[path = "dialogue_audit/selector_translation_tests.rs"]
mod selector_translation_tests;
#[cfg(test)]
#[path = "dialogue_audit/shared_name_runtime_build_tests.rs"]
mod shared_name_runtime_build_tests;
#[cfg(test)]
#[path = "dialogue_audit/summary_tests.rs"]
mod summary_tests;
#[cfg(test)]
#[path = "dialogue_audit/tokens_tests.rs"]
mod tokens_tests;
#[cfg(test)]
#[path = "dialogue_audit/translation_tests.rs"]
mod translation_tests;
#[cfg(test)]
#[path = "dialogue_audit/translation_workspace_layout_tests.rs"]
mod translation_workspace_layout_tests;
#[cfg(test)]
#[path = "dialogue_audit/translation_workspace_tests.rs"]
mod translation_workspace_tests;

pub use audit::audit_dialogue_assets;
pub use codebook::audit_dialogue_codebook;
pub use codebook_model::{DialogueCodebookAuditConfig, DialogueCodebookAuditManifest};
pub use codebook_review_application::{
    DialogueCodebookReviewApplicationConfig, DialogueCodebookReviewApplicationReport,
    apply_dialogue_codebook_review,
};
pub use corpus::build_dialogue_source_corpus;
pub use corpus_model::{
    DialogueSourceCorpus, DialogueSourceCorpusBuildReport, DialogueSourceCorpusConfig,
};
pub use dialogue_build_capacity::audit_dialogue_build_capacity;
pub use dialogue_build_capacity_model::{
    DialogueBuildCapacityAssetAudit, DialogueBuildCapacityAuditConfig,
    DialogueBuildCapacityAuditReport, DialogueBuildCapacityBankAudit,
};
pub use dialogue_code_allocation::build_dialogue_code_allocation;
pub use dialogue_code_allocation_model::{
    DialogueCharacterCodeAssignment, DialogueCodeAllocationAsset, DialogueCodeAllocationConfig,
    DialogueCodeAllocationReport,
};
pub use dialogue_consumer_union::audit_dialogue_consumer_union;
pub use dialogue_consumer_union_model::{
    DialogueConsumerUnionAuditConfig, DialogueConsumerUnionAuditReport,
};
pub use dialogue_development_runtime::audit_dialogue_development_runtime;
pub use dialogue_development_runtime_model::{
    DialogueDevelopmentRuntimeAuditConfig, DialogueDevelopmentRuntimeAuditReport,
};
pub use dialogue_disc_build::build_dialogue_development_disc;
pub use dialogue_disc_build_model::{
    DialogueDiscAssetReadback, DialogueDiscBuildConfig, DialogueDiscBuildReport,
    DialogueDiscRecordReadback,
};
pub use dialogue_fixed_code_consumers_model::{
    DialogueFixedCodeConsumerAssetAudit, DialogueFixedCodeConsumerAuditReport,
};
pub use dialogue_font::{
    DialogueExtensionGlyphInstall, install_dialogue_allocated_glyph,
    install_dialogue_extension_glyph,
};
pub use dialogue_font_allocation::{
    DialogueFontAllocationAuditConfig, DialogueFontAllocationAuditReport,
    DialogueFontAssetAllocationAudit, audit_dialogue_font_allocation,
};
pub use dialogue_font_build::build_dialogue_font_images;
pub use dialogue_font_build_model::{
    DialogueFontBuild, DialogueFontBuildAsset, DialogueFontBuildConfig, DialogueFontBuildReport,
    DialogueFontRecordBuild,
};
pub use dialogue_font_conflicts::audit_dialogue_font_conflicts;
pub use dialogue_font_conflicts_model::{
    DialogueFontConflictAssetManifest, DialogueFontConflictAssetRef,
    DialogueFontConflictAuditConfig, DialogueFontConflictAuditManifest, DialogueFontConflictCode,
    DialogueFontConflictCodeShard, DialogueFontConflictGroup, DialogueFontConflictGroupAsset,
    DialogueFontConflictGroupShard, DialogueFontConflictRequirementShard,
    DialogueFontConflictShardRef,
};
pub use dialogue_message_build::prepare_dialogue_message_images;
pub use dialogue_message_build_model::{
    DialogueMessageBuildAsset, DialogueMessageBuildConfig, DialogueMessageBuildReport,
};
pub use dialogue_translation_assets::{
    audit_dialogue_translation_assets, sync_dialogue_translation_assets,
};
pub use dialogue_translation_assets_model::{
    DialogueTranslationAssetAuditConfig, DialogueTranslationAssetReport,
    DialogueTranslationAssetSyncConfig,
};
pub use glossary::audit_dialogue_glossary;
pub use glossary_model::{DialogueGlossaryAuditConfig, DialogueGlossaryAuditReport};
pub use layout_model::{DialogueLayoutAuditConfig, DialogueLayoutAuditReport};
pub use model::{
    DialogueAuditConfig, DialogueAuditManifest, DialogueFontPreviewConfig,
    DialogueFontPreviewManifest,
};
pub use name_companion_reference_audit::audit_name_companion_references;
pub use name_companion_reference_model::{
    NameCompanionReferenceAuditConfig, NameCompanionReferenceAuditReport,
    NameCompanionStaticReference,
};
pub use name_entry::audit_dialogue_name_entry;
pub use name_entry_build::build_dialogue_name_entry_image;
pub use name_entry_build_model::{
    DialogueNameEntryBuildConfig, DialogueNameEntryBuildReport,
    DialogueNameEntryCandidateAssignment, DialogueNameEntryCandidatePageReport,
};
pub use name_entry_composed_build::{
    ComposedNameInputBuild, ComposedNameInputBuildConfig, ComposedNameInputBuildReport,
    build_composed_name_input,
};
pub use name_entry_fixed_graphics_model::{
    DialogueNameEntryFixedGraphicBuildReport, DialogueNameEntryFixedGraphicInstall,
};
pub use name_entry_font_build_model::{
    DialogueNameEntryFontBuildReport, DialogueNameEntryGlyphInstall,
};
pub use name_entry_model::{
    DialogueNameEntryAuditConfig, DialogueNameEntryAuditReport, DialogueNameEntryInputAudit,
    DialogueNameEntryPageAudit, DialogueNameFieldAudit,
};
pub(crate) use parser::DECODED_IMAGE_SIZE;
pub use preview::build_dialogue_font_previews;
pub use review::build_dialogue_codebook_review;
pub use review_model::{DialogueCodebookReviewConfig, DialogueCodebookReviewManifest};
pub use script::audit_dialogue_scenes;
pub use script_model::{DialogueSceneAuditConfig, DialogueSceneAuditReport};
pub use selector_address_flow::audit_dialogue_selector_address_flow;
pub use selector_address_flow_model::{
    DialogueSelectorAddressFlowAuditConfig, DialogueSelectorAddressFlowAuditReport,
};
pub use selector_translation_audit::audit_dialogue_selector_translation;
pub use selector_translation_migration::refresh_dialogue_selector_translation;
pub use selector_translation_model::{
    DialogueSelectorTranslationAuditConfig, DialogueSelectorTranslationAuditReport,
    DialogueSelectorTranslationInitConfig, DialogueSelectorTranslationRefreshConfig,
    DialogueSelectorTranslationScope, DialogueSelectorTranslationWorkspaceManifest,
};
pub use selector_translation_workspace::initialize_dialogue_selector_translation;
pub use translation::{audit_dialogue_translation, initialize_dialogue_translation};
pub use translation_decision_hash::{
    DialogueTranslationDecisionHashConfig, DialogueTranslationDecisionHashReport,
    hash_dialogue_translation_decisions,
};
pub use translation_model::{
    DialogueDevelopmentInputPolicy, DialogueTranslationAuditConfig, DialogueTranslationAuditReport,
    DialogueTranslationInitConfig, DialogueTranslationInput, DialogueTranslationRefreshConfig,
    DialogueTranslationScope,
};
pub use translation_workspace_layout::audit_dialogue_layout;
pub use translation_workspace_migration::refresh_dialogue_translation;
pub use translation_workspace_model::{
    DialogueTranslationAssetRepertoire, DialogueTranslationWorkspaceAuditReport,
    DialogueTranslationWorkspaceManifest,
};

#[path = "dialogue_audit/stat_result_layout.rs"]
mod stat_result_layout;

#[cfg(test)]
pub(crate) use name_entry_selection_hook::build_name_entry_selection_replacement;

#[path = "dialogue_audit/name_entry_control_labels.rs"]
mod name_entry_control_labels;

#[cfg(test)]
#[path = "dialogue_audit/name_entry_control_labels_tests.rs"]
mod name_entry_control_labels_tests;

#[path = "dialogue_audit/choice_columns.rs"]
mod choice_columns;
