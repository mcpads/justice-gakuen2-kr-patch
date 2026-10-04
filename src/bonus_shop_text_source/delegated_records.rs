use std::collections::{BTreeMap, BTreeSet};

use anyhow::{Context, Result, ensure};

use super::model::{
    BonusShopTextDelegatedRecord, BonusShopTextDelegatedWriter, BonusShopTextSourceUnit,
};

pub(super) const DELEGATED_RECORD_KIND: &str =
    "Justice Gakuen 2 delegated bonus-shop source record";

#[derive(Clone, Copy)]
pub(super) struct DelegatedRecordSpec {
    pub(super) id: &'static str,
    pub(super) file: &'static str,
    pub(super) writer_id: &'static str,
    pub(super) unit_ids: &'static [&'static str],
}

pub(super) const DELEGATED_RECORD_SPECS: [DelegatedRecordSpec; 2] = [
    DelegatedRecordSpec {
        id: "clerk-dialogue-004",
        file: "delegated/clerk-dialogue/clerk-dialogue-004.json",
        writer_id: "bonus_shop_exit_confirmation",
        unit_ids: &["woman_prompt", "woman_yes", "woman_no"],
    },
    DelegatedRecordSpec {
        id: "clerk-dialogue-014",
        file: "delegated/clerk-dialogue/clerk-dialogue-014.json",
        writer_id: "bonus_shop_exit_confirmation",
        unit_ids: &["elder_prompt", "elder_yes", "elder_no"],
    },
];

pub(super) fn delegated_record_spec(id: &str) -> Option<DelegatedRecordSpec> {
    DELEGATED_RECORD_SPECS
        .iter()
        .copied()
        .find(|spec| spec.id == id)
}

pub(super) fn delegated_record(
    source: BonusShopTextSourceUnit,
    spec: DelegatedRecordSpec,
) -> BonusShopTextDelegatedRecord {
    BonusShopTextDelegatedRecord {
        kind: DELEGATED_RECORD_KIND.to_string(),
        id: spec.id.to_string(),
        source,
        writer: BonusShopTextDelegatedWriter {
            id: spec.writer_id.to_string(),
            unit_ids: spec.unit_ids.iter().map(|id| (*id).to_string()).collect(),
        },
    }
}

pub(super) fn validate_delegated_record(record: &BonusShopTextDelegatedRecord) -> Result<()> {
    let spec = delegated_record_spec(&record.id).ok_or_else(|| {
        anyhow::anyhow!(
            "bonus-shop delegated record {} has no declared writer",
            record.id
        )
    })?;
    ensure!(
        record.kind == DELEGATED_RECORD_KIND
            && record.source.unit_id == record.id
            && record.writer.id == spec.writer_id
            && record.writer.unit_ids
                == spec
                    .unit_ids
                    .iter()
                    .map(|id| (*id).to_string())
                    .collect::<Vec<_>>(),
        "bonus-shop delegated record {} changed its writer binding",
        record.id
    );
    Ok(())
}

pub(super) fn validate_exit_confirmation_delegations(
    records: &BTreeMap<String, BonusShopTextDelegatedRecord>,
    build: &crate::bonus_shop_exit_confirmation::BonusShopExitConfirmationBuild,
) -> Result<()> {
    ensure!(
        records.len() == DELEGATED_RECORD_SPECS.len(),
        "bonus-shop exit-confirmation delegation population changed"
    );
    let build_units = build
        .report
        .units
        .iter()
        .map(|unit| unit.id.as_str())
        .collect::<BTreeSet<_>>();
    for record in records.values() {
        validate_delegated_record(record)?;
        let source_record = build
            .report
            .records
            .iter()
            .find(|candidate| candidate.record_offset == record.source.source_offset)
            .with_context(|| {
                format!(
                    "delegated bonus-shop record {} is absent from the exit-confirmation writer",
                    record.id
                )
            })?;
        ensure!(
            source_record.pointer_storage_offset == record.source.pointer_storage_offset
                && source_record.source_record_sha256 == record.source.source_record_sha256
                && record
                    .writer
                    .unit_ids
                    .iter()
                    .all(|id| build_units.contains(id.as_str())),
            "delegated bonus-shop record {} does not match the exit-confirmation writer",
            record.id
        );
    }
    Ok(())
}
