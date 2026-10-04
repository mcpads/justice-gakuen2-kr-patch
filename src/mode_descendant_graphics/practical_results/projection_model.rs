//! Fail-closed data model for practical-result residency, CLUT, and consumer projections.

use std::fmt;

use serde::Deserialize;

use super::model::{PhysicalRegionId, SourceReferenceId};
use super::source_atlas_domain_model::SourceAtlasDomainId;

macro_rules! string_id {
    ($name:ident) => {
        #[derive(Debug, Clone, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
        #[serde(transparent)]
        pub(super) struct $name(String);

        impl $name {
            pub(super) fn as_str(&self) -> &str {
                &self.0
            }

            pub(super) fn is_empty(&self) -> bool {
                self.0.is_empty()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

string_id!(PracticalResultVramResidencyId);
string_id!(PracticalResultClutBindingId);
string_id!(PracticalResultConsumerId);
string_id!(PracticalResultConsumerDescriptorId);
string_id!(PracticalResultConsumerProjectionId);
string_id!(PracticalResultActionConsumerOccurrenceId);
string_id!(PracticalResultSemanticEntryId);
string_id!(PracticalResultProtectedRegionId);

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultActionConsumerOccurrenceCatalog {
    pub(super) kind: PracticalResultActionConsumerOccurrenceCatalogKind,
    pub(super) scope: PracticalResultActionConsumerOccurrenceCatalogScope,
    pub(super) occurrences: Vec<PracticalResultActionConsumerOccurrence>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub(super) enum PracticalResultActionConsumerOccurrenceCatalogKind {
    #[serde(rename = "justice_gakuen2_practical_result_action_consumer_occurrence_catalog")]
    JusticeGakuen2PracticalResultActionConsumerOccurrenceCatalog,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub(super) enum PracticalResultActionConsumerOccurrenceCatalogScope {
    #[serde(rename = "catalog_descriptor_action_consumers_only")]
    CatalogDescriptorActionConsumersOnly,
}

/// An independently declared occurrence of the practical-result secondary
/// action renderer. Other projection mechanisms deliberately do not belong to
/// this denominator.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultActionConsumerOccurrence {
    pub(super) occurrence_id: PracticalResultActionConsumerOccurrenceId,
    pub(super) overlay_path: String,
    pub(super) overlay_size: usize,
    pub(super) overlay_sha256: String,
    pub(super) runtime_base: String,
    pub(super) consumer_id: PracticalResultConsumerId,
    pub(super) descriptor_id: PracticalResultConsumerDescriptorId,
    pub(super) pointer_table_offset: String,
    pub(super) pointer_table_runtime_address: String,
    pub(super) pointer_table_size: usize,
    pub(super) pointer_table_sha256: String,
    pub(super) aliases: Vec<usize>,
    pub(super) descriptor_offset: String,
    pub(super) descriptor_runtime_address: String,
    pub(super) descriptor_capacity: usize,
    pub(super) descriptor_sha256: String,
    pub(super) semantic_entry_id: Option<PracticalResultSemanticEntryId>,
    pub(super) evidence_status: PracticalResultProjectionEvidenceStatus,
    pub(super) semantic_status: PracticalResultProjectionSemanticStatus,
    pub(super) unresolved_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultVramResidencyCatalog {
    pub(super) kind: PracticalResultVramResidencyCatalogKind,
    pub(super) residencies: Vec<PracticalResultVramResidency>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub(super) enum PracticalResultVramResidencyCatalogKind {
    #[serde(rename = "justice_gakuen2_practical_result_vram_residency_catalog")]
    JusticeGakuen2PracticalResultVramResidencyCatalog,
}

/// Common residency geometry plus exactly one loader-evidence shape.
///
/// The common struct deliberately does not use `deny_unknown_fields` because
/// Serde does not support that attribute on a struct containing a flattened
/// tagged enum. The flattened enum is the fail-closed boundary and rejects
/// every field not owned by the selected evidence variant.
#[derive(Debug, Deserialize)]
pub(super) struct PracticalResultVramResidency {
    pub(super) id: PracticalResultVramResidencyId,
    pub(super) source_atlas_domain_id: SourceAtlasDomainId,
    pub(super) source_path: String,
    pub(super) tim_offset: String,
    pub(super) bpp: u8,
    pub(super) source_tim_size: usize,
    pub(super) source_tim_sha256: String,
    pub(super) image_vram_word_x: usize,
    pub(super) image_vram_y: usize,
    pub(super) pixel_width: usize,
    pub(super) pixel_height: usize,
    pub(super) texture_page: u8,
    pub(super) texture_bank: u8,
    #[serde(flatten)]
    pub(super) evidence: PracticalResultVramResidencyEvidence,
    pub(super) unresolved_reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "evidence_status",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(super) enum PracticalResultVramResidencyEvidence {
    StaticConfirmedLoadAndUploadIntent {
        producer_overlay_path: String,
        runtime_base: String,
        loader_span_offset: String,
        loader_span_size: usize,
        loader_span_sha256: String,
        record_load_buffer_setup_offset: String,
        indirect_load_call_offset: String,
        catalog_index_setup_offset: String,
        catalog_index: String,
        main_catalog_source_path: String,
        main_catalog_source_sha256: String,
        main_catalog_entry_offset: String,
        main_catalog_entry_size: usize,
        main_catalog_entry_sha256: String,
        record_load_buffer_ram_address: String,
        unconditional_tim_pointer_setup_offset: String,
        unconditional_tim_postprocess_call_offset: String,
    },
    StaticConfirmedIndexedMemberLoadAndUploadIntent {
        producer_overlay_path: String,
        runtime_base: String,
        loader_span_offset: String,
        loader_span_size: usize,
        loader_span_sha256: String,
        record_load_buffer_setup_offset: String,
        indirect_load_call_offset: String,
        catalog_index_setup_offset: String,
        catalog_index: String,
        selected_member_index: usize,
        selected_when_state_byte_equals_two: bool,
        member_index_setup_offset: String,
        main_catalog_source_path: String,
        main_catalog_source_sha256: String,
        main_catalog_entry_offset: String,
        main_catalog_entry_size: usize,
        main_catalog_entry_sha256: String,
        record_load_buffer_ram_address: String,
        first_tim_pointer_setup_offset: String,
        first_tim_postprocess_call_offset: String,
    },
}

#[cfg(test)]
mod vram_residency_evidence_tests {
    use super::PracticalResultVramResidency;

    const VALID_DIRECT_RESIDENCY: &str = r#"{
        "id": "direct_residency",
        "source_atlas_domain_id": "direct_atlas",
        "source_path": "DAT2/DIRECT.BIZ",
        "tim_offset": "0x00000",
        "bpp": 4,
        "source_tim_size": 64,
        "source_tim_sha256": "tim",
        "image_vram_word_x": 896,
        "image_vram_y": 256,
        "pixel_width": 256,
        "pixel_height": 256,
        "texture_page": 14,
        "texture_bank": 1,
        "producer_overlay_path": "DAT1/DIRECT.BIN",
        "runtime_base": "0x800a2000",
        "loader_span_offset": "0x0100",
        "loader_span_size": 64,
        "loader_span_sha256": "loader",
        "record_load_buffer_setup_offset": "0x0100",
        "indirect_load_call_offset": "0x0110",
        "catalog_index_setup_offset": "0x0120",
        "catalog_index": "0x0001",
        "main_catalog_source_path": "MAIN.EXE",
        "main_catalog_source_sha256": "main",
        "main_catalog_entry_offset": "0x0200",
        "main_catalog_entry_size": 12,
        "main_catalog_entry_sha256": "entry",
        "record_load_buffer_ram_address": "0x800aa000",
        "unconditional_tim_pointer_setup_offset": "0x0130",
        "unconditional_tim_postprocess_call_offset": "0x0140",
        "evidence_status": "static_confirmed_load_and_upload_intent",
        "unresolved_reason": "runtime lifetime remains open"
    }"#;

    #[test]
    fn direct_residency_has_one_unambiguous_evidence_shape() {
        serde_json::from_str::<PracticalResultVramResidency>(VALID_DIRECT_RESIDENCY)
            .expect("valid direct residency evidence");
    }

    #[test]
    fn direct_residency_rejects_unowned_fields() {
        let invalid = VALID_DIRECT_RESIDENCY.replace(
            "\"unresolved_reason\":",
            "\"selected_member_index\": 0, \"unresolved_reason\":",
        );
        assert!(serde_json::from_str::<PracticalResultVramResidency>(&invalid).is_err());
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultClutBindingCatalog {
    pub(super) kind: PracticalResultClutBindingCatalogKind,
    pub(super) bindings: Vec<PracticalResultClutBinding>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub(super) enum PracticalResultClutBindingCatalogKind {
    #[serde(rename = "justice_gakuen2_practical_result_clut_binding_catalog")]
    JusticeGakuen2PracticalResultClutBindingCatalog,
}

#[derive(Debug, Deserialize)]
pub(super) struct PracticalResultClutBinding {
    pub(super) id: PracticalResultClutBindingId,
    pub(super) clut_vram_x: usize,
    pub(super) clut_vram_y: usize,
    #[serde(flatten)]
    pub(super) evidence: PracticalResultClutBindingEvidence,
}

impl PracticalResultClutBinding {
    pub(super) fn evidence_status(&self) -> PracticalResultClutBindingEvidenceStatus {
        match self.evidence {
            PracticalResultClutBindingEvidence::StaticConfirmed { .. } => {
                PracticalResultClutBindingEvidenceStatus::StaticConfirmed
            }
            PracticalResultClutBindingEvidence::StaticConfirmedExternalProducerFamily {
                ..
            } => PracticalResultClutBindingEvidenceStatus::StaticConfirmedExternalProducerFamily,
            PracticalResultClutBindingEvidence::Unresolved { .. } => {
                PracticalResultClutBindingEvidenceStatus::Unresolved
            }
        }
    }

    pub(super) fn source_atlas_domain_id(&self) -> Option<&SourceAtlasDomainId> {
        match &self.evidence {
            PracticalResultClutBindingEvidence::StaticConfirmed {
                source_atlas_domain_id,
                ..
            } => Some(source_atlas_domain_id),
            PracticalResultClutBindingEvidence::StaticConfirmedExternalProducerFamily {
                ..
            }
            | PracticalResultClutBindingEvidence::Unresolved { .. } => None,
        }
    }

    pub(super) fn unresolved_reason(&self) -> Option<&str> {
        match &self.evidence {
            PracticalResultClutBindingEvidence::Unresolved { unresolved_reason } => {
                Some(unresolved_reason)
            }
            PracticalResultClutBindingEvidence::StaticConfirmed { .. }
            | PracticalResultClutBindingEvidence::StaticConfirmedExternalProducerFamily {
                ..
            } => None,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(
    tag = "evidence_status",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub(super) enum PracticalResultClutBindingEvidence {
    StaticConfirmed {
        source_atlas_domain_id: SourceAtlasDomainId,
        source_path: String,
        tim_offset: String,
        bpp: u8,
        palette_index: usize,
        palette_sha256: String,
    },
    StaticConfirmedExternalProducerFamily {
        source_path: String,
        member_count: usize,
        member_decoded_size: usize,
        member_palette_sha256s: Vec<String>,
        unique_palette_count: usize,
        palette_family_sha256: String,
    },
    Unresolved {
        unresolved_reason: String,
    },
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum PracticalResultClutBindingEvidenceStatus {
    StaticConfirmed,
    StaticConfirmedExternalProducerFamily,
    Unresolved,
}

#[cfg(test)]
mod clut_binding_tests {
    use super::{PracticalResultClutBinding, PracticalResultClutBindingEvidence};

    #[test]
    fn external_palette_producer_rejects_internal_atlas_fields() {
        let json = r#"{
            "id": "external",
            "clut_vram_x": 0,
            "clut_vram_y": 503,
            "evidence_status": "static_confirmed_external_producer_family",
            "source_path": "DAT2/TESTMJ.TIZ",
            "member_count": 1,
            "member_decoded_size": 64,
            "member_palette_sha256s": ["member"],
            "unique_palette_count": 1,
            "palette_family_sha256": "family",
            "source_atlas_domain_id": "impossible-mixed-owner"
        }"#;

        assert!(serde_json::from_str::<PracticalResultClutBinding>(json).is_err());
    }

    #[test]
    fn external_palette_producer_has_one_unambiguous_evidence_shape() {
        let json = r#"{
            "id": "external",
            "clut_vram_x": 0,
            "clut_vram_y": 503,
            "evidence_status": "static_confirmed_external_producer_family",
            "source_path": "DAT2/TESTMJ.TIZ",
            "member_count": 1,
            "member_decoded_size": 64,
            "member_palette_sha256s": ["member"],
            "unique_palette_count": 1,
            "palette_family_sha256": "family"
        }"#;

        let binding = serde_json::from_str::<PracticalResultClutBinding>(json).unwrap();
        assert!(matches!(
            binding.evidence,
            PracticalResultClutBindingEvidence::StaticConfirmedExternalProducerFamily { .. }
        ));
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultConsumerProjectionCatalog {
    pub(super) kind: PracticalResultConsumerProjectionCatalogKind,
    pub(super) descriptors: Vec<PracticalResultConsumerMechanism>,
    pub(super) projections: Vec<PracticalResultConsumerMechanism>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub(super) enum PracticalResultConsumerProjectionCatalogKind {
    #[serde(rename = "justice_gakuen2_practical_result_consumer_projection_catalog")]
    JusticeGakuen2PracticalResultConsumerProjectionCatalog,
}

/// All twelve values currently admitted by the JSON `consumer_kind` field.
///
/// The two flattened common structs deliberately do not carry their own
/// `deny_unknown_fields`: when multiple structs are flattened, each sees fields
/// belonging to the other. The enclosing tagged enum is the fail-closed boundary
/// and rejects any key left unconsumed by the variant and its flattened structs.
#[derive(Debug, Deserialize)]
#[serde(tag = "consumer_kind", deny_unknown_fields)]
pub(super) enum PracticalResultConsumerMechanism {
    #[serde(rename = "catalog_descriptor_renderer")]
    CatalogDescriptorRenderer {
        descriptor_id: PracticalResultConsumerDescriptorId,
        source_atlas_domain_ids: Vec<SourceAtlasDomainId>,
        consumer_id: PracticalResultConsumerId,
        overlay_path: String,
        overlay_sha256: String,
        runtime_base: String,
        renderer_offset: String,
        renderer_runtime_address: String,
        renderer_size: usize,
        renderer_sha256: String,
        pointer_table_offset: String,
        pointer_table_runtime_address: String,
        pointer_table_size: usize,
        pointer_table_sha256: String,
        aliases: Vec<usize>,
        descriptor_offset: String,
        descriptor_runtime_address: String,
        descriptor_capacity: usize,
        descriptor_sha256: String,
        fragments: Vec<PracticalResultCatalogDescriptorFragment>,
        evidence_status: PracticalResultProjectionEvidenceStatus,
    },
    #[serde(rename = "catalog_descriptor_projection")]
    CatalogDescriptorProjection {
        occurrence_id: PracticalResultActionConsumerOccurrenceId,
        #[serde(flatten)]
        source: PracticalResultProjectionSource,
        #[serde(flatten)]
        binding: PracticalResultProjectionBinding,
        descriptor_evidence: PracticalResultCatalogDescriptorEvidence,
        semantic_entry_id: Option<PracticalResultSemanticEntryId>,
        ordered_source_reference_ids: Option<Vec<SourceReferenceId>>,
        physical_region_ids: Vec<PhysicalRegionId>,
        alternative_source_bindings: Vec<PracticalResultAlternativeSourceBinding>,
    },
    #[serde(rename = "catalog_digit_descriptor_projection")]
    CatalogDigitDescriptorProjection {
        #[serde(flatten)]
        source: PracticalResultProjectionSource,
        #[serde(flatten)]
        binding: PracticalResultProjectionBinding,
        renderer_evidence: PracticalResultCatalogRendererEvidence,
        descriptor_table_evidence: PracticalResultDigitDescriptorTableEvidence,
        protected_region_ids: Vec<PracticalResultProtectedRegionId>,
        alternative_source_bindings: Vec<PracticalResultAlternativeSourceBinding>,
    },
    #[serde(rename = "direct_numeric_sprite_sites_projection")]
    DirectNumericSpriteSitesProjection {
        #[serde(flatten)]
        source: PracticalResultProjectionSource,
        #[serde(flatten)]
        binding: PracticalResultProjectionBinding,
        sites: Vec<PracticalResultDirectNumericSpriteSite>,
        physical_region_ids: Vec<PhysicalRegionId>,
        protected_region_ids: Vec<PracticalResultProtectedRegionId>,
        alternative_source_bindings: Vec<PracticalResultAlternativeSourceBinding>,
    },
    #[serde(rename = "direct_sprite_selector_projection")]
    DirectSpriteSelectorProjection {
        #[serde(flatten)]
        source: PracticalResultProjectionSource,
        #[serde(flatten)]
        binding: PracticalResultProjectionBinding,
        renderer_evidence: PracticalResultDirectSpriteRendererEvidence,
        selector_evidence: PracticalResultDirectSpriteSelectorEvidence,
        protected_region_ids: Vec<PracticalResultProtectedRegionId>,
        alternative_source_bindings: Vec<PracticalResultAlternativeSourceBinding>,
    },
    #[serde(rename = "g_catalog_descriptor_selector_projection")]
    GCatalogDescriptorSelectorProjection {
        #[serde(flatten)]
        source: PracticalResultProjectionSource,
        #[serde(flatten)]
        binding: PracticalResultProjectionBinding,
        renderer_evidence: PracticalResultCatalogRendererEvidence,
        selector_evidence: PracticalResultGDescriptorSelectorEvidence,
        protected_region_ids: Vec<PracticalResultProtectedRegionId>,
        alternative_source_bindings: Vec<PracticalResultAlternativeSourceBinding>,
    },
    #[serde(rename = "g_dynamic_config_tile_stream_projection")]
    GDynamicConfigTileStreamProjection {
        #[serde(flatten)]
        source: PracticalResultProjectionSource,
        #[serde(flatten)]
        binding: PracticalResultProjectionBinding,
        renderer_evidence: PracticalResultGDynamicRendererEvidence,
        config_graph_evidence: PracticalResultDynamicConfigGraphEvidence,
    },
    #[serde(rename = "go_catalog_descriptor_selector_projection")]
    GoCatalogDescriptorSelectorProjection {
        #[serde(flatten)]
        source: PracticalResultProjectionSource,
        #[serde(flatten)]
        binding: PracticalResultProjectionBinding,
        renderer_evidence: PracticalResultCatalogRendererEvidence,
        selector_evidence: PracticalResultGoDescriptorSelectorEvidence,
        protected_region_ids: Vec<PracticalResultProtectedRegionId>,
    },
    #[serde(rename = "go_dynamic_config_tile_stream_projection")]
    GoDynamicConfigTileStreamProjection {
        #[serde(flatten)]
        source: PracticalResultProjectionSource,
        #[serde(flatten)]
        binding: PracticalResultProjectionBinding,
        renderer_evidence: PracticalResultGoDynamicRendererEvidence,
        config_graph_evidence: PracticalResultDynamicConfigGraphEvidence,
    },
    #[serde(rename = "numeric_lookup_renderer_projection")]
    NumericLookupRendererProjection {
        #[serde(flatten)]
        source: PracticalResultProjectionSource,
        #[serde(flatten)]
        binding: PracticalResultProjectionBinding,
        renderer_evidence: PracticalResultDirectSpriteRendererEvidence,
        lookup_table_evidence: PracticalResultNumericLookupTableEvidence,
        protected_region_ids: Vec<PracticalResultProtectedRegionId>,
    },
    #[serde(rename = "static_matrix_tile_stream_projection")]
    StaticMatrixTileStreamProjection {
        #[serde(flatten)]
        source: PracticalResultProjectionSource,
        #[serde(flatten)]
        binding: PracticalResultProjectionBinding,
        renderer_evidence: PracticalResultStaticTileRendererEvidence,
        stream_table_evidence: PracticalResultMatrixTileStreamEvidence,
    },
    #[serde(rename = "static_selector_tile_stream_projection")]
    StaticSelectorTileStreamProjection {
        #[serde(flatten)]
        source: PracticalResultProjectionSource,
        #[serde(flatten)]
        binding: PracticalResultProjectionBinding,
        renderer_evidence: PracticalResultStaticTileRendererEvidence,
        stream_table_evidence: PracticalResultSelectorTileStreamEvidence,
    },
}

#[derive(Debug, Deserialize)]
pub(super) struct PracticalResultProjectionSource {
    pub(super) id: PracticalResultConsumerProjectionId,
    pub(super) consumer_id: PracticalResultConsumerId,
    pub(super) overlay_path: String,
    pub(super) overlay_size: usize,
    pub(super) overlay_sha256: String,
    pub(super) runtime_base: String,
}

#[derive(Debug, Deserialize)]
pub(super) struct PracticalResultProjectionBinding {
    pub(super) source_atlas_domain_id: SourceAtlasDomainId,
    pub(super) residency_id: PracticalResultVramResidencyId,
    pub(super) clut_binding_id: PracticalResultClutBindingId,
    pub(super) evidence_status: PracticalResultProjectionEvidenceStatus,
    pub(super) semantic_status: PracticalResultProjectionSemanticStatus,
    pub(super) unresolved_reason: Option<String>,
    #[serde(default)]
    pub(super) source_read_reachability_evidence:
        Option<PracticalResultSourceReadReachabilityEvidence>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultAlternativeSourceBinding {
    pub(super) source_projection_id: PracticalResultConsumerProjectionId,
    pub(super) source_atlas_domain_id: SourceAtlasDomainId,
    pub(super) residency_id: PracticalResultVramResidencyId,
    pub(super) clut_binding_id: PracticalResultClutBindingId,
    pub(super) activation_evidence: PracticalResultAlternativeSourceActivationEvidence,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum PracticalResultAlternativeSourceActivationEvidence {
    Siken20IndexedResultPostUploadDescriptorAlias {
        descriptor_alias: usize,
    },
    Siken20IndexedResultPostUploadDelegatedCallback {
        callback_index: usize,
        route_state_byte_equals_zero: bool,
    },
}

#[cfg(test)]
mod alternative_source_binding_tests {
    use super::{
        PracticalResultAlternativeSourceActivationEvidence, PracticalResultAlternativeSourceBinding,
    };

    #[test]
    fn indexed_result_activation_has_one_fail_closed_evidence_shape() {
        let json = r#"{
            "source_projection_id": "member00_alias46_source_projection",
            "source_atlas_domain_id": "siken20_member00_tim0_indexed_4bpp",
            "residency_id": "member00_residency",
            "clut_binding_id": "member00_palette1",
            "activation_evidence": {
                "kind": "siken20_indexed_result_post_upload_descriptor_alias",
                "descriptor_alias": 46
            }
        }"#;

        let binding = serde_json::from_str::<PracticalResultAlternativeSourceBinding>(json)
            .expect("valid indexed-result activation evidence");
        assert!(matches!(
            binding.activation_evidence,
            PracticalResultAlternativeSourceActivationEvidence::Siken20IndexedResultPostUploadDescriptorAlias {
                descriptor_alias: 46
            }
        ));
    }

    #[test]
    fn indexed_result_activation_rejects_unowned_fields() {
        let json = r#"{
            "source_projection_id": "member00_alias46_source_projection",
            "source_atlas_domain_id": "siken20_member00_tim0_indexed_4bpp",
            "residency_id": "member00_residency",
            "clut_binding_id": "member00_palette1",
            "activation_evidence": {
                "kind": "siken20_indexed_result_post_upload_descriptor_alias",
                "descriptor_alias": 46,
                "assumed_runtime_path": true
            }
        }"#;

        assert!(serde_json::from_str::<PracticalResultAlternativeSourceBinding>(json).is_err());
    }

    #[test]
    fn delegated_result_callback_has_one_fail_closed_evidence_shape() {
        let json = r#"{
            "source_projection_id": "member00_sikeng21_rating_source_projection",
            "source_atlas_domain_id": "siken20_member00_tim0_indexed_4bpp",
            "residency_id": "member00_residency",
            "clut_binding_id": "member00_palette7",
            "activation_evidence": {
                "kind": "siken20_indexed_result_post_upload_delegated_callback",
                "callback_index": 1,
                "route_state_byte_equals_zero": false
            }
        }"#;

        let binding = serde_json::from_str::<PracticalResultAlternativeSourceBinding>(json)
            .expect("valid delegated result-callback evidence");
        assert!(matches!(
            binding.activation_evidence,
            PracticalResultAlternativeSourceActivationEvidence::Siken20IndexedResultPostUploadDelegatedCallback {
                callback_index: 1,
                route_state_byte_equals_zero: false,
            }
        ));
    }

    #[test]
    fn delegated_result_callback_rejects_unowned_fields() {
        let json = r#"{
            "source_projection_id": "member00_sikeng21_rating_source_projection",
            "source_atlas_domain_id": "siken20_member00_tim0_indexed_4bpp",
            "residency_id": "member00_residency",
            "clut_binding_id": "member00_palette7",
            "activation_evidence": {
                "kind": "siken20_indexed_result_post_upload_delegated_callback",
                "callback_index": 1,
                "route_state_byte_equals_zero": false,
                "assumed_runtime_path": true
            }
        }"#;

        assert!(serde_json::from_str::<PracticalResultAlternativeSourceBinding>(json).is_err());
    }
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum PracticalResultSourceReadReachabilityEvidence {
    AdoptedDeclaredEntrypointClosure {
        consumer_runtime_addresses: Vec<String>,
    },
    LoadedImageHeaderCallback {
        callback_pointer_offset: String,
        callback_pointer_runtime_address: String,
        callback_pointer_value: String,
        callback_pointer_sha256: String,
    },
    DormantOutsideCompleteLoadedImageHeaderInterface {
        header_offset: String,
        header_runtime_address: String,
        header_size: usize,
        header_sha256: String,
        code_callback_pointer_offsets: Vec<String>,
        data_pointer_offsets: Vec<String>,
    },
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
pub(super) enum PracticalResultProjectionEvidenceStatus {
    #[serde(rename = "static_confirmed")]
    Confirmed,
    #[serde(rename = "static_confirmed_and_runtime_observed")]
    ConfirmedAndRuntimeObserved,
    #[serde(rename = "static_confirmed_consumer_path")]
    ConsumerPathConfirmed,
    #[serde(rename = "static_confirmed_descriptor_only")]
    DescriptorOnly,
    #[serde(rename = "static_confirmed_dormant_outside_complete_header_interface")]
    DormantOutsideCompleteHeaderInterfaceConfirmed,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum PracticalResultProjectionSemanticStatus {
    PartiallyResolved,
    RuntimeObserved,
    StaticConfirmed,
    Unresolved,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultCatalogDescriptorFragment {
    pub(super) source_u: usize,
    pub(super) source_v: usize,
    pub(super) width: usize,
    pub(super) height: usize,
    pub(super) texture_page: u8,
    pub(super) texture_bank: u8,
    pub(super) clut_x_index: usize,
    pub(super) clut_y_offset: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultCatalogDescriptorEvidence {
    pub(super) descriptor_id: PracticalResultConsumerDescriptorId,
    pub(super) pointer_table_offset: String,
    pub(super) selector_indices: Vec<usize>,
    pub(super) descriptor_offset: String,
    pub(super) descriptor_runtime_address: String,
    pub(super) descriptor_capacity: usize,
    pub(super) descriptor_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultCatalogRendererEvidence {
    pub(super) renderer_offset: String,
    pub(super) renderer_runtime_address: String,
    pub(super) renderer_size: usize,
    pub(super) renderer_sha256: String,
    pub(super) pointer_table_offset: String,
    pub(super) pointer_table_runtime_address: String,
    pub(super) pointer_table_size: usize,
    pub(super) pointer_table_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultDigitDescriptorTableEvidence {
    pub(super) descriptor_bytes_in_raw_value_order_size: usize,
    pub(super) descriptor_bytes_in_raw_value_order_sha256: String,
    pub(super) raw_value_entries: Vec<PracticalResultDigitDescriptorEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultDigitDescriptorEntry {
    pub(super) raw_value: usize,
    pub(super) descriptor_index: usize,
    pub(super) descriptor_offset: String,
    pub(super) descriptor_runtime_address: String,
    pub(super) source_rect: PracticalResultSourceRect,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub(super) enum PracticalResultDirectNumericSpriteSite {
    Geometry(PracticalResultDirectGeometrySpriteSite),
    Lookup(PracticalResultDirectLookupSpriteSite),
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultDirectGeometrySpriteSite {
    pub(super) texture_page_setup_offset: String,
    pub(super) texture_page_setup_runtime_address: String,
    pub(super) get_tpage_call_offset: String,
    pub(super) get_tpage_call_runtime_address: String,
    pub(super) get_clut_call_offset: String,
    pub(super) get_clut_call_runtime_address: String,
    pub(super) geometry_table_offset: String,
    pub(super) geometry_table_runtime_address: String,
    pub(super) geometry_table_size: usize,
    pub(super) geometry_table_sha256: String,
    pub(super) source_rects: Vec<PracticalResultSourceRect>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultDirectLookupSpriteSite {
    pub(super) texture_page_setup_offset: String,
    pub(super) texture_page_setup_runtime_address: String,
    pub(super) get_tpage_call_offset: String,
    pub(super) get_tpage_call_runtime_address: String,
    pub(super) get_clut_call_offset: String,
    pub(super) get_clut_call_runtime_address: String,
    pub(super) lookup_table_offset: String,
    pub(super) lookup_table_runtime_address: String,
    pub(super) lookup_table_size: usize,
    pub(super) lookup_table_sha256: String,
    pub(super) source_geometry: PracticalResultDigitSourceGeometry,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultDirectSpriteRendererEvidence {
    pub(super) function_offset: String,
    pub(super) function_runtime_address: String,
    pub(super) function_size: usize,
    pub(super) function_sha256: String,
    pub(super) texture_page_setup_offset: String,
    pub(super) texture_page_setup_runtime_address: String,
    pub(super) get_tpage_call_offset: String,
    pub(super) get_tpage_call_runtime_address: String,
    pub(super) get_clut_call_offset: String,
    pub(super) get_clut_call_runtime_address: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultDirectSpriteSelectorEvidence {
    pub(super) uv_table_offset: String,
    pub(super) uv_table_runtime_address: String,
    pub(super) uv_table_size: usize,
    pub(super) uv_table_sha256: String,
    pub(super) raw_selector_entries: Vec<PracticalResultDirectSpriteSelectorEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultDirectSpriteSelectorEntry {
    pub(super) raw_table_indices: Vec<usize>,
    pub(super) status_code_values: Vec<usize>,
    pub(super) source_rect: PracticalResultSourceRect,
    pub(super) protected_region_id: PracticalResultProtectedRegionId,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultGDescriptorSelectorEvidence {
    pub(super) selector_table_offset: String,
    pub(super) selector_table_runtime_address: String,
    pub(super) selector_table_size: usize,
    pub(super) selector_table_sha256: String,
    pub(super) selector_load_offset: String,
    pub(super) selector_load_runtime_address: String,
    pub(super) descriptor_table_load_offset: String,
    pub(super) descriptor_table_load_runtime_address: String,
    pub(super) renderer_call_offset: String,
    pub(super) renderer_call_runtime_address: String,
    pub(super) descriptor_bytes_in_raw_selector_order_size: usize,
    pub(super) descriptor_bytes_in_raw_selector_order_sha256: String,
    pub(super) raw_selector_entries: Vec<PracticalResultCatalogSelectorEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultGoDescriptorSelectorEvidence {
    pub(super) selector_table_offset: String,
    pub(super) selector_table_runtime_address: String,
    pub(super) selector_table_size: usize,
    pub(super) selector_table_sha256: String,
    pub(super) descriptor_table_load_offset: String,
    pub(super) descriptor_table_load_runtime_address: String,
    pub(super) renderer_call_offset: String,
    pub(super) renderer_call_runtime_address: String,
    pub(super) descriptor_bytes_in_raw_selector_order_size: usize,
    pub(super) descriptor_bytes_in_raw_selector_order_sha256: String,
    pub(super) raw_selector_entries: Vec<PracticalResultCatalogSelectorEntry>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultCatalogSelectorEntry {
    pub(super) raw_selector_value: usize,
    pub(super) descriptor_index: usize,
    pub(super) descriptor_offset: String,
    pub(super) descriptor_runtime_address: String,
    pub(super) source_rect: PracticalResultSourceRect,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultGDynamicRendererEvidence {
    pub(super) function_offset: String,
    pub(super) function_runtime_address: String,
    pub(super) function_size: usize,
    pub(super) function_sha256: String,
    pub(super) texture_page_setup_offset: String,
    pub(super) texture_page_setup_runtime_address: String,
    pub(super) get_tpage_call_offset: String,
    pub(super) get_tpage_call_runtime_address: String,
    pub(super) get_clut_call_offset: String,
    pub(super) get_clut_call_runtime_address: String,
    pub(super) state_row_load_offset: String,
    pub(super) state_row_load_runtime_address: String,
    pub(super) config_load_offset: String,
    pub(super) config_load_runtime_address: String,
    pub(super) state_column_load_offset: String,
    pub(super) state_column_load_runtime_address: String,
    pub(super) selector_calculation_offset_start: String,
    pub(super) selector_calculation_offset_end: String,
    pub(super) mapped_id_load_offset: String,
    pub(super) mapped_id_load_runtime_address: String,
    pub(super) stream_pointer_load_offset_start: String,
    pub(super) stream_pointer_load_offset_end: String,
    pub(super) tile_projection_witness: PracticalResultTileProjectionWitness,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultGoDynamicRendererEvidence {
    pub(super) function_offset: String,
    pub(super) function_runtime_address: String,
    pub(super) function_size: usize,
    pub(super) function_sha256: String,
    pub(super) texture_page_setup_offset: String,
    pub(super) texture_page_setup_runtime_address: String,
    pub(super) get_tpage_call_offset: String,
    pub(super) get_tpage_call_runtime_address: String,
    pub(super) get_clut_call_offset: String,
    pub(super) get_clut_call_runtime_address: String,
    pub(super) state_row_load_offset: String,
    pub(super) state_row_load_runtime_address: String,
    pub(super) config_load_offset: String,
    pub(super) config_load_runtime_address: String,
    pub(super) state_column_load_offset: String,
    pub(super) state_column_load_runtime_address: String,
    pub(super) selector_calculation_offset_start: String,
    pub(super) selector_calculation_offset_end: String,
    pub(super) mapped_id_load_offset: String,
    pub(super) mapped_id_load_runtime_address: String,
    pub(super) stream_pointer_load_offset_start: String,
    pub(super) stream_pointer_load_offset_end: String,
    pub(super) tile_projection_witness: PracticalResultTileProjectionWitness,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultDynamicConfigGraphEvidence {
    pub(super) header_root_table_pointer_offset: String,
    pub(super) header_root_table_pointer_runtime_address: String,
    pub(super) header_root_table_pointer_value: String,
    pub(super) header_root_table_pointer_sha256: String,
    pub(super) root_table_offset: String,
    pub(super) root_table_runtime_address: String,
    pub(super) root_count: usize,
    pub(super) root_table_size: usize,
    pub(super) root_table_sha256: String,
    pub(super) graph_span_offset: String,
    pub(super) graph_span_runtime_address: String,
    pub(super) graph_span_size: usize,
    pub(super) graph_span_sha256: String,
    pub(super) selector_grid: PracticalResultDynamicSelectorGrid,
    pub(super) stream_encoding: PracticalResultDynamicStreamEncoding,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultDynamicSelectorGrid {
    pub(super) row_count: usize,
    pub(super) column_count: usize,
    pub(super) storage_size: usize,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum PracticalResultDynamicStreamEncoding {
    CountedGroupsWithOptionalFfRowSeparator,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultNumericLookupTableEvidence {
    pub(super) lookup_table_offset: String,
    pub(super) lookup_table_runtime_address: String,
    pub(super) lookup_table_size: usize,
    pub(super) lookup_table_sha256: String,
    pub(super) source_geometry: PracticalResultDigitSourceGeometry,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultStaticTileRendererEvidence {
    pub(super) function_offset: String,
    pub(super) function_runtime_address: String,
    pub(super) function_size: usize,
    pub(super) function_sha256: String,
    pub(super) direct_caller_offset: String,
    pub(super) direct_caller_runtime_address: String,
    pub(super) exported_callback_offset: String,
    pub(super) exported_callback_runtime_address: String,
    pub(super) texture_page_setup_offset: String,
    pub(super) texture_page_setup_runtime_address: String,
    pub(super) get_tpage_call_offset: String,
    pub(super) get_tpage_call_runtime_address: String,
    pub(super) get_clut_call_offset: String,
    pub(super) get_clut_call_runtime_address: String,
    pub(super) tile_projection_witness: PracticalResultTileProjectionWitness,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultTileProjectionWitness {
    pub(super) tile_id_load_offsets: Vec<String>,
    pub(super) source_u_projection_offset: String,
    pub(super) source_v_projection_offset: String,
    pub(super) tile_id_increment_offset: String,
    #[serde(default)]
    pub(super) dynamic_header_transfer_offset: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultMatrixTileStreamEvidence {
    pub(super) row_axis_load_offset: String,
    pub(super) row_axis_load_runtime_address: String,
    pub(super) column_axis_load_offset: String,
    pub(super) column_axis_load_runtime_address: String,
    pub(super) selector_calculation_offsets: Vec<String>,
    pub(super) pointer_load_offsets: Vec<String>,
    pub(super) selector_grid: PracticalResultStaticSelectorGrid,
    pub(super) pointer_table_offset: String,
    pub(super) pointer_table_runtime_address: String,
    pub(super) pointer_table_size: usize,
    pub(super) pointer_table_sha256: String,
    pub(super) stream_arena_offset: String,
    pub(super) stream_arena_runtime_address: String,
    pub(super) stream_arena_size: usize,
    pub(super) stream_arena_sha256: String,
    pub(super) stream_encoding: PracticalResultStaticStreamEncoding,
    pub(super) expected_derived_tile_id_min: usize,
    pub(super) expected_derived_tile_id_max: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultStaticSelectorGrid {
    pub(super) row_count: usize,
    pub(super) column_count: usize,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum PracticalResultStaticStreamEncoding {
    TerminatedRunsWithFeRowSeparator,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultSelectorTileStreamEvidence {
    pub(super) selector_axis_load_offset: String,
    pub(super) selector_axis_load_runtime_address: String,
    pub(super) selector_scale_offset: String,
    pub(super) selector_scale_runtime_address: String,
    pub(super) pointer_load_offset: String,
    pub(super) pointer_load_runtime_address: String,
    pub(super) pointer_table_offset: String,
    pub(super) pointer_table_runtime_address: String,
    pub(super) pointer_table_size: usize,
    pub(super) pointer_table_sha256: String,
    pub(super) stream_arena_offset: String,
    pub(super) stream_arena_runtime_address: String,
    pub(super) stream_arena_size: usize,
    pub(super) stream_arena_sha256: String,
    pub(super) streams: Vec<PracticalResultSelectorTileStream>,
    pub(super) stream_encoding: PracticalResultStaticStreamEncoding,
    pub(super) expected_derived_tile_id_min: usize,
    pub(super) expected_derived_tile_id_max: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultSelectorTileStream {
    pub(super) raw_selector_value: usize,
    pub(super) offset: String,
    pub(super) runtime_address: String,
    pub(super) encoded_size: usize,
    pub(super) encoded_sha256: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultSourceRect {
    pub(super) u: usize,
    pub(super) v: usize,
    pub(super) width: usize,
    pub(super) height: usize,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PracticalResultDigitSourceGeometry {
    pub(super) raw_values: Vec<usize>,
    pub(super) u_by_raw_value: Vec<usize>,
    pub(super) v: usize,
    pub(super) width: usize,
    pub(super) height: usize,
}
