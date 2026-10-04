use std::sync::atomic::{AtomicUsize, Ordering};

use crate::pipeline::sha256_bytes;

use super::sha256_file;

static NEXT_DIRECTORY: AtomicUsize = AtomicUsize::new(0);

#[test]
fn file_digest_matches_the_current_file_bytes() {
    let directory = temporary_directory();
    let path = directory.join("input.bin");
    let bytes = b"source-bound bytes";
    std::fs::write(&path, bytes).unwrap();

    assert_eq!(sha256_file(&path).unwrap(), sha256_bytes(bytes));

    std::fs::remove_dir_all(directory).unwrap();
}

#[test]
fn file_digest_is_recomputed_after_the_file_changes() {
    let directory = temporary_directory();
    let path = directory.join("input.bin");
    std::fs::write(&path, b"before").unwrap();
    let before = sha256_file(&path).unwrap();

    std::fs::write(&path, b"after with a different length").unwrap();
    let after = sha256_file(&path).unwrap();

    assert_ne!(before, after);
    assert_eq!(
        after,
        sha256_bytes(b"after with a different length".as_slice())
    );
    std::fs::remove_dir_all(directory).unwrap();
}

#[cfg(unix)]
#[test]
fn file_digest_is_recomputed_after_same_length_content_replaces_the_file() {
    let directory = temporary_directory();
    let path = directory.join("input.bin");
    std::fs::write(&path, b"before").unwrap();
    let original_modified_at = std::fs::metadata(&path).unwrap().modified().unwrap();
    let before = sha256_file(&path).unwrap();

    std::fs::write(&path, b"after!").unwrap();
    std::fs::File::options()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(std::fs::FileTimes::new().set_modified(original_modified_at))
        .unwrap();
    let after = sha256_file(&path).unwrap();

    assert_ne!(before, after);
    assert_eq!(after, sha256_bytes(b"after!"));
    std::fs::remove_dir_all(directory).unwrap();
}

fn temporary_directory() -> std::path::PathBuf {
    let sequence = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
    let directory = std::env::temp_dir().join(format!(
        "justice-gakuen2-file-digest-{}-{sequence}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    directory
}
