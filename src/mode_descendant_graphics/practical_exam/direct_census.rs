use std::collections::BTreeSet;

use anyhow::{Context, Result, ensure};
use psx_r3000a::{Instruction, Register, decode};

use crate::pipeline::sha256_bytes;
use crate::tim::Cell;

use super::{PracticalExamConsumer, PracticalExamTextureRegion};

#[path = "direct_census/draw_packets.rs"]
mod draw_packets;
#[path = "direct_census/eight_bit_domains.rs"]
mod eight_bit_domains;
#[path = "direct_census/packet_escape.rs"]
mod packet_escape;
#[path = "direct_census/packet_submission.rs"]
mod packet_submission;
#[path = "direct_census/profiles.rs"]
mod profiles;
#[path = "direct_census/psx_decode.rs"]
mod psx_decode;
#[path = "direct_census/sprite_regions.rs"]
mod sprite_regions;
#[path = "direct_census/tpage_classification.rs"]
mod tpage_classification;

use draw_packets::validate_draw_packet_denominator;
use eight_bit_domains::validate_eight_bit_coordinate_domains;
use packet_escape::{validate_no_inline_draw_mode_packet, validate_packet_pool_escape_denominator};
use packet_submission::{
    PacketSubmissionAudit, register_originates_in_packet_pool,
    validate_packet_submission_denominator,
};
use profiles::*;
use psx_decode::*;
use sprite_regions::audit_direct_sprite_regions;
use tpage_classification::validate_get_tpage_classifications;

pub(super) struct PracticalExamDirectCensus {
    pub(super) direct_regions: Vec<PracticalExamTextureRegion>,
    pub(super) complete: bool,
}

pub(super) fn audit_practical_exam_direct_census(
    overlay: &[u8],
    consumer: PracticalExamConsumer,
    expected_source_sha256: &str,
) -> Result<PracticalExamDirectCensus> {
    let profile = profile(consumer);
    ensure!(
        sha256_bytes(overlay) == expected_source_sha256,
        "{consumer:?} practical-exam direct-census source changed"
    );
    validate_code_span(overlay, consumer, &profile)?;

    let found_calls = locate_get_tpage_calls(overlay, &profile)?;
    ensure!(
        found_calls == profile.get_tpage_calls,
        "{consumer:?} practical-exam GetTPage denominator changed: expected {}, found {}",
        format_offsets(profile.get_tpage_calls),
        format_offsets(&found_calls)
    );
    validate_get_tpage_classifications(overlay, consumer, &profile)?;
    validate_draw_packet_denominator(overlay, consumer, &profile)?;
    let packet_submission = validate_packet_submission_denominator(overlay, consumer, &profile)?;
    validate_packet_pool_escape_denominator(overlay, consumer, &profile, &packet_submission)?;
    validate_eight_bit_coordinate_domains(overlay, consumer)?;
    validate_no_inline_draw_mode_packet(overlay, consumer, &profile, &packet_submission)?;
    let direct_regions = audit_direct_sprite_regions(overlay, consumer)?;

    Ok(PracticalExamDirectCensus {
        direct_regions,
        complete: true,
    })
}
