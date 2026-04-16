#!/usr/bin/env python
"""ears_report.py — self-contained HTML-Report-Generator für ears.py.

Erzeugt einen standalone HTML-Bericht aus einem Results-Dict. Keine
externen Template-Engines — reines String-Formatting aus stdlib, damit
die Test-Suite überall läuft.

Features:
  - Zusammenfassungs-Karte mit Pass/Fail-Bewertung
  - Tabellen pro Metrik-Gruppe (digital_ears, stereo, loudness, …)
  - Farb-codierte Delta-Spalten (grün/gelb/rot je nach Schwelle)
  - Eingebettetes Spektrogramm-PNG als base64 data-URI
  - Optional 3-fach-Vergleich gegen Sony-Referenz
  - Responsive Layout, Dark-Mode-freundlich
"""
from __future__ import annotations
import base64
import html
from datetime import datetime
from pathlib import Path


# ---- Pass/Fail thresholds (tuned for ATRAC3 @ 132 kbps VLC) ----

THRESHOLDS = {
    # (metric_key, 'good' if >= x, 'warn' if >= y, else 'fail')
    # direction: 'hi' means higher-is-better, 'lo' means lower-is-better,
    # 'abs' means closer-to-0 is better.
    'snr':            ('hi',  18.0, 14.0),
    'hf_corr':        ('hi',  0.85, 0.75),
    'pe_worst':       ('lo',  2.5,  5.0),
    'nf':             ('lo', -75.0, -60.0),
    'nmr_max_db':     ('lo',  0.0,  6.0),
    'loudness_delta': ('abs', 0.5,  2.0),
    'centroid_shift_hz': ('abs', 300.0, 1000.0),
    'onset_f1':       ('hi',  0.90, 0.75),
    'crest_delta_db': ('abs', 1.0,  3.0),
    'dr14_delta':     ('abs', 1.5,  4.0),
    'flux_ratio':     ('abs_1', 0.15, 0.40),  # abs(ratio - 1)
}


def _rate(key, value):
    """Return 'good'/'warn'/'fail' for a metric value."""
    if value is None:
        return 'info'
    # Accept numpy scalar types which are not isinstance(float).
    try:
        value = float(value)
    except (TypeError, ValueError):
        return 'info'
    if key not in THRESHOLDS:
        return 'info'
    direction, good, warn = THRESHOLDS[key]
    if direction == 'hi':
        if value >= good: return 'good'
        if value >= warn: return 'warn'
        return 'fail'
    if direction == 'lo':
        if value <= good: return 'good'
        if value <= warn: return 'warn'
        return 'fail'
    if direction == 'abs':
        v = abs(value)
        if v <= good: return 'good'
        if v <= warn: return 'warn'
        return 'fail'
    if direction == 'abs_1':
        v = abs(value - 1.0)
        if v <= good: return 'good'
        if v <= warn: return 'warn'
        return 'fail'
    return 'info'


# ---- HTML building blocks ----

