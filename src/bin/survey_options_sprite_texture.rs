use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use justice_gakuen2_kr::options::{
    OptionsSpriteTextureSurveyConfig, survey_options_sprite_texture,
};

#[derive(Debug, Parser)]
#[command(name = "survey-options-sprite-texture")]
#[command(about = "Render the source-bound options sprite TIM outside the build pipeline")]
struct Cli {
    /// Exact original single-track MODE2/2352 CUE.
    #[arg(long)]
    cue: PathBuf,
    /// Directory for source-bound PNG views and their report.
    #[arg(long, default_value = "work/options-sprite-texture")]
    output_dir: PathBuf,
    /// Replace outputs owned by this survey.
    #[arg(long)]
    force: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let report = survey_options_sprite_texture(&OptionsSpriteTextureSurveyConfig {
        cue: cli.cue,
        output_dir: cli.output_dir,
        force: cli.force,
    })?;
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
