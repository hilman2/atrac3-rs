#!/usr/bin/env python
"""bench.py — one-shot benchmark across all four test snippets.

Builds the encoder, encodes the four 30-s snippets with the chosen
engine and tag, scores them against the orig with ear_judge, and
prints the ranked table plus the sum.

    python tools/bench.py [--tag TAG] [--engine ENG] [--label LBL]
                          [--against classic_ref,sony_ref]

The `--against` flag adds versioned baselines to every comparison so
we always see the current run next to Sony/Classic for context.
"""
import argparse
import json
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
PYTHON = sys.executable


def auto_tag(engine):
    ts = datetime.now().strftime('%H%M%S')
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


def run_quiet(cmd, env=None, cwd=None):
    r = subprocess.run(cmd, capture_output=True, text=True, env=env, cwd=cwd)
    if r.returncode != 0:
        print(f"FAILED: {' '.join(map(str, cmd))}", file=sys.stderr)
        print(r.stderr.strip(), file=sys.stderr)
        sys.exit(1)
    return r.stdout


def encode_song(song_name, tag, engine):
    """Run tools/encode.py for one song. Returns the output .wav path."""
    input_wav = Path('testsong') / f'{song_name}.wav'
    out_wav = Path('_tmp_at3stats') / f'{song_name}_{tag}.wav'
    if out_wav.exists():
        # already have it
        return out_wav
    cmd = [PYTHON, str(TOOLS_DIR / 'encode.py'),
           str(input_wav), '--tag', tag, '--engine', engine, '--force']
    run_quiet(cmd)
    return out_wav


def ensure_baseline(song_name, tag, engine):
    out_wav = Path('_tmp_at3stats') / f'{song_name}_{tag}.wav'
    if not out_wav.exists():
        cmd = [PYTHON, str(TOOLS_DIR / 'encode.py'),
               str(Path('testsong') / f'{song_name}.wav'),
               '--tag', tag]
        if engine == 'sony':
            cmd += ['--sony']
        else:
            cmd += ['--engine', engine]
        run_quiet(cmd)
    return out_wav


def judge(song_name, encoded_path, label):
    """Run ear_judge.py single form, return parsed result dict."""
    orig = Path('testsong') / f'{song_name}.wav'
    json_path = Path('_tmp_at3stats') / f'{song_name}_{label}_judge.json'
    cmd = [PYTHON, str(TOOLS_DIR / 'ear_judge.py'),
           str(orig), str(encoded_path), '--label', label,
           '--json', str(json_path), '--quiet']
    run_quiet(cmd)
    return json.loads(json_path.read_text(encoding='utf-8'))


def main():
    ap = argparse.ArgumentParser(description=__doc__,
                                 formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument('--tag', default=None)
    ap.add_argument('--engine', choices=['classic', 'frankenstein'],
                    default='frankenstein')
    ap.add_argument('--label', default=None,
                    help='label for the output column (default: tag)')
    ap.add_argument('--against', default='sony_ref,classic_ref',
                    help='comma-separated baseline tags to include')
    ap.add_argument('--songs', default='HateMe_30s,Crystallize90_30s,Three_Twelve_30s,classic_30s')
    args = ap.parse_args()

    tag = args.tag or auto_tag(args.engine)
    label = args.label or tag
    songs = args.songs.split(',')
    baselines = args.against.split(',') if args.against else []

    # make sure the encoder is built
    print("building release binary …")
    r = subprocess.run(['cargo', 'build', '--release'],
                       capture_output=True, text=True, cwd=REPO_ROOT)
    if r.returncode != 0:
        print("BUILD FAILED:", file=sys.stderr)
        print(r.stderr[-2000:], file=sys.stderr)
        sys.exit(1)

    # Ensure baselines exist for each song
    for song in songs:
        for bl in baselines:
            # infer engine from tag
            eng = 'sony' if 'sony' in bl else ('classic' if 'classic' in bl else args.engine)
            ensure_baseline(song, bl, eng)

    # Encode the candidate on each song
    print(f"encoding candidate  engine={args.engine}  tag={tag}")
    candidate_wavs = {}
    for song in songs:
        candidate_wavs[song] = encode_song(song, tag, args.engine)

    # Judge every song: candidate + each baseline
    print()
    header = f"{'Song':<22s}  {'Source':<13s}"
    for bl in baselines:
        header += f"  {bl:>10s}"
    header += f"  {label:>11s}"
    print(header)
    print('-' * len(header))

    totals = {bl: 0.0 for bl in baselines}
    totals[label] = 0.0

    for song in songs:
        orig = Path('testsong') / f'{song}.wav'
        cols = []
        source_cls = ''
        for bl in baselines:
            wav = Path('_tmp_at3stats') / f'{song}_{bl}.wav'
            r = judge(song, wav, bl)
            cols.append(r['score'])
            source_cls = r['source_class']
            totals[bl] += r['score']
        r = judge(song, candidate_wavs[song], label)
        cand_score = r['score']
        totals[label] += cand_score

        line = f"{song:<22s}  {source_cls:<13s}"
        for s in cols:
            line += f"  {s:>10.2f}"
        marker = ''
        best = min(cols + [cand_score])
        if cand_score == best:
            marker = ' *'
        line += f"  {cand_score:>10.2f}{marker}"
        print(line)
    print('-' * len(header))
    sum_line = f"{'SUM':<22s}  {'':<13s}"
    for bl in baselines:
        sum_line += f"  {totals[bl]:>10.2f}"
    sum_line += f"  {totals[label]:>10.2f}"
    print(sum_line)
    print()
    # final verdict
    min_bl = min((totals[bl], bl) for bl in baselines)
    if totals[label] < min_bl[0]:
        print(f"→ candidate BEATS best baseline ({min_bl[1]} = {min_bl[0]:.2f})")
    else:
        print(f"→ best baseline {min_bl[1]} = {min_bl[0]:.2f}  |  candidate {totals[label]:.2f}  "
              f"(Δ = {totals[label]-min_bl[0]:+.2f})")


if __name__ == '__main__':
    main()