_CSS = """
:root {
  --bg: #0e1116; --panel: #161a22; --border: #2c313c;
  --text: #e4e8ef; --muted: #8b94a5;
  --good: #3fb950; --warn: #d29922; --fail: #f85149; --info: #58a6ff;
  --row-alt: #1a1f29;
}
* { box-sizing: border-box; }
body { margin: 0; padding: 2rem; background: var(--bg); color: var(--text);
       font: 14px/1.5 -apple-system, Segoe UI, Roboto, Helvetica, Arial, sans-serif; }
h1 { font-size: 1.5rem; margin: 0 0 .2rem; }
h2 { font-size: 1.1rem; margin: 1.2rem 0 .4rem; border-bottom: 1px solid var(--border); padding-bottom: .2rem; }
h3 { font-size: 0.95rem; margin: .8rem 0 .3rem; color: var(--muted); }
.container { max-width: 1100px; margin: 0 auto; }
.meta { color: var(--muted); font-size: .85rem; margin-bottom: 1.5rem; }
.summary { display: grid; grid-template-columns: repeat(auto-fit, minmax(180px, 1fr));
           gap: .6rem; margin-bottom: 1.5rem; }
.card { background: var(--panel); border: 1px solid var(--border); border-radius: 6px;
        padding: .8rem 1rem; }
.card .k { font-size: .75rem; color: var(--muted); text-transform: uppercase; letter-spacing: .05em; }
.card .v { font-size: 1.6rem; font-weight: 600; margin-top: .2rem; }
.card.good .v { color: var(--good); }
.card.warn .v { color: var(--warn); }
.card.fail .v { color: var(--fail); }
.card .sub { font-size: .75rem; color: var(--muted); margin-top: .3rem; }
table { width: 100%; border-collapse: collapse; background: var(--panel);
        border: 1px solid var(--border); border-radius: 6px; overflow: hidden; font-size: .9rem; }
th, td { padding: .45rem .7rem; text-align: left; border-bottom: 1px solid var(--border); }
th { background: #1d222c; color: var(--muted); font-weight: 600; font-size: .8rem;
     text-transform: uppercase; letter-spacing: .04em; }
tbody tr:nth-child(even) { background: var(--row-alt); }
tbody tr:last-child td { border-bottom: none; }
td.num { text-align: right; font-variant-numeric: tabular-nums; font-family: Consolas, Menlo, monospace; }
td.good { color: var(--good); font-weight: 600; }
td.warn { color: var(--warn); font-weight: 600; }
td.fail { color: var(--fail); font-weight: 600; }
td.info { color: var(--info); }
td.muted { color: var(--muted); }
.spectro { margin: 1rem 0; }
.spectro img { max-width: 100%; border: 1px solid var(--border); border-radius: 6px; }
.pill { display: inline-block; padding: .1rem .55rem; border-radius: 999px;
        font-size: .72rem; font-weight: 600; text-transform: uppercase; letter-spacing: .05em; }
.pill.good { background: rgba(63,185,80,.15); color: var(--good); }
.pill.warn { background: rgba(210,153,34,.15); color: var(--warn); }
.pill.fail { background: rgba(248,81,73,.15); color: var(--fail); }
.pill.info { background: rgba(88,166,255,.15); color: var(--info); }
.note { color: var(--muted); font-size: .82rem; margin: .3rem 0 .8rem; }
footer { color: var(--muted); font-size: .75rem; text-align: center; margin-top: 3rem; padding-top: 1rem;
         border-top: 1px solid var(--border); }
details { background: var(--panel); border: 1px solid var(--border); border-radius: 6px;
          padding: .4rem .8rem; margin: .4rem 0; }
details > summary { cursor: pointer; font-weight: 600; padding: .2rem 0; user-select: none; }
"""


def _fmt(v, precision=2):
    if v is None:
        return '—'
    if isinstance(v, float):
        return f"{v:+.{precision}f}" if abs(v) < 1 else f"{v:.{precision}f}"
    return str(v)


def _row(name, value, rating='info', sony=None, unit='', desc=''):
    cls = rating
    v_str = _fmt(value) + (f" {unit}" if unit and value is not None else '')
    sony_str = _fmt(sony) + (f" {unit}" if unit and sony is not None else '')
    return (
        f"<tr>"
        f"<td>{html.escape(name)}</td>"
        f"<td class='num {cls}'>{v_str}</td>"
        f"<td class='num muted'>{sony_str}</td>"
        f"<td class='muted'>{html.escape(desc)}</td>"
        f"</tr>"
    )


def _table(title, rows, note=None):
    head = "<tr><th>Metrik</th><th style='text-align:right'>Wert</th><th style='text-align:right'>Sony</th><th>Interpretation</th></tr>"
    note_html = f"<p class='note'>{html.escape(note)}</p>" if note else ''
    return f"<h3>{html.escape(title)}</h3>{note_html}<table><thead>{head}</thead><tbody>{''.join(rows)}</tbody></table>"


def _summary_card(key, label, value, unit='', desc=''):
    rating = _rate(key, value)
    return (
        f"<div class='card {rating}'>"
        f"<div class='k'>{html.escape(label)}</div>"
        f"<div class='v'>{_fmt(value)}{html.escape(' ' + unit) if unit else ''}</div>"
        f"<div class='sub'>{html.escape(desc)}</div>"
        f"</div>"
    )


def _embed_png(path):
    if not path or not Path(path).exists():
        return ''
    data = Path(path).read_bytes()
    b64 = base64.b64encode(data).decode('ascii')
    return f"<div class='spectro'><img src='data:image/png;base64,{b64}' alt='Spektrogramm'/></div>"


