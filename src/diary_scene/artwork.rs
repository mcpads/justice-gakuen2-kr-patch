//! Source-bound standalone Diary artwork archives.
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct SceneArtworkArchiveReport {
    pub path: String,
    pub source_size: usize,
    pub source_sha256: String,
    pub patched_sha256: String,
    pub members: Vec<SceneArtworkMemberReport>,
}
#[derive(Debug, Clone, Serialize)]
pub struct SceneArtworkMemberReport {
    pub index: usize,
    pub source_decoded_sha256: String,
    pub patched_decoded_sha256: String,
    pub source_compressed_size: usize,
    pub rebuilt_compressed_size: usize,
    pub outside_mask_preserved: bool,
    pub compression_requirement_satisfied: bool,
}
pub struct SceneArtworkArchiveBuild {
    pub data: Vec<u8>,
    pub report: SceneArtworkArchiveReport,
}
