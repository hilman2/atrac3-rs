use crate::metrics::{CompareMetrics, compare_wavs, read_wav};
use anyhow::{Context, Result, bail, ensure};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::tempdir;

#[derive(Debug, Clone)]
pub struct ReferenceEncode {
    pub bitrate_kbps: u32,
    pub loop_start: Option<u32>,
    pub loop_end: Option<u32>,
    pub whole_loop: bool,
}

#[derive(Debug, Clone)]
pub struct OracleConfig {
    pub tool_path: PathBuf,
    pub source_wav: Option<PathBuf>,
    pub candidate_at3: PathBuf,
    pub reference_at3: PathBuf,
    pub reference_encode: Option<ReferenceEncode>,
    pub decoded_dir: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct OracleResult {
    pub metrics: CompareMetrics,
    pub candidate_decoded: PathBuf,
    pub reference_decoded: PathBuf,
}

pub fn encode_reference(
    tool: &Path,
    source_wav: &Path,
    output_at3: &Path,
    settings: &ReferenceEncode,
) -> Result<()> {
    if let (Some(start), Some(end)) = (settings.loop_start, settings.loop_end) {
        ensure!(start < end, "loop start must be smaller than loop end");
        ensure!(
            start + 6143 <= end,
            "loop range must satisfy start + 6143 <= end for the reference tool"
        );
    }

    let mut args = vec![
        "-e".to_string(),
        "-br".to_string(),
        settings.bitrate_kbps.to_string(),
    ];

    if settings.whole_loop {
        args.push("-wholeloop".to_string());
    } else if let (Some(start), Some(end)) = (settings.loop_start, settings.loop_end) {
        args.extend(["-loop".to_string(), start.to_string(), end.to_string()]);
    }

    args.push(source_wav.display().to_string());
    args.push(output_at3.display().to_string());

    run_tool(tool, &args).with_context(|| {
        format!(
            "failed to encode reference AT3 {} -> {}",
            source_wav.display(),
            output_at3.display()
        )
    })
}

pub fn decode_at3(tool: &Path, input_at3: &Path, output_wav: &Path) -> Result<()> {
    run_tool(
        tool,
        &[
            "-d".to_string(),
            input_at3.display().to_string(),
            output_wav.display().to_string(),
        ],
    )
    .with_context(|| {
        format!(
            "failed to decode AT3 {} -> {}",
            input_at3.display(),
            output_wav.display()
        )
    })
}

pub fn run_oracle(config: &OracleConfig) -> Result<OracleResult> {
    ensure!(
        config.tool_path.exists(),
        "tool not found: {}",
        config.tool_path.display()
    );
    ensure!(
        config.candidate_at3.exists(),
        "candidate AT3 not found: {}",
        config.candidate_at3.display()
    );

    if !config.reference_at3.exists() {
        let source = config
            .source_wav
            .as_ref()
            .context("reference AT3 missing and no source WAV supplied")?;
        let settings = config
            .reference_encode
            .as_ref()
            .context("reference AT3 missing and no bitrate/reference encode settings supplied")?;
        encode_reference(&config.tool_path, source, &config.reference_at3, settings)?;
    }

    let owned_tempdir;
    let decoded_dir = if let Some(dir) = &config.decoded_dir {
        fs::create_dir_all(dir)?;
        dir.clone()
    } else {
        owned_tempdir = tempdir()?;
        owned_tempdir.path().to_path_buf()
    };

    let candidate_decoded = decoded_dir.join("candidate_decoded.wav");
    let reference_decoded = decoded_dir.join("reference_decoded.wav");

    decode_at3(&config.tool_path, &config.candidate_at3, &candidate_decoded)?;
    decode_at3(&config.tool_path, &config.reference_at3, &reference_decoded)?;

    let candidate = read_wav(&candidate_decoded)?;
    let reference = read_wav(&reference_decoded)?;
    let metrics = compare_wavs(&reference, &candidate)?;

    Ok(OracleResult {
        metrics,
        candidate_decoded,
        reference_decoded,
    })
}

fn run_tool(tool: &Path, args: &[String]) -> Result<()> {
    let output = Command::new(tool)
        .args(args)
        .output()
        .with_context(|| format!("failed to spawn {}", tool.display()))?;

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!(
            "{} exited with {}\nstdout:\n{}\nstderr:\n{}",
            tool.display(),
            output.status,
            stdout,
            stderr
        );
    }

    Ok(())
}
