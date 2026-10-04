use std::path::PathBuf;

use clap::Parser;

use super::{Cli, Command};
use justice_gakuen2_kr::dialogue_audit::DialogueDevelopmentInputPolicy;

const CLI_PARSER_TEST_STACK_BYTES: usize = 16 * 1024 * 1024;

#[test]
fn development_build_commands_share_one_default_spec() {
    run_cli_parser_test(|| {
        let cli = Cli::try_parse_from([
            "justice-gakuen2-kr",
            "build-dialogue-disc",
            "--prepared-dialogue",
            "prepared",
            "--cue",
            "original.cue",
        ])
        .unwrap();

        let Command::BuildDialogueDisc {
            spec, input_policy, ..
        } = cli.command
        else {
            panic!("parsed the wrong command");
        };

        assert_eq!(spec, PathBuf::from("assets/build/development.json"));
        assert_eq!(input_policy, DialogueDevelopmentInputPolicy::CompleteScope);

        for command in [
            "build-dialogue-fonts",
            "build-dialogue-name-entry",
            "build-mode-select-assets",
            "build-mode-descendant-assets",
            "build-options-assets",
        ] {
            let mut args = vec!["justice-gakuen2-kr", command, "--cue", "original.cue"];
            if command == "build-dialogue-fonts" {
                args.extend(["--prepared-dialogue", "prepared"]);
            }
            let cli = Cli::try_parse_from(args).unwrap();
            let spec = match cli.command {
                Command::BuildDialogueFonts { spec, .. }
                | Command::BuildDialogueNameEntry { spec, .. }
                | Command::BuildModeSelectAssets { spec, .. }
                | Command::BuildModeDescendantAssets { spec, .. }
                | Command::BuildOptionsAssets { spec, .. } => spec,
                _ => panic!("parsed the wrong command"),
            };
            assert_eq!(spec, PathBuf::from("assets/build/development.json"));
        }
    });
}

#[test]
fn mode_select_graphics_audit_uses_one_default_spec() {
    run_cli_parser_test(|| {
        let cli =
            Cli::try_parse_from(["justice-gakuen2-kr", "audit-mode-select-graphics"]).unwrap();
        let Command::AuditModeSelectGraphics { spec, force } = cli.command else {
            panic!("parsed the wrong command");
        };
        assert_eq!(spec, PathBuf::from("specs/mode-select-graphics-audit.json"));
        assert!(!force);
    });
}

#[test]
fn disc_asset_comparison_defaults_to_an_ignored_work_directory() {
    run_cli_parser_test(|| {
        let cli = Cli::try_parse_from([
            "justice-gakuen2-kr",
            "compare-disc-assets",
            "--source-cue",
            "original.cue",
            "--patched-cue",
            "patched.cue",
        ])
        .unwrap();
        let Command::CompareDiscAssets {
            output_dir, force, ..
        } = cli.command
        else {
            panic!("parsed the wrong command");
        };
        assert_eq!(output_dir, PathBuf::from("work/disc-asset-comparison"));
        assert!(!force);
    });
}

#[test]
fn authored_subset_policy_reaches_font_and_disc_build_commands() {
    run_cli_parser_test(|| {
        let fonts = Cli::try_parse_from([
            "justice-gakuen2-kr",
            "build-dialogue-fonts",
            "--prepared-dialogue",
            "prepared",
            "--cue",
            "original.cue",
            "--input-policy",
            "authored-subset",
        ])
        .unwrap();
        let Command::BuildDialogueFonts { input_policy, .. } = fonts.command else {
            panic!("parsed the wrong command");
        };
        assert_eq!(input_policy, DialogueDevelopmentInputPolicy::AuthoredSubset);

        let disc = parse_dialogue_disc(&["--input-policy", "authored-subset"]);
        let Command::BuildDialogueDisc { input_policy, .. } = disc.command else {
            panic!("parsed the wrong command");
        };
        assert_eq!(input_policy, DialogueDevelopmentInputPolicy::AuthoredSubset);
    });
}

#[test]
fn development_runtime_audit_requires_explicit_emucap_identity() {
    run_cli_parser_test(|| {
        let cli = Cli::try_parse_from([
            "justice-gakuen2-kr",
            "audit-dialogue-development-runtime",
            "--disc-report",
            "selected-disc.json",
            "--disc-bin",
            "selected-disc.bin",
            "--runtime-ram",
            "selected-ram.bin",
            "--runtime-frame",
            "selected-frame.png",
            "--launch-id",
            "launch-example",
            "--emulator-build",
            "mednafen-example",
            "--capability-revision",
            "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        ])
        .unwrap();

        let Command::AuditDialogueDevelopmentRuntime {
            source_path,
            launch_id,
            emulator_build,
            ..
        } = cli.command
        else {
            panic!("parsed the wrong command");
        };
        assert_eq!(source_path, "DAT2/MGG04T.BIZ");
        assert_eq!(launch_id, "launch-example");
        assert_eq!(emulator_build, "mednafen-example");
    });
}

fn run_cli_parser_test(test: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .name("cli-parser-test".to_string())
        .stack_size(CLI_PARSER_TEST_STACK_BYTES)
        .spawn(test)
        .expect("spawn CLI parser test thread")
        .join()
        .expect("CLI parser test thread failed");
}

fn parse_dialogue_disc(extra_args: &[&str]) -> Cli {
    let mut args = vec![
        "justice-gakuen2-kr",
        "build-dialogue-disc",
        "--prepared-dialogue",
        "prepared",
        "--cue",
        "original.cue",
    ];
    args.extend_from_slice(extra_args);
    Cli::try_parse_from(args).unwrap()
}

#[test]
fn product_builds_require_explicit_prepared_dialogue() {
    run_cli_parser_test(|| {
        for command in ["build-dialogue-disc", "build-dialogue-fonts"] {
            let error =
                Cli::try_parse_from(["justice-gakuen2-kr", command, "--cue", "original.cue"])
                    .expect_err("build must not infer or regenerate prepared inputs");
            assert_eq!(
                error.kind(),
                clap::error::ErrorKind::MissingRequiredArgument
            );
            assert!(error.to_string().contains("--prepared-dialogue"));
        }
    });
}

#[test]
fn archived_evidence_inputs_must_be_selected_explicitly() {
    run_cli_parser_test(|| {
        assert!(
            Cli::try_parse_from([
                "justice-gakuen2-kr",
                "audit-dialogue-scenes",
                "--cue",
                "original.cue"
            ])
            .is_err()
        );
        let parsed = Cli::try_parse_from([
            "justice-gakuen2-kr",
            "audit-dialogue-scenes",
            "--cue",
            "original.cue",
            "--runtime-ram",
            "/restored/evidence/ram.bin",
            "--runtime-frame",
            "/restored/evidence/frame.png",
        ])
        .unwrap();
        match parsed.command {
            Command::AuditDialogueScenes {
                runtime_ram,
                runtime_frame,
                ..
            } => {
                assert_eq!(runtime_ram, PathBuf::from("/restored/evidence/ram.bin"));
                assert_eq!(runtime_frame, PathBuf::from("/restored/evidence/frame.png"));
            }
            _ => panic!("unexpected command"),
        }
        assert!(
            Cli::try_parse_from(["justice-gakuen2-kr", "sync-dialogue-translation-assets"])
                .is_err()
        );
    });
}