def _render_digital_ears(de, sony_de=None):
    rows = []
    sony_vals = sony_de or {}
    rows.append(_row('SNR overall',         de.get('snr'),      _rate('snr', de.get('snr')),      sony_vals.get('snr'),      'dB', '≥18 gut, <14 hörbar'))
    rows.append(_row('Noise Floor',         de.get('nf'),       _rate('nf', de.get('nf')),        sony_vals.get('nf'),       'dBFS', 'tiefer = besser'))
    rows.append(_row('Pre-Echo worst',      de.get('pe_worst'), _rate('pe_worst', de.get('pe_worst')), sony_vals.get('pe_worst'), 'dB', '<2.5 gut, >5 hörbar'))
    rows.append(_row('HF Envelope Corr',    de.get('hf_corr'),  _rate('hf_corr', de.get('hf_corr')), sony_vals.get('hf_corr'),  '',   'Sony ≈ 0.928'))
    rows.append(_row('Spectral Flatness |Δ|', abs(de.get('sf', 0)) if de.get('sf') is not None else None, 'info', sony_vals.get('sf'), '',  'nahe 0 = gute Spektralform'))
    rows.append(_row('Phase err',           de.get('phase'),    'info', sony_vals.get('phase'), 'rad', 'Stereo + Transients'))
    out = _table('Digital Ears — Zeitbereich + Spektrum', rows)

    if de.get('bands'):
        brows = []
        for bname, data in de['bands'].items():
            if isinstance(data, tuple):
                snr_val, en = data
            else:
                snr_val, en = data, None
            sony_band = sony_vals.get('bands', {}).get(bname) if sony_vals.get('bands') else None
            sony_snr = sony_band[0] if isinstance(sony_band, tuple) else sony_band
            rating = _rate('snr', snr_val)
            brows.append(_row(bname, snr_val, rating, sony_snr, 'dB',
                              f"E={en:+.2f}dB" if en is not None else ''))
        out += _table('Per-Band SNR', brows)
    return out


def _render_stereo(st):
    if not st:
        return ''
    rows = []
    rows.append(_row('Mid SNR',            st.get('mid_snr'),      'info', None, 'dB', ''))
    rows.append(_row('Side SNR',           st.get('side_snr'),     'info', None, 'dB', 'Stereoseite erhalten?'))
    rows.append(_row('L-R correlation ref',  st.get('r_lr_corr'),  'info', None, '',   ''))
    rows.append(_row('L-R correlation test', st.get('t_lr_corr'),  'info', None, '',   ''))
    rows.append(_row('Width ref',          st.get('r_width'),      'info', None, 'dB', 'Side-Energy vs Mid'))
    rows.append(_row('Width test',         st.get('t_width'),      'info', None, 'dB', ''))
    return _table('Stereo-Analyse', rows)


def _render_loudness(lu):
    if not lu:
        return ''
    rows = [
        _row('Loudness ref',   lu.get('ref_lufs'),  'info', None, 'LUFS', 'K-weighted integrated'),
        _row('Loudness test',  lu.get('test_lufs'), 'info', None, 'LUFS', ''),
        _row('Loudness Δ',     lu.get('delta_db'),  _rate('loudness_delta', lu.get('delta_db')), None, 'dB', '|Δ|<0.5 dB gut'),
    ]
    return _table('Loudness (BS.1770 simplified)', rows)


def _render_spectral(sp):
    if not sp:
        return ''
    rows = [
        _row('Centroid ref',    sp.get('centroid_ref_hz'),     'info', None, 'Hz', 'spektrale Helligkeit'),
        _row('Centroid test',   sp.get('centroid_test_hz'),    'info', None, 'Hz', ''),
        _row('Centroid shift',  sp.get('centroid_shift_hz'),   _rate('centroid_shift_hz', sp.get('centroid_shift_hz')), None, 'Hz', '|Δ|<300 Hz gut'),
        _row('Rolloff 85% ref', sp.get('rolloff_ref_hz'),      'info', None, 'Hz', ''),
        _row('Rolloff 85% test',sp.get('rolloff_test_hz'),     'info', None, 'Hz', ''),
        _row('Rolloff shift',   sp.get('rolloff_shift_hz'),    'info', None, 'Hz', ''),
        _row('Flatness ref',    sp.get('flatness_ref'),        'info', None, '',   ''),
        _row('Flatness test',   sp.get('flatness_test'),       'info', None, '',   ''),
        _row('Flux ratio',      sp.get('flux_ratio'),          _rate('flux_ratio', sp.get('flux_ratio')), None, '', '<1 = glatter, >1 = rauer'),
    ]
    return _table('Spektrale Features', rows)


