use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail, ensure};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CueSheet {
    pub image_path: PathBuf,
    pub source_text: String,
}

impl CueSheet {
    pub fn parse(path: &Path) -> Result<Self> {
        let source_text = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read CUE: {}", path.display()))?;
        ensure!(source_text.is_ascii(), "CUE must be ASCII");

        let mut file_name = None;
        let mut track_count = 0usize;
        let mut mode = None;
        let mut index_01 = None;

        for (line_number, raw) in source_text.lines().enumerate() {
            let line = raw.trim();
            if line.is_empty() {
                continue;
            }
            let upper = line.to_ascii_uppercase();
            if upper.starts_with("FILE ") {
                ensure!(file_name.is_none(), "CUE must contain exactly one FILE");
                let first_quote = line.find('"').with_context(|| {
                    format!("CUE FILE must use quotes at line {}", line_number + 1)
                })?;
                let rest = &line[first_quote + 1..];
                let second_quote = rest.find('"').with_context(|| {
                    format!("unterminated CUE FILE quote at line {}", line_number + 1)
                })?;
                let suffix = rest[second_quote + 1..].trim();
                ensure!(
                    suffix.eq_ignore_ascii_case("BINARY"),
                    "CUE FILE must use BINARY"
                );
                file_name = Some(rest[..second_quote].to_string());
            } else if upper.starts_with("TRACK ") {
                track_count += 1;
                let fields: Vec<_> = line.split_whitespace().collect();
                ensure!(fields.len() == 3, "invalid TRACK line");
                mode = Some(fields[2].to_ascii_uppercase());
            } else if upper.starts_with("INDEX 01 ") {
                let fields: Vec<_> = line.split_whitespace().collect();
                ensure!(fields.len() == 3, "invalid INDEX line");
                index_01 = Some(parse_msf(fields[2])?);
            } else if ["REM ", "TITLE ", "PERFORMER ", "CATALOG ", "PREGAP "]
                .iter()
                .any(|prefix| upper.starts_with(prefix))
            {
                continue;
            } else {
                bail!("unsupported CUE line {}: {raw:?}", line_number + 1);
            }
        }

        ensure!(track_count == 1, "CUE must contain exactly one TRACK");
        ensure!(mode.as_deref() == Some("MODE2/2352"), "expected MODE2/2352");
        ensure!(index_01 == Some(0), "expected INDEX 01 00:00:00");
        let file_name = file_name.context("CUE does not contain FILE")?;
        let parent = path.parent().unwrap_or_else(|| Path::new("."));

        Ok(Self {
            image_path: parent.join(file_name),
            source_text,
        })
    }

    pub fn rewritten_for(&self, output_cue: &Path, output_bin: &Path) -> Result<String> {
        let file_name = output_bin
            .file_name()
            .context("output BIN has no file name")?
            .to_str()
            .context("output BIN file name is not UTF-8")?;
        ensure!(
            output_bin.parent() == output_cue.parent(),
            "output CUE and BIN must share a directory"
        );

        let replacement = format!("FILE \"{file_name}\" BINARY");
        let mut replaced = false;
        let mut output = String::new();
        for raw in self.source_text.split_inclusive('\n') {
            let line = raw.strip_suffix('\n').unwrap_or(raw);
            let newline = if raw.ends_with('\n') { "\n" } else { "" };
            if !replaced && line.trim_start().to_ascii_uppercase().starts_with("FILE ") {
                output.push_str(&replacement);
                output.push_str(newline);
                replaced = true;
            } else {
                output.push_str(raw);
            }
        }
        ensure!(replaced, "CUE FILE line disappeared during rewrite");
        Ok(output)
    }
}

fn parse_msf(value: &str) -> Result<u32> {
    let fields: Vec<_> = value.split(':').collect();
    ensure!(fields.len() == 3, "invalid CUE timestamp: {value}");
    let minute: u32 = fields[0].parse()?;
    let second: u32 = fields[1].parse()?;
    let frame: u32 = fields[2].parse()?;
    ensure!(second < 60 && frame < 75, "invalid CUE timestamp: {value}");
    Ok((minute * 60 + second) * 75 + frame)
}

#[cfg(test)]
#[path = "cue_tests.rs"]
mod tests;
