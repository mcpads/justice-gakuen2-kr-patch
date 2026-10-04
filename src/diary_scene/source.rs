use anyhow::{Context, Result, ensure};

use crate::compression::decompress;
use crate::pipeline::sha256_bytes;
use crate::source_disc::SupportedSourceDisc;
use crate::tzz::{TzzMember, parse_tzz};

pub const DIARY_SCENE_PATH: &str = "DAT2/MGBG.TZZ";
pub(super) const SOURCE_ARCHIVE_SHA256: &str =
    "0cee31213c28ab78c5da0a48be2bc945b6b21e6e055997968191632cb327c7d4";
pub(super) const SOURCE_MEMBER_COUNT: usize = 112;
pub(super) const SOURCE_RUNTIME_BUNDLE_COUNT: usize = 48;
pub(super) const SOURCE_RUNTIME_MEMBER_COUNT: usize = 660;

pub(super) struct DiarySceneSource {
    pub(super) archive: Vec<u8>,
    pub(super) members: Vec<TzzMember>,
    pub(super) decoded_members: Vec<Vec<u8>>,
}

pub(super) fn load_source_from_disc(source: &SupportedSourceDisc) -> Result<DiarySceneSource> {
    let (_, archive) = source.read_record(DIARY_SCENE_PATH)?;
    ensure!(
        sha256_bytes(&archive) == SOURCE_ARCHIVE_SHA256,
        "MGBG.TZZ source hash changed"
    );
    let members = parse_tzz(&archive)?;
    ensure!(
        members.len() == SOURCE_MEMBER_COUNT,
        "MGBG.TZZ member count changed"
    );
    let decoded_members = members
        .iter()
        .map(|member| {
            decompress(&archive[member.compressed_range()], false)
                .with_context(|| format!("failed to decode MGBG.TZZ member {}", member.index))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(DiarySceneSource {
        archive,
        members,
        decoded_members,
    })
}