def _render_dynamic(dr):
    if not dr:
        return ''
    rows = [
        _row('Crest ref',  dr.get('crest_ref_db'),   'info', None, 'dB', 'peak-to-rms'),
        _row('Crest test', dr.get('crest_test_db'),  'info', None, 'dB', ''),
        _row('Crest Δ',    dr.get('crest_delta_db'), _rate('crest_delta_db', dr.get('crest_delta_db')), None, 'dB', 'negativ = Kompression'),
        _row('DR14 ref',   dr.get('dr14_ref'),       'info', None, 'dB', ''),
        _row('DR14 test',  dr.get('dr14_test'),      'info', None, 'dB', ''),
        _row('DR14 Δ',     dr.get('dr14_delta'),     _rate('dr14_delta', dr.get('dr14_delta')), None, 'dB', 'Dynamik-Verlust'),
    ]
    return _table('Dynamik-Bereich', rows)


def _render_nmr(nm):
    if not nm:
        return ''
    rows = [
        _row('NMR max',  nm.get('nmr_max_db'),  _rate('nmr_max_db', nm.get('nmr_max_db')), None, 'dB', '<0 = unter Masking-Schwelle'),
        _row('NMR mean', nm.get('nmr_mean_db'), 'info', None, 'dB', ''),
    ]
    out = _table('Noise-to-Mask-Ratio (Bark-Bänder)', rows,
                 note='NMR > 0 in einem Band bedeutet Quantisierungsrauschen könnte maskiert hörbar sein.')
    if nm.get('bands'):
        detail_rows = []
        for lo, hi, nmr in nm['bands']:
            rating = 'good' if nmr < 0 else ('warn' if nmr < 6 else 'fail')
            detail_rows.append(f"<tr><td>{lo}-{hi} Hz</td><td class='num {rating}'>{_fmt(nmr)} dB</td></tr>")
        out += (
            "<details><summary>Pro-Band NMR-Werte</summary>"
            "<table><thead><tr><th>Bark-Band</th><th style='text-align:right'>NMR</th></tr></thead>"
            f"<tbody>{''.join(detail_rows)}</tbody></table></details>"
        )
    return out


def _render_onsets(on):
    if not on:
        return ''
    rows = [
        _row('Ref onsets',     on.get('ref_count'),  'info', None, '', 'im Original'),
        _row('Test onsets',    on.get('test_count'), 'info', None, '', 'im encoded'),
        _row('Matched',        on.get('matched'),    'info', None, '', '±20 ms Toleranz'),
        _row('Missed',         on.get('missed'),     'info', None, '', 'Transienten verschmiert'),
        _row('Phantom',        on.get('phantom'),    'info', None, '', 'Pre-Echo/Clicks'),
        _row('Precision',      on.get('precision'),  'info', None, '', ''),
        _row('Recall',         on.get('recall'),     'info', None, '', ''),
        _row('F1',             on.get('f1'),         _rate('onset_f1', on.get('f1')), None, '', '>0.9 gut'),
    ]
    return _table('Transient-Erhaltung', rows)


def _render_thd(th):
    if not th:
        return ''
    if th.get('fundamental_hz') is None:
        return _table('THD+N', [_row('Fundamental', None, 'info', None, '', 'kein stabiler Ton gefunden')])
    rows = [
        _row('Fundamental',  th.get('fundamental_hz'),  'info', None, 'Hz', 'dominanter Ton'),
        _row('Signal-Level', th.get('signal_dbfs'),     'info', None, 'dBFS', ''),
        _row('THD+N',        th.get('thd_db'),          'info', None, 'dB', 'niedriger = sauberer'),
    ]
    return _table('THD+N (auf dominanter Frequenz)', rows)


def _overall_rating(all_ratings):
    if not all_ratings:
        return 'info'
    if any(r == 'fail' for r in all_ratings): return 'fail'
    if any(r == 'warn' for r in all_ratings): return 'warn'
    return 'good'


def _summary_cards(ours):
    de = ours.get('digital_ears', {})
    lu = ours.get('loudness', {})
    nm = ours.get('nmr', {})
    on = ours.get('onsets', {})
    sp = ours.get('spectral', {})
    cards = [
        _summary_card('snr',            'SNR',             de.get('snr'),          'dB',  'overall'),
        _summary_card('hf_corr',        'HF Env Corr',     de.get('hf_corr'),      '',    'Sony ≈ 0.928'),
        _summary_card('pe_worst',       'Pre-Echo worst',  de.get('pe_worst'),     'dB',  'niedriger = besser'),
        _summary_card('nmr_max_db',     'NMR max',         nm.get('nmr_max_db'),   'dB',  '<0 unhörbar'),
        _summary_card('loudness_delta', 'Loudness Δ',      lu.get('delta_db'),     'dB',  'K-weighted'),
        _summary_card('onset_f1',       'Onset F1',        on.get('f1'),           '',    'Transient-Erhalt'),
        _summary_card('centroid_shift_hz', 'Centroid Δ',   sp.get('centroid_shift_hz'), 'Hz', 'Helligkeit'),
    ]
    return cards


