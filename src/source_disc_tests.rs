use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::cue::CueSheet;
use crate::pipeline::sha256_bytes;

use super::VerifiedSourceDisc;

static NEXT_TEST_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

#[test]
fn source_path_replacement_does_not_change_the_verified_snapshot() {
    let fixture = SourceDiscFixture::create(b"verified source bytes");
    let source_sha256 = sha256_bytes(&fixture.source_bytes);
    let source = VerifiedSourceDisc::create_in(
        &fixture.cue_path,
        &source_sha256,
        fixture.source_bytes.len() as u64,
        &fixture.snapshot_parent,
    )
    .unwrap();

    let displaced_source = fixture.root.join("displaced-source.bin");
    std::fs::rename(&fixture.source_path, &displaced_source).unwrap();
    std::fs::write(&fixture.source_path, b"replacement source data").unwrap();

    assert_eq!(
        std::fs::read(source.snapshot_image_path()).unwrap(),
        fixture.source_bytes
    );
    assert_eq!(source.source_bin_sha256(), source_sha256);
    let snapshot_cue = CueSheet::parse(source.snapshot_cue_path()).unwrap();
    assert_eq!(snapshot_cue.image_path, source.snapshot_image_path());

    drop(source);
    fixture.remove();
}

#[test]
fn wrong_source_hash_is_rejected_without_leaving_a_snapshot() {
    let fixture = SourceDiscFixture::create(b"source with the wrong expected hash");
    let result = VerifiedSourceDisc::create_in(
        &fixture.cue_path,
        &sha256_bytes(b"different source"),
        fixture.source_bytes.len() as u64,
        &fixture.snapshot_parent,
    );

    assert!(result.is_err());
    assert!(directory_is_empty(&fixture.snapshot_parent));
    fixture.remove();
}

#[test]
fn wrong_source_size_is_rejected_without_creating_a_snapshot() {
    let fixture = SourceDiscFixture::create(b"source with the wrong expected size");
    let result = VerifiedSourceDisc::create_in(
        &fixture.cue_path,
        &sha256_bytes(&fixture.source_bytes),
        fixture.source_bytes.len() as u64 + 1,
        &fixture.snapshot_parent,
    );

    assert!(result.is_err());
    assert!(directory_is_empty(&fixture.snapshot_parent));
    fixture.remove();
}

#[test]
fn dropping_a_verified_source_disc_removes_its_private_files() {
    let fixture = SourceDiscFixture::create(b"temporary verified source");
    let source = VerifiedSourceDisc::create_in(
        &fixture.cue_path,
        &sha256_bytes(&fixture.source_bytes),
        fixture.source_bytes.len() as u64,
        &fixture.snapshot_parent,
    )
    .unwrap();
    let snapshot_bin = source.snapshot_image_path().to_path_buf();
    let snapshot_cue = source.snapshot_cue_path().to_path_buf();

    assert!(snapshot_bin.is_file());
    assert!(snapshot_cue.is_file());
    drop(source);

    assert!(!snapshot_bin.exists());
    assert!(!snapshot_cue.exists());
    assert!(directory_is_empty(&fixture.snapshot_parent));
    fixture.remove();
}

struct SourceDiscFixture {
    root: PathBuf,
    source_path: PathBuf,
    cue_path: PathBuf,
    snapshot_parent: PathBuf,
    source_bytes: Vec<u8>,
}

impl SourceDiscFixture {
    fn create(source_bytes: &[u8]) -> Self {
        let sequence = NEXT_TEST_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "justice-gakuen2-source-disc-test-{}-{sequence}",
            std::process::id()
        ));
        std::fs::create_dir(&root).unwrap();
        let source_path = root.join("original.bin");
        let cue_path = root.join("original.cue");
        let snapshot_parent = root.join("snapshots");
        std::fs::write(&source_path, source_bytes).unwrap();
        std::fs::write(
            &cue_path,
            "FILE \"original.bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n",
        )
        .unwrap();
        std::fs::create_dir(&snapshot_parent).unwrap();
        Self {
            root,
            source_path,
            cue_path,
            snapshot_parent,
            source_bytes: source_bytes.to_vec(),
        }
    }

    fn remove(self) {
        std::fs::remove_dir_all(self.root).unwrap();
    }
}

fn directory_is_empty(path: &Path) -> bool {
    std::fs::read_dir(path).unwrap().next().is_none()
}
