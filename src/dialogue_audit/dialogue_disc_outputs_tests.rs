use std::path::PathBuf;

use super::dialogue_disc_outputs::DialogueDiscOutputs;
use crate::test_support::temporary_directory;

#[test]
fn forced_preparation_preserves_last_verified_outputs() {
    let output_dir = test_output_dir("prepare-preserves-final");
    std::fs::create_dir_all(&output_dir).unwrap();
    let outputs = DialogueDiscOutputs::prepare(&output_dir, true).unwrap();
    write_final_outputs(&outputs, b"last verified");
    std::fs::write(outputs.staged_bin(), b"interrupted build").unwrap();

    assert!(DialogueDiscOutputs::prepare(&output_dir, true).is_err());
    assert_eq!(
        std::fs::read(outputs.staged_bin()).unwrap(),
        b"interrupted build"
    );
    drop(outputs);
    let prepared = DialogueDiscOutputs::prepare(&output_dir, true).unwrap();

    assert_eq!(std::fs::read(prepared.bin()).unwrap(), b"last verified");
    assert_eq!(std::fs::read(prepared.cue()).unwrap(), b"last verified");
    assert_eq!(
        std::fs::read(prepared.manifest()).unwrap(),
        b"last verified"
    );
    assert!(!prepared.staged_bin().exists());
    std::fs::remove_dir_all(output_dir).unwrap();
}

#[test]
fn complete_staged_set_replaces_last_verified_outputs() {
    let output_dir = test_output_dir("publish-complete-set");
    let outputs = DialogueDiscOutputs::prepare(&output_dir, true).unwrap();
    write_final_outputs(&outputs, b"old");
    write_staged_outputs(&outputs, b"new");

    outputs.publish().unwrap();

    assert_eq!(std::fs::read(outputs.bin()).unwrap(), b"new");
    assert_eq!(std::fs::read(outputs.cue()).unwrap(), b"new");
    assert_eq!(std::fs::read(outputs.manifest()).unwrap(), b"new");
    std::fs::remove_dir_all(output_dir).unwrap();
}

#[test]
fn incomplete_staged_set_leaves_last_verified_outputs_untouched() {
    let output_dir = test_output_dir("reject-incomplete-set");
    let outputs = DialogueDiscOutputs::prepare(&output_dir, true).unwrap();
    write_final_outputs(&outputs, b"last verified");
    std::fs::write(outputs.staged_bin(), b"incomplete").unwrap();

    assert!(outputs.publish().is_err());
    assert_eq!(std::fs::read(outputs.bin()).unwrap(), b"last verified");
    assert_eq!(std::fs::read(outputs.cue()).unwrap(), b"last verified");
    assert_eq!(std::fs::read(outputs.manifest()).unwrap(), b"last verified");
    std::fs::remove_dir_all(output_dir).unwrap();
}

#[test]
fn every_failed_member_replacement_restores_the_previous_set() {
    for fail_at in 0..3 {
        let root = test_output_dir(&format!("publication-failure-{fail_at}"));
        let outputs = DialogueDiscOutputs::prepare(&root, true).unwrap();
        write_final_outputs(&outputs, b"old");
        write_staged_outputs(&outputs, b"new");
        let mut index = 0;
        let result = outputs.publish_with(|source, target| {
            if index == fail_at {
                return Err(std::io::Error::other("injected I/O failure"));
            }
            index += 1;
            std::fs::rename(source, target)
        });
        assert!(result.is_err());
        for path in [outputs.bin(), outputs.cue(), outputs.manifest()] {
            assert_eq!(std::fs::read(path).unwrap(), b"old");
        }
        assert!(!root.join(".disc-publication").exists());
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn interrupted_publication_recovers_on_next_prepare_with_or_without_previous_outputs() {
    for had_previous in [false, true] {
        for interrupt_after in 1..=3 {
            let root = test_output_dir(&format!(
                "publication-interrupt-{had_previous}-{interrupt_after}"
            ));
            let outputs = DialogueDiscOutputs::prepare(&root, true).unwrap();
            if had_previous {
                write_final_outputs(&outputs, b"old");
            }
            write_staged_outputs(&outputs, b"new");
            let mut index = 0;
            let interrupted = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                outputs.publish_with(|source, target| {
                    std::fs::rename(source, target)?;
                    index += 1;
                    assert_ne!(index, interrupt_after, "injected process interruption");
                    Ok(())
                })
            }));
            assert!(interrupted.is_err());
            drop(outputs);
            let recovered = DialogueDiscOutputs::prepare(&root, true).unwrap();
            for path in [recovered.bin(), recovered.cue(), recovered.manifest()] {
                if had_previous {
                    assert_eq!(std::fs::read(path).unwrap(), b"old");
                } else {
                    assert!(!path.exists());
                }
            }
            assert!(!root.join(".disc-publication").exists());
            write_staged_outputs(&recovered, b"retry");
            recovered.publish().unwrap();
            assert_eq!(std::fs::read(recovered.bin()).unwrap(), b"retry");
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}

fn write_final_outputs(outputs: &DialogueDiscOutputs, bytes: &[u8]) {
    std::fs::write(outputs.bin(), bytes).unwrap();
    std::fs::write(outputs.cue(), bytes).unwrap();
    std::fs::write(outputs.manifest(), bytes).unwrap();
}

fn write_staged_outputs(outputs: &DialogueDiscOutputs, bytes: &[u8]) {
    std::fs::write(outputs.staged_bin(), bytes).unwrap();
    std::fs::write(outputs.staged_cue(), bytes).unwrap();
    std::fs::write(outputs.staged_manifest(), bytes).unwrap();
}

fn test_output_dir(label: &str) -> PathBuf {
    temporary_directory(&format!("dialogue-disc-{label}"))
}