def generate(results, out_path):
    """Render the HTML report for a results-dict.

    Dict schema:
      results['label'], results['input'], results['bitrate'], results['timestamp']
      results['ours'] = { digital_ears, stereo, loudness, spectral, dynamic, nmr, onsets, thd }
      results['sony'] (optional) = same structure as 'ours'
      results['spectrogram_png'] (optional) = path to PNG
    """
    label = html.escape(str(results.get('label', '')))
    input_path = html.escape(str(results.get('input', '')))
    timestamp = html.escape(str(results.get('timestamp', datetime.now().isoformat(timespec='seconds'))))
    bitrate = html.escape(str(results.get('bitrate', 'k132')))
    ours = results.get('ours', {})
    sony = results.get('sony')
    spectro = results.get('spectrogram_png')

    # collect ratings for overall pass/fail pill
    ratings = []
    for k in ('snr', 'hf_corr', 'pe_worst', 'nmr_max_db', 'onset_f1',
              'loudness_delta', 'centroid_shift_hz'):
        v = (ours.get('digital_ears', {}).get(k)
             or ours.get('loudness', {}).get('delta_db') if k == 'loudness_delta' else None)
        if k == 'nmr_max_db':
            v = ours.get('nmr', {}).get('nmr_max_db')
        elif k == 'onset_f1':
            v = ours.get('onsets', {}).get('f1')
        elif k == 'loudness_delta':
            v = ours.get('loudness', {}).get('delta_db')
        elif k == 'centroid_shift_hz':
            v = ours.get('spectral', {}).get('centroid_shift_hz')
        else:
            v = ours.get('digital_ears', {}).get(k)
        if v is not None:
            ratings.append(_rate(k, v))
    overall = _overall_rating(ratings)
    pill = f"<span class='pill {overall}'>{overall.upper()}</span>"

    summary_html = ''.join(_summary_cards(ours))

    body_parts = [
        _render_digital_ears(ours.get('digital_ears', {}),
                             sony.get('digital_ears') if sony else None),
        _render_stereo(ours.get('stereo')),
        _render_loudness(ours.get('loudness')),
        _render_spectral(ours.get('spectral')),
        _render_dynamic(ours.get('dynamic')),
        _render_nmr(ours.get('nmr')),
        _render_onsets(ours.get('onsets')),
        _render_thd(ours.get('thd')),
    ]

    sony_section = ''
    if sony:
        sony_parts = [
            _render_digital_ears(sony.get('digital_ears', {})),
            _render_loudness(sony.get('loudness')),
            _render_spectral(sony.get('spectral')),
            _render_dynamic(sony.get('dynamic')),
            _render_nmr(sony.get('nmr')),
            _render_onsets(sony.get('onsets')),
        ]
        sony_section = (
            "<h2>Sony-Referenz (gleicher Input, psp_at3tool encode+decode)</h2>"
            + ''.join(sony_parts)
        )

    spectro_html = ''
    if spectro:
        spectro_html = (
            "<h2>Spektrogramm-Diff (erste 10 s)</h2>"
            "<p class='note'>Oben: Original · Mitte: encoded · Unten: Fehler. "
            "Vertikale Streifen im Fehler-Panel sind Frame-Burst-Artefakte.</p>"
            + _embed_png(spectro)
        )

    document = f"""<!DOCTYPE html>
<html lang="de"><head>
<meta charset="utf-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1"/>
<title>ears report — {label}</title>
<style>{_CSS}</style>
</head><body>
<div class="container">
<h1>ears.py Report {pill}</h1>
<div class="meta">
  <div><strong>Input:</strong> {input_path}</div>
  <div><strong>Label:</strong> {label} &nbsp;·&nbsp; <strong>Bitrate:</strong> {bitrate} &nbsp;·&nbsp; <strong>Timestamp:</strong> {timestamp}</div>
</div>

<h2>Zusammenfassung</h2>
<div class="summary">{summary_html}</div>

<h2>Unsere Encoder-Ausgabe (ATRAC3-rs)</h2>
{''.join(body_parts)}

{sony_section}

{spectro_html}

<footer>Generiert von ears.py · {timestamp}</footer>
</div>
</body></html>
"""
    Path(out_path).write_text(document, encoding='utf-8')
    return out_path
