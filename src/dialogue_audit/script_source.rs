use anyhow::{Result, ensure};

use crate::cue::CueSheet;
use crate::disc::rebuild::read_record;
use crate::pipeline::sha256_bytes;
use crate::source_disc::SupportedSourceDisc;

pub(super) const MGAME_PATH: &str = "DAT1/MGAME.BIN";
pub(super) const MGAME_SIZE: usize = 0x296a4;
pub(super) const MGAME_SHA256: &str =
    "887c7c1775db5a18aed19661a08d109a5f35c54303259b21c7d6560ef1a3baba";

pub(super) fn load_mgame(cue: &CueSheet) -> Result<Vec<u8>> {
    let (_, mgame) = read_record(&cue.image_path, MGAME_PATH)?;
    validate_mgame(mgame)
}

pub(super) fn load_mgame_from_source(source: &SupportedSourceDisc) -> Result<Vec<u8>> {
    let (_, mgame) = source.read_record(MGAME_PATH)?;
    validate_mgame(mgame)
}

fn validate_mgame(mgame: Vec<u8>) -> Result<Vec<u8>> {
    ensure!(mgame.len() == MGAME_SIZE, "unexpected DAT1/MGAME.BIN size");
    ensure!(
        sha256_bytes(&mgame) == MGAME_SHA256,
        "unexpected DAT1/MGAME.BIN SHA-256"
    );
    Ok(mgame)
}
