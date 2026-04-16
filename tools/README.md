# atrac3-rs / tools

Unified test suite for the atrac3-rs encoder.

## ears.py

One input WAV → full professional-grade measurement pipeline.

```
cd /path/to/test-assets   # directory with testsong/, psp_at3tool.exe
python /path/to/atrac3-rs/tools/ears.py testsong/Crystallize90.wav [--sony] [--html] [--quick]
```

### What it does

1. Encodes with `at3cmp proto-at3` (VLC, k132 by default).
2. Decodes with `psp_at3tool.exe -d`.
3. Runs the full metric stack against the original:
   - **digital_ears** — SNR, HF Envelope Correlation, Pre-Echo, Per-Band SNR,
     Artefakt-Lokalisation, Spektrogramm-Diff PNG
   - **stereo_ears** — Mid/Side SNR, L-R correlation, per-band width
   - **ears_metrics** — K-weighted LUFS delta, spectral features (centroid,
     rolloff, flatness, flux), dynamic range (crest factor + DR14),
     Bark-band NMR, onset preservation (F1), THD+N
4. Optional `--sony`: encode Sony reference too for a 3-way comparison.
5. Optional `--html FILE`: self-contained HTML report with colour-coded
   pass/fail tables and embedded spectrogram.
6. Optional `--json FILE`: machine-readable metrics dump.

### Pass / Fail thresholds

Tuned for ATRAC3 @ 132 kbps VLC. Defined in `ears_report.THRESHOLDS`:

| Metric               | good   | warn     | fail    |
|----------------------|--------|----------|---------|
| SNR                  | ≥ 18 dB| 14-18 dB | < 14 dB |
| HF Env Corr          | ≥ 0.85 | 0.75-0.85| < 0.75  |
| Pre-Echo worst       | ≤ 2.5  | 2.5-5    | > 5     |
| NMR max (Bark)       | < 0 dB | 0-6      | > 6     |
| Loudness Δ (K-wgt)   | < 0.5  | 0.5-2    | > 2     |
| Onset F1             | ≥ 0.90 | 0.75-0.90| < 0.75  |
| Centroid shift       | < 300 Hz| 300-1000 Hz| > 1000 Hz|

### CLAUDE.md traps, handled automatically

- `--coding-mode vlc` is always passed (CLC crashes the Sony decoder with 0x1000105).
- Output WAVs are removed before decode (psp_at3tool refuses to overwrite).
- Bitrate defaults to `k132` (the project reference point).

### Options

```
--sony              also encode a Sony reference and compare
--bitrate FLAG      at3cmp bitrate (default k132)
--bitrate-kbps N    psp_at3tool bitrate in kbps (default 132)
--label NAME        override the label used in headings
--no-stereo         skip stereo analysis
--no-plot           skip spectrogram PNG
--no-artifacts      skip the artefact-localisation section
--quick             equivalent to --no-plot --no-artifacts
--json FILE         write the full metric dict as JSON
--html [FILE]       write the HTML report (empty = auto-name next to input)
```

### Environment variables

- `PSP_AT3TOOL` — absolute path to psp_at3tool.exe (otherwise searched in
  cwd, repo parent, and tools/ directory)
