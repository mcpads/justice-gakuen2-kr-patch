use super::dialogue_disc_records::OwnedDiscRecord;
use crate::disc::rebuild::DiscRecordSourceIdentity;

#[test]
fn product_contribution_preserves_its_producer_declared_source_identity() {
    let producer_source = b"producer-consumed logical record";
    let identity = DiscRecordSourceIdentity::from_bytes(producer_source);
    let record = OwnedDiscRecord::new(
        "fixture compositor",
        "SAME-SIZE-OTHER.BIN",
        vec![0x5a; producer_source.len()],
        identity.clone(),
    );

    let contribution = record.contribution();

    assert_eq!(contribution.source, identity);
    assert_eq!(contribution.path, "SAME-SIZE-OTHER.BIN");
}
