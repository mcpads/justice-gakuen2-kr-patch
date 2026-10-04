use std::path::PathBuf;

use super::dialogue_font_conflict_writer::write_group_indexes;
use super::dialogue_font_conflicts_model::DialogueFontConflictShardRef;
use super::translation_workspace_validation::MAX_SHARD_BYTES;

#[test]
fn large_conflict_population_uses_bounded_group_indexes() {
    let root = std::env::temp_dir().join(format!(
        "justice-gakuen2-conflict-index-test-{}",
        std::process::id()
    ));
    if root.exists() {
        std::fs::remove_dir_all(&root).unwrap();
    }

    let shards = (0..400)
        .map(|index| DialogueFontConflictShardRef {
            path: format!("groups/unit-{index:03}.json"),
            content_sha256: format!("{index:064x}"),
            entry_count: 32,
        })
        .collect::<Vec<_>>();
    let indexes = write_group_indexes(&root, &shards).unwrap();

    assert!(indexes.len() > 1);
    assert_eq!(
        indexes.iter().map(|index| index.shard_count).sum::<usize>(),
        shards.len()
    );
    assert_eq!(
        indexes.iter().map(|index| index.entry_count).sum::<usize>(),
        shards.iter().map(|shard| shard.entry_count).sum::<usize>()
    );
    for index in indexes {
        assert!(
            std::fs::metadata(root.join(PathBuf::from(index.path)))
                .unwrap()
                .len()
                <= u64::try_from(MAX_SHARD_BYTES).unwrap()
        );
    }

    std::fs::remove_dir_all(root).unwrap();
}
