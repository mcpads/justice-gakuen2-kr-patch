use super::*;

#[test]
fn parses_and_rewrites_supported_cue() {
    let dir = std::env::temp_dir().join(format!("justice-cue-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let cue = dir.join("source.cue");
    std::fs::write(
        &cue,
        "FILE \"source.bin\" BINARY\n  TRACK 01 MODE2/2352\n    INDEX 01 00:00:00\n",
    )
    .unwrap();
    let parsed = CueSheet::parse(&cue).unwrap();
    assert_eq!(parsed.image_path, dir.join("source.bin"));
    let rewritten = parsed
        .rewritten_for(&dir.join("out.cue"), &dir.join("out.bin"))
        .unwrap();
    assert!(rewritten.starts_with("FILE \"out.bin\" BINARY\n"));
    std::fs::remove_dir_all(dir).unwrap();
}
