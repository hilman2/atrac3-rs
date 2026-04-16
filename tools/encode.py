#!/usr/bin/env python
"""encode.py — versioned encode-and-decode helper.

Produces a uniquely-named `.at3` and `.wav` pair in _tmp_at3stats/ so
iteration loops never overwrite a previous version's output. Tag
defaults to `<engine>_<git-sha>[_dirty]_<HHMM>`; pass --tag to pin it
to a specific label for comparison runs (e.g. --tag frank_v8).

    python tools/encode.py testsong/HateMe.wav --engine frankenstein
    python tools/encode.py testsong/HateMe.wav --tag frank_v12 --engine frankenstein
    python tools/encode.py testsong/HateMe.wav --tag classic_ref --engine classic
    python tools/encode.py testsong/HateMe.wav --sony --tag sony_ref

Prints the output paths at the end — these are the inputs you feed to
ear_judge.py or hf_scope.py next.
"""
import argparse
import os
import subprocess
import sys
from datetime import datetime
from pathlib import Path

try:
    sys.stdout.reconfigure(encoding='utf-8')
    sys.stderr.reconfigure(encoding='utf-8')
except Exception:
    pass

TOOLS_DIR = Path(__file__).resolve().parent
REPO_ROOT = TOOLS_DIR.parent
AT3CMP = REPO_ROOT / 'target' / 'release' / 'at3cmp.exe'
_PSP_CANDIDATES = [
    Path(p) for p in [
        os.environ.get('PSP_AT3TOOL', ''),
        str(Path.cwd() / 'psp_at3tool.exe'),
        str(REPO_ROOT.parent / 'psp_at3tool.exe'),
        str(TOOLS_DIR / 'psp_at3tool.exe'),
    ] if p
]
PSP_TOOL = next((p for p in _PSP_CANDIDATES if p.exists()), _PSP_CANDIDATES[1])
TMP_DIR = Path.cwd() / '_tmp_at3stats'


def auto_tag(engine):
    ts = datetime.now().strftime('%H%M')
    try:
        sha = subprocess.run(
            ['git', 'rev-parse', '--short', 'HEAD'],
            capture_output=True, text=True, cwd=REPO_ROOT, check=True,
        ).stdout.strip()
        dirty = subprocess.run(
            ['git', 'status', '--porcelain'],
            capture_output=True, text=True, cwd=REPO_ROOT, check=True,
        ).stdout.strip()
        suffix = '_d' if dirty else ''
        return f'{engine}_{sha}{suffix}_{ts}'
    except Exception:
        return f'{engine}_{ts}'


def run(cmd, env=None):
    result = subprocess.run(cmd, capture_output=True, text=True, env=env)
    if result.returncode != 0:
        raise RuntimeError(
            f"Command failed ({result.returncode}): {' '.join(map(str, cmd))}\n"
            f"stderr: {result.stderr.strip()}\nstdout: {result.stdout.strip()}"
        )
    return result.stdout


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('input_wav')
    ap.add_argument('--tag', default=None,
                    help='version tag (default: engine_sha_hhmm)')
    ap.add_argument('--engine', choices=['classic', 'frankenstein'],
                    default='frankenstein')
    ap.add_argument('--sony', action='store_true',
                    help='use Sony psp_at3tool for encode instead of at3cmp')
    ap.add_argument('--bitrate', default='k132')
    ap.add_argument('--bitrate-kbps', type=int, default=132)
    ap.add_argument('--frames', type=int, default=9999)
    ap.add_argument('--force', action='store_true',
                    help='overwrite an existing output with the same tag')
    args = ap.parse_args()

    TMP_DIR.mkdir(exist_ok=True)
    input_wav = Path(args.input_wav).resolve()
    if not input_wav.exists():
        sys.exit(f"input not found: {input_wav}")

    if args.sony:
        engine_label = 'sony'
    else:
        engine_label = args.engine
    tag = args.tag or auto_tag(engine_label)

    stem = input_wav.stem
    out_at3 = TMP_DIR / f'{stem}_{tag}.at3'
    out_wav = TMP_DIR / f'{stem}_{tag}.wav'

    if out_wav.exists() and not args.force:
        sys.exit(f"output exists: {out_wav}  (pass --force to overwrite, or pick a different --tag)")

    # Remove stale outputs — psp_at3tool refuses to overwrite.
    for p in (out_at3, out_wav):
        if p.exists():
            p.unlink()

    env = os.environ.copy()
    if args.engine == 'frankenstein' and not args.sony:
        env['ATRAC3_ENGINE'] = 'frankenstein'
    elif 'ATRAC3_ENGINE' in env:
        del env['ATRAC3_ENGINE']

    print(f"encoding {input_wav.name}  →  {tag}   (engine={engine_label})")

    if args.sony:
        run([str(PSP_TOOL), '-e', '-br', str(args.bitrate_kbps),
             str(input_wav), str(out_at3)], env=env)
    else:
        if not AT3CMP.exists():
            sys.exit(f"at3cmp.exe not found: {AT3CMP}\n"
                     f"  build with: cd {REPO_ROOT} && cargo build --release")
        run([str(AT3CMP), 'proto-at3',
             '--input', str(input_wav),
             '--output', str(out_at3),
             '--frames', str(args.frames),
             '--coding-mode', 'vlc',
             '--bitrate', args.bitrate], env=env)

    run([str(PSP_TOOL), '-d', str(out_at3), str(out_wav)], env=env)

    print(f"  {out_at3}")
    print(f"  {out_wav}")
    print(f"tag: {tag}")


if __name__ == '__main__':
    main()
