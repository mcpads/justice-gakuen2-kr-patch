//! Prepared message data shared by preparation and product materialization.
use crate::paged_compression::PagedCompressionProfile;

#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct DialogueMessageBuildPlan {
    pub(super) stat_result_layout: super::stat_result_layout::StatResultLayout,
    pub(super) backup_slot_text: super::backup_slot_text::BackupSlotTextCounts,
    pub(super) primary_layout: super::dialogue_message_layout::PrimaryDialogueLayoutBuildReport,
    pub(super) source_bin_sha256: String,
    pub(super) codebook_sha256: String,
    pub(super) input_policy: String,
    pub(super) semantic_group_count: usize,
    pub(super) development_authored_group_count: usize,
    pub(super) selector_semantic_group_count: usize,
    pub(super) selector_development_authored_group_count: usize,
    pub(super) selector_development_full_input_available: bool,
    pub(super) release_candidate_selector_input_eligible: bool,
    pub(super) development_build_input_available: bool,
    pub(super) development_translation_input_available: bool,
    pub(super) release_candidate_translation_input_eligible: bool,
    pub(super) assets: Vec<DialogueMessageBuildPlanAsset>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct DialogueMessageBuildPlanAsset {
    pub(super) source_path: String,
    pub(super) original_decoded_sha256: String,
    pub(super) rebuilt_decoded_sha256: String,
    pub(super) original_stored_sha256: String,
    pub(super) original_stored_byte_count: usize,
    pub(super) source_catalog_prefix: [u8; 4],
    pub(super) source_compression_profile: PagedCompressionProfile,
    #[serde(skip)]
    pub(super) original_decoded: Vec<u8>,
    #[serde(skip)]
    pub(super) rebuilt_decoded: Vec<u8>,
    pub(super) bank_count: usize,
    pub(super) message_count: usize,
    pub(super) translated_message_count: usize,
    pub(super) rewritten_runtime_insertion_message_count: usize,
    pub(super) preserved_untranslated_message_count: usize,
    pub(super) preserved_unreferenced_message_count: usize,
    pub(super) message_arena_used_byte_count: usize,
    pub(super) message_arena_spare_byte_count: usize,
    pub(super) parse_back_verified: bool,
}
