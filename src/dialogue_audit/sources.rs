use std::path::Path;

use anyhow::{Result, ensure};

use crate::disc::{RawTrack, iso9660::Iso9660};
use crate::pipeline::sha256_bytes;

const EXPECTED_RUNTIME_IMAGE_PATH_COUNT: usize = 78;
const EXPECTED_RUNTIME_IMAGE_PATHS_SHA256: &str =
    "65f36ae2f0b3aed0d99bde1aa974f1d29fef0cf67e6bf0f7972f087a532b8968";
const EXPECTED_MGK_PATHS: [&str; 10] = [
    "DAT2/MGK01.BIZ",
    "DAT2/MGK02.BIZ",
    "DAT2/MGK03.BIZ",
    "DAT2/MGK04.BIZ",
    "DAT2/MGK05.BIZ",
    "DAT2/MGK07.BIZ",
    "DAT2/MGK08.BIZ",
    "DAT2/MGK09.BIZ",
    "DAT2/MGK10.BIZ",
    "DAT2/MGK12.BIZ",
];

pub(super) struct DialogueSource {
    pub(super) path: String,
    pub(super) extent_lba: u32,
    pub(super) data: Vec<u8>,
}

pub(super) fn load_dialogue_runtime_image_sources(
    image_path: &Path,
) -> Result<Vec<DialogueSource>> {
    load_dialogue_sources(
        image_path,
        is_dialogue_runtime_image_path,
        expected_runtime_image_population,
        "DAT2/MG dialogue runtime image",
    )
}

fn is_dialogue_runtime_image_path(path: &str) -> bool {
    path.starts_with("DAT2/MG") && path.ends_with(".BIZ") && path != "DAT2/MGSTAFF1.BIZ"
}

pub(super) fn expected_runtime_image_population(paths: &[&str]) -> bool {
    let mut path_manifest = paths.join("\n").into_bytes();
    path_manifest.push(b'\n');
    paths.len() == EXPECTED_RUNTIME_IMAGE_PATH_COUNT
        && sha256_bytes(&path_manifest) == EXPECTED_RUNTIME_IMAGE_PATHS_SHA256
}

pub(super) fn load_mgk_dialogue_sources(image_path: &Path) -> Result<Vec<DialogueSource>> {
    load_dialogue_sources(
        image_path,
        |path| EXPECTED_MGK_PATHS.contains(&path),
        |paths| paths == EXPECTED_MGK_PATHS,
        "DAT2/MGK dialogue build subset",
    )
}

fn load_dialogue_sources(
    image_path: &Path,
    include: impl Fn(&str) -> bool,
    expected_population: impl Fn(&[&str]) -> bool,
    population_name: &str,
) -> Result<Vec<DialogueSource>> {
    let mut track = RawTrack::open(image_path)?;
    let mut iso = Iso9660::open(&mut track)?;
    let files = iso.files()?;
    let mut dialogue_records: Vec<_> = files
        .into_iter()
        .filter(|(path, _)| include(path))
        .collect();
    dialogue_records.sort_by(|left, right| left.0.cmp(&right.0));
    let actual_paths = dialogue_records
        .iter()
        .map(|(path, _)| path.as_str())
        .collect::<Vec<_>>();
    ensure!(
        expected_population(&actual_paths),
        "unexpected {population_name} population: {actual_paths:?}"
    );

    dialogue_records
        .into_iter()
        .map(|(path, record)| {
            Ok(DialogueSource {
                path,
                extent_lba: record.extent_lba,
                data: iso.read_record(&record)?,
            })
        })
        .collect()
}

#[cfg(test)]
#[path = "sources_tests.rs"]
mod tests;
