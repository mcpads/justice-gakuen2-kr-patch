use super::selector_consumers_model::{
    DialogueSelectorConsumerAudit, DialogueSelectorConsumerReport,
    DialogueSelectorEntryConsumerAudit,
};
use super::selector_translation_audit::audit_test_readiness;
use super::selector_translation_model::DialogueSelectorConsumerEvidence;
use super::selector_translation_model::DialogueSelectorTranslationScope;
use super::selector_translation_source::classify_test_evidence;

#[test]
fn selector_translation_evidence_prefers_exact_runtime_and_entry_consumers() {
    let report = report();

    assert_eq!(
        classify_test_evidence(&report, 2, 5),
        DialogueSelectorConsumerEvidence::RuntimeInsertionRewrite
    );
    assert_eq!(
        classify_test_evidence(&report, 5, 30),
        DialogueSelectorConsumerEvidence::DirectEntryLoad
    );
    assert_eq!(
        classify_test_evidence(&report, 5, 29),
        DialogueSelectorConsumerEvidence::DirectSelectorLoad
    );
    assert_eq!(
        classify_test_evidence(&report, 4, 0),
        DialogueSelectorConsumerEvidence::ConsumerPending
    );
}

#[test]
fn selector_development_and_release_readiness_are_independent() {
    assert_eq!(audit_test_readiness(496, 492, 0, 0), (false, false));
    assert_eq!(audit_test_readiness(496, 0, 496, 490), (true, false));
    assert_eq!(audit_test_readiness(496, 0, 496, 496), (true, true));
}

#[test]
fn selector_scope_keeps_legacy_mgk_and_all_runtime_images_distinct() {
    assert_eq!(
        DialogueSelectorTranslationScope::from_target_scope("selector_banks_2_through_6"),
        Some(DialogueSelectorTranslationScope::MgkDevelopment)
    );
    assert_eq!(
        DialogueSelectorTranslationScope::from_target_scope(
            "all_runtime_image_selector_banks_2_through_6"
        ),
        Some(DialogueSelectorTranslationScope::AllRuntimeImages)
    );
    assert_ne!(
        DialogueSelectorTranslationScope::MgkDevelopment.target_scope(),
        DialogueSelectorTranslationScope::AllRuntimeImages.target_scope()
    );
}

fn report() -> DialogueSelectorConsumerReport {
    DialogueSelectorConsumerReport {
        typed_isa_profile: "psx-r3000a".to_string(),
        selector_table_runtime_start: "0x80101000".to_string(),
        selector_count: 8,
        direct_load_count: 1,
        direct_entry_load_count: 1,
        consumers: vec![DialogueSelectorConsumerAudit {
            selector_index: 5,
            pointer_storage_runtime_address: "0x80101014".to_string(),
            load_instruction_runtime_address: "0x800c1000".to_string(),
            entry_pointer_loads: vec![DialogueSelectorEntryConsumerAudit {
                entry_index: 30,
                load_instruction_runtime_address: "0x800c1048".to_string(),
            }],
        }],
    }
}
