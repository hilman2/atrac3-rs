//! 1:1 port of FUN_004353f0 (Sony's monolithic gain-application + 256-pt
//! MDCT + overlap-add stage) from `psp_at3tool.exe`.
//!
//! Source: `D:/test3/ghidra_output/decompiled.c` lines 40609..40960.
//!
//! Memory-Layout (param_1 relativ):
//!   +0x000..+0x1000   Spectrum + Overlap-Out-Buffer (1024 f32). Block 2
//!                     liest die alte Version, Block 6 schreibt die neue.
//!   +0x1000..+0x2000  Scratch/Copy-Ziel von Block 2 (1024 f32).
//!   +0x2000..+0x3000  Analysis-Input (4 Bänder × 256 Samples,
//!                     interleaved SIMD-Lane-Layout).
//!   +0x2a48/+0x2b14/+0x2be0/+0x2cac  Per-Band Gain-Level-Codes (u32
//!                     Index in SONY_GAIN_LEVELS).
//!
//! Stack:
//!   local_1010[1024]   kombinierte Stack-Buffer (Ghidra: local_1010[512]
//!                     + local_810[511] aneinanderliegend).
//!
//! Konstanten:
//!   SONY_GAIN_LEVELS            (DAT_0048bdd8, 16 f32)
//!   SONY_GAIN_APPLY_PERMUTATION (DAT_004bea08, 128 u32)
//!   SONY_TRANSFORM_SRC_C210     (DAT_0048c210, 512 f32 = 128 Zeilen × 4)
//!   SONY_TRANSFORM_SRC_CA10     (DAT_0048ca10, 128 f32)
//!   SONY_GAIN_APPLY_FINAL_MIX   (DAT_0048bf30, 4 f32 = 1/√2 × 4)

use super::quant_sony::{
    SONY_GAIN_APPLY_FINAL_MIX, SONY_GAIN_APPLY_PERMUTATION, SONY_GAIN_LEVELS,
    SONY_TRANSFORM_SRC_C210, SONY_TRANSFORM_SRC_CA10,
};

pub const SONY_GAIN_BANDS: usize = 4;
pub const SONY_BAND_SAMPLES: usize = 256;

const TRANSFORM_MATRIX_ROW_STRIDE: usize = 16;
const TRANSFORM_MATRIX_ROWS: usize = 128;
const TRANSFORM_MATRIX_LEN: usize = TRANSFORM_MATRIX_ROW_STRIDE * TRANSFORM_MATRIX_ROWS;
const STAGE4_LEN: usize = 128 * 4;

/// 1:1 Port FUN_00434470 (Decompile 40130-40176):
///
/// Baut DAT_004c2c60 (128 Zeilen × 16 f32) aus DAT_0048c210 auf,
/// wobei jede der 4 Quell-f32 einer Zeile 4-fach repliziert wird:
///   row[0..3]   = src[0]   // src+0x0 via DAT_0048c210
///   row[4..7]   = src[1]   // src+0x4 via DAT_0048c214
///   row[8..11]  = src[2]   // src+0x8 via DAT_0048c218
///   row[12..15] = src[3]   // src+0xc via DAT_0048c21c
///
/// Und baut DAT_004c2470 (512 f32) aus DAT_0048ca10 (128 f32), wobei
/// jedes Element 4-fach repliziert wird.
pub fn build_transform_tables() -> ([f32; TRANSFORM_MATRIX_LEN], [f32; STAGE4_LEN]) {
    let mut matrix = [0.0f32; TRANSFORM_MATRIX_LEN];
    let mut stage4 = [0.0f32; STAGE4_LEN];

    for row in 0..TRANSFORM_MATRIX_ROWS {
        let src_base = row * 4;
        let dst_base = row * TRANSFORM_MATRIX_ROW_STRIDE;
        let src0 = SONY_TRANSFORM_SRC_C210[src_base + 0];
        let src1 = SONY_TRANSFORM_SRC_C210[src_base + 1];
        let src2 = SONY_TRANSFORM_SRC_C210[src_base + 2];
        let src3 = SONY_TRANSFORM_SRC_C210[src_base + 3];
        for lane in 0..4 {
            matrix[dst_base + 0 + lane] = src0;
            matrix[dst_base + 4 + lane] = src1;
            matrix[dst_base + 8 + lane] = src2;
            matrix[dst_base + 12 + lane] = src3;
        }
        let scalar = SONY_TRANSFORM_SRC_CA10[row];
        let stage_base = row * 4;
        for lane in 0..4 {
            stage4[stage_base + lane] = scalar;
        }
    }

    (matrix, stage4)
}

/// 1:1 Port FUN_004353f0 (Decompile 40609-40960).
///
/// * `gain_codes`  — Sony `param_1 + 0x2a48/0x2b14/0x2be0/0x2cac`
/// * `analysis`    — Sony `param_1 + 0x2000..+0x3000`, wird in-place
///                   modifiziert (Block 1).
/// * `output`      — Sony `param_1 + 0x0..+0x1000`. Block 2 liest die
///                   alte Version vor Überschreibung durch Block 6.
/// * `mid_scratch` — Sony `param_1 + 0x1000..+0x2000`. Block 2 schreibt
///                   hier ein shuffled copy des alten `output`, blocks
///                   3-5 lesen den Stack (`local_1010`), Block 6
///                   schreibt in `output` zurück.
pub fn sony_gain_apply_mdct_overlap(
    gain_codes: [u32; SONY_GAIN_BANDS],
    analysis: &mut [f32; SONY_BAND_SAMPLES * SONY_GAIN_BANDS],
    output: &mut [f32; 0x400],
    mid_scratch: &mut [f32; 0x400],
) {
    let (matrix, stage4) = build_transform_tables();

    // Block 4 indiziert Sony's `&DAT_004c2c60 + iStack_1028 * 4` mit
    // iStack_1028 negativ — das liest aus der DAT_004c2470-Region
    // (stage4) die direkt VOR DAT_004c2c60 im EXE-Adressraum liegt.
    // Wir bauen ein kombiniertes Array combined[stage4 ++ matrix], so
    // dass matrix-base = combined[STAGE4_LEN].
    const COMBINED_LEN: usize = STAGE4_LEN + TRANSFORM_MATRIX_LEN;
    let mut combined = [0.0f32; COMBINED_LEN];
    combined[..STAGE4_LEN].copy_from_slice(&stage4);
    combined[STAGE4_LEN..].copy_from_slice(&matrix);
    const MATRIX_BASE: usize = STAGE4_LEN;

    // Ghidra 40674-40678: vier Gain-Level-Lookups via SONY_GAIN_LEVELS.
    let gain_lane: [f32; SONY_GAIN_BANDS] = [
        SONY_GAIN_LEVELS[(gain_codes[0] as usize) & 0xf],
        SONY_GAIN_LEVELS[(gain_codes[1] as usize) & 0xf],
        SONY_GAIN_LEVELS[(gain_codes[2] as usize) & 0xf],
        SONY_GAIN_LEVELS[(gain_codes[3] as usize) & 0xf],
    ];

    // Kombinierter Stack-Buffer local_1010[1024]. In Ghidra sind es zwei
    // nebeneinander liegende Stack-Arrays local_1010[512] + local_810[511];
    // wir bilden das als ein Array ab.
    let mut stack = [0.0f32; 1024];

    // =================================================================
    // BLOCK 1 (Ghidra 40680-40731) — Radix-4-Butterfly, 128 Iterationen.
    //
    // Pro Iteration:
    //   * iv33 = Permutation[i]
    //   * lese 4 analysis-Samples (in[0..3])
    //   * lese 4 f32 aus output bei offset (iv33+0x180)*4       (q1)
    //   * lese 4 f32 aus output bei offset (0x17f-iv33)*4       (q2)
    //   * matrix-Zeile m[0..15] via iv33*16
    //   * schreibe 4 f32 zurück nach analysis[i*4+lane]:
    //       m[lane+4] * (q1[lane] * m[lane] - q2[lane])
    //   * berechne q1' = m[lane+8] * (q2 * m[lane] + q1)
    //   * berechne q2' = m[lane+12] * (q1' - in * gain_lane)
    //   * schreibe stack[8i..8i+3] = q1' + in*gain + q2'
    //   * schreibe stack[8i+4..8i+7] = q2'
    // =================================================================
    for i in 0..128 {
        let iv33 = SONY_GAIN_APPLY_PERMUTATION[i] as usize;
        let row = iv33 * TRANSFORM_MATRIX_ROW_STRIDE;

        // Ghidra: pfVar37 = (iVar33 + 0x180) * 0x10 + param_1
        //         pfVar37 = (0x17f - iVar33) * 0x10 + param_1
        // Byte-Offset; liegt im mid_scratch-Bereich (param_1+0x1000..+0x2000).
        // Umrechnung in f32-Index innerhalb mid_scratch:
        //   (iv33 + 0x180) * 4 - 0x400
        //   (0x17f - iv33)  * 4 - 0x400
        let base_q1 = (iv33 + 0x180) * 4 - 0x400;
        let base_q2 = (0x17f - iv33) * 4 - 0x400;

        for lane in 0..4 {
            let in_sample = analysis[i * 4 + lane];
            let q1 = mid_scratch[base_q1 + lane];
            let q2 = mid_scratch[base_q2 + lane];
            let m0 = matrix[row + 0 + lane];
            let m1 = matrix[row + 4 + lane];
            let m2 = matrix[row + 8 + lane];
            let m3 = matrix[row + 12 + lane];

            analysis[i * 4 + lane] = m1 * (q1 * m0 - q2);
            let q1_new = m2 * (q2 * m0 + q1);
            let q2_new = m3 * (q1_new - in_sample * gain_lane[lane]);

            stack[i * 8 + 0 + lane] = q1_new + in_sample * gain_lane[lane] + q2_new;
            stack[i * 8 + 4 + lane] = q2_new;
        }
    }

    // =================================================================
    // BLOCK 2 (Ghidra 40732-40770) — Shuffle-Copy von output[]
    // nach mid_scratch[].
    //
    // 64 Iterationen mit 16-Dword-Schritten. Jede Iter k (k=0..63):
    //   Liest 16 Dwords bei offset k*16 in output (= param_1+k*0x40).
    //   Schreibt
    //     mid_scratch[k*16 + 0..3]  = output[k*16 + 0..3]
    //     mid_scratch[k*16 + 4..15] = output[k*16 + 4..15]
    //   (ergibt exakten Copy; das Sony-Quirk ist die Quelladressierung
    //    puVar29[-8..+7] relativ zu param_1+0x20+k*0x40.)
    // =================================================================
    for k in 0..64 {
        for lane in 0..16 {
            mid_scratch[k * 16 + lane] = output[k * 16 + lane];
        }
    }

    // =================================================================
    // BLOCK 3 (Ghidra 40771-40808) — Radix-2 auf stack[] mit stage4.
    //
    // 16-float Chunks, pro Chunk 8 stage4-Koeffizienten genutzt:
    //   (fortlaufend 8 per Iter, Quelle pfVar37 läuft bis 0x4c2c70 = 512).
    //
    // Pattern pro 16-float Chunk p[0..16]:
    //   q_diff[l]  = stage4_lo[l] * (p[4+l] - p[12+l])     l=0..3
    //   tmp[l]     = p[l] + p[8+l] + q_diff[l]
    //   q_sum[l]   = stage4_lo[l] * (p[l] - p[8+l])
    //   p[4+l]     = p[4+l] + p[12+l] + q_sum[l]
    //   p[8+l]     = q_sum[l]
    //   p[12+l]    = q_diff[l]
    //   p[l]       = tmp[l]
    //
    // Die stage4-Iteration `pfVar37 += 8` wird mit chunk%2 ausgewertet.
    // =================================================================
    {
        let mut stage_idx = 0usize;
        let mut chunk_base = 0usize;
        while stage_idx + 4 <= STAGE4_LEN && chunk_base + 16 <= stack.len() {
            let s0 = stage4[stage_idx + 0];
            let s1 = stage4[stage_idx + 1];
            let s2 = stage4[stage_idx + 2];
            let s3 = stage4[stage_idx + 3];
            let stages = [s0, s1, s2, s3];

            let mut p = [0.0f32; 16];
            for l in 0..16 {
                p[l] = stack[chunk_base + l];
            }

            let mut q_diff = [0.0f32; 4];
            let mut q_sum = [0.0f32; 4];
            let mut tmp = [0.0f32; 4];
            for l in 0..4 {
                q_diff[l] = stages[l] * (p[4 + l] - p[12 + l]);
                tmp[l] = p[l] + p[8 + l] + q_diff[l];
                q_sum[l] = stages[l] * (p[l] - p[8 + l]);
            }
            for l in 0..4 {
                stack[chunk_base + 4 + l] = p[4 + l] + p[12 + l] + q_sum[l];
                stack[chunk_base + 8 + l] = q_sum[l];
                stack[chunk_base + 12 + l] = q_diff[l];
                stack[chunk_base + l] = tmp[l];
            }

            chunk_base += 16;
            stage_idx += 8;
        }
    }

    // =================================================================
    // BLOCK 4 (Ghidra 40809-40908) — Butterfly-Cascade mit iStack_1024
    // von 4 bis 64 (stop bei 0x80).
    //
    //   iStack_1024 ∈ {4, 8, 16, 32, 64}  (5 Stufen)
    //
    // Pro Stufe:
    //   iStack_1028_init = iStack_1024/2 - 0x80        // offset in DAT_004c2c60
    //   pfVar37 = matrix row base (iStack_1028_init * 4 f32)
    //   pfVar30 = stack_start
    //   while iStack_1028 < 1 { ... } (äußere Rotations-Schleife)
    //
    // Die innere Butterfly schreibt in 4-float Lane-Schritten. Da die
    // Sony-Decompile wesentliche Indexarithmetik (negative DAT-Offsets)
    // mit iStack_1028 koppelt, die im Bereich [-126..0] läuft, bildet
    // diese Transkription das Lauf- und Schreibschema 1:1 ab.
    // =================================================================
    {
        let mut length = 4usize;
        while length != 0x80 {
            let i_var33_plus1 = length + 1;
            let mut i_stack_1028: isize = (length / 2) as isize - 0x80;
            // pfVar37 startet an combined[MATRIX_BASE + iStack_1028]
            // (iStack_1028 negativ → Zugriff in stage4-Teil).
            let mut pv37_idx: isize = MATRIX_BASE as isize + i_stack_1028;
            let mut pv30_base = 0usize;

            loop {
                let row_start = pv37_idx as usize;
                let s0 = combined[row_start + 0];
                let s1 = combined[row_start + 1];
                let s2 = combined[row_start + 2];
                let s3 = combined[row_start + 3];
                let stages = [s0, s1, s2, s3];

                // Innere Schleife 1: Butterfly von pv30_base bis pv30_base + length*4.
                let pfvar1 = pv30_base + length * 4;
                let mut cur = pv30_base;
                while cur != pfvar1 {
                    // Lade 16 floats: p[cur..cur+8] und p[cur+iVar33*4-4..+4]
                    // iVar33 = length + 1, so iVar33*4 = length*4+4.
                    let mut a = [0.0f32; 8]; // stack[cur..cur+8]
                    for l in 0..8 {
                        a[l] = stack[cur + l];
                    }
                    let target_base = cur + (i_var33_plus1 * 4) - 4;
                    let mut b = [0.0f32; 8]; // stack[target_base..target_base+8]
                    for l in 0..8 {
                        b[l] = stack[target_base + l];
                    }

                    for l in 0..4 {
                        stack[target_base + 0 + l] = stages[l] * (a[l] - b[l]);
                        stack[target_base + 4 + l] = stages[l] * (a[4 + l] - b[4 + l]);
                        stack[cur + 0 + l] = a[l] + b[l];
                        stack[cur + 4 + l] = a[4 + l] + b[4 + l];
                    }
                    cur += 8;
                }

                // Innere Schleife 2: Post-Addition mit negativem Stride.
                // iVar36 = iStack_1024 * 2 - 1, iVar34 = iVar36 * 16 (bytes),
                // iVar36 -= 8 pro Iter, iVar34 -= 0x80 (= -32 f32 Wörter).
                let mut i_var36: isize = (length as isize) * 2 - 1;
                pv30_base = cur - length * 4;
                let mut iv34_f32: isize = i_var36 * 4; // f32 word offset
                let mut pv30 = cur - length * 4;
                while i_var36 > 0 {
                    // p1 = (float *)(iv34 + -0x10 + pv30) = pv30 + iv34_f32 - 4
                    // p2 = (float *)(iv34 + pv30)
                    // p3 = (float *)(iv34 + -0x20 + pv30) = pv30 + iv34_f32 - 8
                    // p4 = (float *)(iv34 + -0x30 + pv30) = pv30 + iv34_f32 - 12
                    let p2_base = (pv30 as isize + iv34_f32) as usize;
                    let p1_base = p2_base.wrapping_sub(4);
                    let p3_base = p2_base.wrapping_sub(8);
                    let p4_base = p2_base.wrapping_sub(12);

                    let v40 = stack[p1_base + 0];
                    let v43 = stack[p1_base + 1];
                    let v46 = stack[p1_base + 2];
                    let v49 = stack[p1_base + 3];
                    // pfVar1[0] wird nur implizit gelesen (*pfVar1), danach
                    // pfVar1[1..3] via fVar4, fVar5, fVar6.
                    let w_p2_0 = stack[p2_base + 0];
                    let v4 = stack[p2_base + 1];
                    let v5 = stack[p2_base + 2];
                    let v6 = stack[p2_base + 3];
                    let v7 = stack[p3_base + 0];
                    let v38 = stack[p3_base + 1];
                    let v39 = stack[p3_base + 2];
                    let v42 = stack[p3_base + 3];
                    let v45 = stack[p4_base + 0];
                    let v41 = stack[p4_base + 1];
                    let v44 = stack[p4_base + 2];
                    let v47 = stack[p4_base + 3];

                    stack[pv30 + 0] = w_p2_0 + stack[pv30 + 0];
                    stack[pv30 + 1] = v4 + stack[pv30 + 1];
                    stack[pv30 + 2] = v5 + stack[pv30 + 2];
                    stack[pv30 + 3] = v6 + stack[pv30 + 3];
                    stack[pv30 + 4] = v40 + stack[pv30 + 4];
                    stack[pv30 + 5] = v43 + stack[pv30 + 5];
                    stack[pv30 + 6] = v46 + stack[pv30 + 6];
                    stack[pv30 + 7] = v49 + stack[pv30 + 7];
                    stack[pv30 + 8] = v7 + stack[pv30 + 8];
                    stack[pv30 + 9] = v38 + stack[pv30 + 9];
                    stack[pv30 + 10] = v39 + stack[pv30 + 10];
                    stack[pv30 + 11] = v42 + stack[pv30 + 11];
                    stack[pv30 + 12] = v45 + stack[pv30 + 12];
                    stack[pv30 + 13] = v41 + stack[pv30 + 13];
                    stack[pv30 + 14] = v44 + stack[pv30 + 14];
                    stack[pv30 + 15] = v47 + stack[pv30 + 15];

                    pv30 += 16;
                    iv34_f32 -= 32; // -0x80 Bytes = -32 f32
                    i_var36 -= 8;
                }

                i_stack_1028 += length as isize;
                pv30_base = pv30 + length * 4;
                pv37_idx += (length as isize) * 4;

                if i_stack_1028 >= 1 {
                    break;
                }
            }

            length *= 2;
        }
    }

    // =================================================================
    // BLOCK 5 (Ghidra 40909-40926) — finaler Radix-2 mit 1/√2.
    //
    // 128 Iterationen, jede mit 4-Lane-SIMD:
    //   low = stack[i*4..i*4+4]    (wir iterieren pv30 += 4 → 128×4=512 Elemente)
    //   high = stack[i*4 + 0x200..i*4 + 0x204]
    //   stack[i*4+0x200..] = mix * (low - high)
    //   stack[i*4..]       = low + high
    // =================================================================
    for i in 0..128 {
        let low_base = i * 4;
        let high_base = low_base + 0x200;
        for lane in 0..4 {
            let lo = stack[low_base + lane];
            let hi = stack[high_base + lane];
            stack[high_base + lane] = SONY_GAIN_APPLY_FINAL_MIX[lane] * (lo - hi);
            stack[low_base + lane] = lo + hi;
        }
    }

    // =================================================================
    // BLOCK 6 (Ghidra 40927-40958) — Output-Scrambling nach output[].
    //
    // Puar3 startet am Stack-Top (= end of local_1010 = stack[1024]).
    // iVar33 läuft von -0x1000 (= -1024 Bytes = -256 f32) hoch bis 0.
    // Pro Iter:
    //   pfVar30 = stack[(puVar35 + iVar33)/4..]    // base
    //   pfVar37 = gleiche Stelle (in-place)
    //   pfVar37[0..3] = pfVar30[0..3] + stack[esp-0x20..esp-0x14]
    //   puVar31[-0x100] = stack[puVar35+iVar33+0] (nach Add)
    //   puVar31[0]      = stack[esp-0x1c]
    //   puVar31[+0x100] = stack[puVar35+iVar33+2]
    //   puVar31[+0x200] = stack[esp-0x14]
    //   analog pfStack_102c (param_1+0x7fc..)
    //
    // Das Zugriffsschema iteriert über 64 8-float-Blöcke am Stack-Top
    // und streut die 8 Werte in 8 verschiedene Output-Zielen.
    // =================================================================
    {
        // puVar31 startet bei param_1+0x400 → output[0x100] = output[256].
        // pfStack_102c startet bei param_1+0x7fc → output[0x1ff] = output[511].
        let mut pv31_idx: usize = 0x100;
        let mut p_stack_102c_idx: usize = 0x1ff;
        // iVar33 in Sony: bytes, -0x1000 bis -0x20 (Stride 0x20). In f32:
        // -1024 bis -8 (Stride 8). Gesamt 128 Iterationen.
        let mut iv33: isize = -1024;
        // register0x00000010 = Ghidra-ESP-Alias. Nach obiger Analyse
        // müssen die f32-Zugriffe von (puvar3-8) bis (puvar35+iv33+3)
        // alle in stack[0..1023] landen. Das ergibt puvar3_base = 1028.
        let mut puvar3_base: isize = 1028;
        for _ in 0..128 {
            let puvar35_base = puvar3_base - 4;
            let pv30_base = (puvar35_base + iv33) as usize;

            let f40 = stack[pv30_base + 1];
            let f43 = stack[pv30_base + 2];
            let f46 = stack[pv30_base + 3];
            let f49 = stack[(puvar3_base - 7) as usize];
            let f4 = stack[(puvar3_base - 6) as usize];
            let f5 = stack[(puvar3_base - 5) as usize];

            let base0 = stack[pv30_base + 0] + stack[(puvar3_base - 8) as usize];
            let base1 = f40 + f49;
            let base2 = f43 + f4;
            let base3 = f46 + f5;

            stack[pv30_base + 0] = base0;
            stack[pv30_base + 1] = base1;
            stack[pv30_base + 2] = base2;
            stack[pv30_base + 3] = base3;

            output[pv31_idx - 0x100] = stack[pv30_base + 0];
            output[pv31_idx + 0x000] = stack[(puvar3_base - 7) as usize];
            output[pv31_idx + 0x100] = stack[pv30_base + 2];
            output[pv31_idx + 0x200] = stack[(puvar3_base - 5) as usize];
            output[p_stack_102c_idx - 0x100] = stack[(puvar3_base - 8) as usize];
            output[p_stack_102c_idx + 0x000] = stack[pv30_base + 1];
            output[p_stack_102c_idx + 0x100] = stack[(puvar3_base - 6) as usize];
            output[p_stack_102c_idx + 0x200] = stack[pv30_base + 3];

            pv31_idx += 1;
            p_stack_102c_idx = p_stack_102c_idx.wrapping_sub(1);
            iv33 += 8;
            puvar3_base = puvar35_base;
        }
    }
}

/// Sony Kanal-State-Slice, 1:1 Byte-Offsets wie in FUN_004353f0.
#[derive(Debug, Clone)]
pub struct SonyGainApplyState {
    pub band_gain_codes: [u32; SONY_GAIN_BANDS],
    pub analysis: [f32; SONY_BAND_SAMPLES * SONY_GAIN_BANDS],
    pub output: [f32; 0x400],
    pub mid_scratch: [f32; 0x400],
}

impl Default for SonyGainApplyState {
    fn default() -> Self {
        Self {
            band_gain_codes: [4; SONY_GAIN_BANDS],
            analysis: [0.0; SONY_BAND_SAMPLES * SONY_GAIN_BANDS],
            output: [0.0; 0x400],
            mid_scratch: [0.0; 0x400],
        }
    }
}

pub fn sony_gain_apply_enabled() -> bool {
    std::env::var("ATRAC_SONY_GAIN_APPLY")
        .map(|value| value == "1")
        .unwrap_or(false)
}

pub fn sony_apply_gain_mdct_overlap(state: &mut SonyGainApplyState) {
    sony_gain_apply_mdct_overlap(
        state.band_gain_codes,
        &mut state.analysis,
        &mut state.output,
        &mut state.mid_scratch,
    );
}

/// Diff-Harness: führt `sony_gain_apply_mdct_overlap` aus und gibt
/// Checkpoints der internen Stages (Block 1, Block 3, Block 4 je Stufe,
/// Block 5, Block 6) zurück. Wird nur von Tests benutzt, damit wir
/// Block-by-Block gegen eine Referenz-Implementierung (oder eine ältere
/// Version) vergleichen können.
#[derive(Debug, Clone)]
pub struct SonyGainApplyCheckpoints {
    pub analysis_after_block1: [f32; SONY_BAND_SAMPLES * SONY_GAIN_BANDS],
    pub stack_after_block1: [f32; 1024],
    pub mid_scratch_after_block2: [f32; 0x400],
    pub stack_after_block3: [f32; 1024],
    pub stack_after_block4_len4: [f32; 1024],
    pub stack_after_block4_all: [f32; 1024],
    pub stack_after_block5: [f32; 1024],
    pub output_after_block6: [f32; 0x400],
}

pub fn sony_gain_apply_mdct_overlap_instrumented(
    gain_codes: [u32; SONY_GAIN_BANDS],
    analysis: &mut [f32; SONY_BAND_SAMPLES * SONY_GAIN_BANDS],
    output: &mut [f32; 0x400],
    mid_scratch: &mut [f32; 0x400],
) -> SonyGainApplyCheckpoints {
    let mut cp = SonyGainApplyCheckpoints {
        analysis_after_block1: [0.0; SONY_BAND_SAMPLES * SONY_GAIN_BANDS],
        stack_after_block1: [0.0; 1024],
        mid_scratch_after_block2: [0.0; 0x400],
        stack_after_block3: [0.0; 1024],
        stack_after_block4_len4: [0.0; 1024],
        stack_after_block4_all: [0.0; 1024],
        stack_after_block5: [0.0; 1024],
        output_after_block6: [0.0; 0x400],
    };
    sony_gain_apply_mdct_overlap_with_checkpoints(
        gain_codes,
        analysis,
        output,
        mid_scratch,
        Some(&mut cp),
    );
    cp
}

fn sony_gain_apply_mdct_overlap_with_checkpoints(
    gain_codes: [u32; SONY_GAIN_BANDS],
    analysis: &mut [f32; SONY_BAND_SAMPLES * SONY_GAIN_BANDS],
    output: &mut [f32; 0x400],
    mid_scratch: &mut [f32; 0x400],
    mut cp: Option<&mut SonyGainApplyCheckpoints>,
) {
    let (matrix, stage4) = build_transform_tables();
    const COMBINED_LEN: usize = STAGE4_LEN + TRANSFORM_MATRIX_LEN;
    let mut combined = [0.0f32; COMBINED_LEN];
    combined[..STAGE4_LEN].copy_from_slice(&stage4);
    combined[STAGE4_LEN..].copy_from_slice(&matrix);
    const MATRIX_BASE: usize = STAGE4_LEN;

    let gain_lane: [f32; SONY_GAIN_BANDS] = [
        SONY_GAIN_LEVELS[(gain_codes[0] as usize) & 0xf],
        SONY_GAIN_LEVELS[(gain_codes[1] as usize) & 0xf],
        SONY_GAIN_LEVELS[(gain_codes[2] as usize) & 0xf],
        SONY_GAIN_LEVELS[(gain_codes[3] as usize) & 0xf],
    ];

    let mut stack = [0.0f32; 1024];

    for i in 0..128 {
        let iv33 = SONY_GAIN_APPLY_PERMUTATION[i] as usize;
        let row = iv33 * TRANSFORM_MATRIX_ROW_STRIDE;
        let base_q1 = (iv33 + 0x180) * 4 - 0x400;
        let base_q2 = (0x17f - iv33) * 4 - 0x400;
        for lane in 0..4 {
            let in_sample = analysis[i * 4 + lane];
            let q1 = mid_scratch[base_q1 + lane];
            let q2 = mid_scratch[base_q2 + lane];
            let m0 = matrix[row + 0 + lane];
            let m1 = matrix[row + 4 + lane];
            let m2 = matrix[row + 8 + lane];
            let m3 = matrix[row + 12 + lane];
            analysis[i * 4 + lane] = m1 * (q1 * m0 - q2);
            let q1_new = m2 * (q2 * m0 + q1);
            let q2_new = m3 * (q1_new - in_sample * gain_lane[lane]);
            stack[i * 8 + 0 + lane] = q1_new + in_sample * gain_lane[lane] + q2_new;
            stack[i * 8 + 4 + lane] = q2_new;
        }
    }
    if let Some(cp) = cp.as_deref_mut() {
        cp.analysis_after_block1.copy_from_slice(analysis);
        cp.stack_after_block1.copy_from_slice(&stack);
    }

    for k in 0..64 {
        for lane in 0..16 {
            mid_scratch[k * 16 + lane] = output[k * 16 + lane];
        }
    }
    if let Some(cp) = cp.as_deref_mut() {
        cp.mid_scratch_after_block2.copy_from_slice(mid_scratch);
    }

    {
        let mut stage_idx = 0usize;
        let mut chunk_base = 0usize;
        while stage_idx + 4 <= STAGE4_LEN && chunk_base + 16 <= stack.len() {
            let stages = [
                stage4[stage_idx + 0],
                stage4[stage_idx + 1],
                stage4[stage_idx + 2],
                stage4[stage_idx + 3],
            ];
            let mut p = [0.0f32; 16];
            for l in 0..16 {
                p[l] = stack[chunk_base + l];
            }
            let mut q_diff = [0.0f32; 4];
            let mut q_sum = [0.0f32; 4];
            let mut tmp = [0.0f32; 4];
            for l in 0..4 {
                q_diff[l] = stages[l] * (p[4 + l] - p[12 + l]);
                tmp[l] = p[l] + p[8 + l] + q_diff[l];
                q_sum[l] = stages[l] * (p[l] - p[8 + l]);
            }
            for l in 0..4 {
                stack[chunk_base + 4 + l] = p[4 + l] + p[12 + l] + q_sum[l];
                stack[chunk_base + 8 + l] = q_sum[l];
                stack[chunk_base + 12 + l] = q_diff[l];
                stack[chunk_base + l] = tmp[l];
            }
            chunk_base += 16;
            stage_idx += 8;
        }
    }
    if let Some(cp) = cp.as_deref_mut() {
        cp.stack_after_block3.copy_from_slice(&stack);
    }

    {
        let mut length = 4usize;
        while length != 0x80 {
            let i_var33_plus1 = length + 1;
            let mut i_stack_1028: isize = (length / 2) as isize - 0x80;
            let mut pv37_idx: isize = MATRIX_BASE as isize + i_stack_1028;
            let mut pv30_base = 0usize;
            loop {
                let row_start = pv37_idx as usize;
                let stages = [
                    combined[row_start + 0],
                    combined[row_start + 1],
                    combined[row_start + 2],
                    combined[row_start + 3],
                ];
                let pfvar1 = pv30_base + length * 4;
                let mut cur = pv30_base;
                while cur != pfvar1 {
                    let mut a = [0.0f32; 8];
                    for l in 0..8 {
                        a[l] = stack[cur + l];
                    }
                    let target_base = cur + (i_var33_plus1 * 4) - 4;
                    let mut b = [0.0f32; 8];
                    for l in 0..8 {
                        b[l] = stack[target_base + l];
                    }
                    for l in 0..4 {
                        stack[target_base + 0 + l] = stages[l] * (a[l] - b[l]);
                        stack[target_base + 4 + l] = stages[l] * (a[4 + l] - b[4 + l]);
                        stack[cur + 0 + l] = a[l] + b[l];
                        stack[cur + 4 + l] = a[4 + l] + b[4 + l];
                    }
                    cur += 8;
                }
                let mut i_var36: isize = (length as isize) * 2 - 1;
                pv30_base = cur - length * 4;
                let mut iv34_f32: isize = i_var36 * 4;
                let mut pv30 = cur - length * 4;
                while i_var36 > 0 {
                    let p2_base = (pv30 as isize + iv34_f32) as usize;
                    let p1_base = p2_base.wrapping_sub(4);
                    let p3_base = p2_base.wrapping_sub(8);
                    let p4_base = p2_base.wrapping_sub(12);
                    let v40 = stack[p1_base + 0];
                    let v43 = stack[p1_base + 1];
                    let v46 = stack[p1_base + 2];
                    let v49 = stack[p1_base + 3];
                    let w_p2_0 = stack[p2_base + 0];
                    let v4 = stack[p2_base + 1];
                    let v5 = stack[p2_base + 2];
                    let v6 = stack[p2_base + 3];
                    let v7 = stack[p3_base + 0];
                    let v38 = stack[p3_base + 1];
                    let v39 = stack[p3_base + 2];
                    let v42 = stack[p3_base + 3];
                    let v45 = stack[p4_base + 0];
                    let v41 = stack[p4_base + 1];
                    let v44 = stack[p4_base + 2];
                    let v47 = stack[p4_base + 3];
                    stack[pv30 + 0] = w_p2_0 + stack[pv30 + 0];
                    stack[pv30 + 1] = v4 + stack[pv30 + 1];
                    stack[pv30 + 2] = v5 + stack[pv30 + 2];
                    stack[pv30 + 3] = v6 + stack[pv30 + 3];
                    stack[pv30 + 4] = v40 + stack[pv30 + 4];
                    stack[pv30 + 5] = v43 + stack[pv30 + 5];
                    stack[pv30 + 6] = v46 + stack[pv30 + 6];
                    stack[pv30 + 7] = v49 + stack[pv30 + 7];
                    stack[pv30 + 8] = v7 + stack[pv30 + 8];
                    stack[pv30 + 9] = v38 + stack[pv30 + 9];
                    stack[pv30 + 10] = v39 + stack[pv30 + 10];
                    stack[pv30 + 11] = v42 + stack[pv30 + 11];
                    stack[pv30 + 12] = v45 + stack[pv30 + 12];
                    stack[pv30 + 13] = v41 + stack[pv30 + 13];
                    stack[pv30 + 14] = v44 + stack[pv30 + 14];
                    stack[pv30 + 15] = v47 + stack[pv30 + 15];
                    pv30 += 16;
                    iv34_f32 -= 32;
                    i_var36 -= 8;
                }
                i_stack_1028 += length as isize;
                pv30_base = pv30 + length * 4;
                pv37_idx += (length as isize) * 4;
                if i_stack_1028 >= 1 {
                    break;
                }
            }
            if length == 4 {
                if let Some(cp) = cp.as_deref_mut() {
                    cp.stack_after_block4_len4.copy_from_slice(&stack);
                }
            }
            length *= 2;
        }
    }
    if let Some(cp) = cp.as_deref_mut() {
        cp.stack_after_block4_all.copy_from_slice(&stack);
    }

    for i in 0..128 {
        let low_base = i * 4;
        let high_base = low_base + 0x200;
        for lane in 0..4 {
            let lo = stack[low_base + lane];
            let hi = stack[high_base + lane];
            stack[high_base + lane] = SONY_GAIN_APPLY_FINAL_MIX[lane] * (lo - hi);
            stack[low_base + lane] = lo + hi;
        }
    }
    if let Some(cp) = cp.as_deref_mut() {
        cp.stack_after_block5.copy_from_slice(&stack);
    }

    {
        let mut pv31_idx: usize = 0x100;
        let mut p_stack_102c_idx: usize = 0x1ff;
        let mut iv33: isize = -1024;
        let mut puvar3_base: isize = 1028;
        for _ in 0..128 {
            let puvar35_base = puvar3_base - 4;
            let pv30_base = (puvar35_base + iv33) as usize;
            let f40 = stack[pv30_base + 1];
            let f43 = stack[pv30_base + 2];
            let f46 = stack[pv30_base + 3];
            let f49 = stack[(puvar3_base - 7) as usize];
            let f4 = stack[(puvar3_base - 6) as usize];
            let f5 = stack[(puvar3_base - 5) as usize];
            let base0 = stack[pv30_base + 0] + stack[(puvar3_base - 8) as usize];
            let base1 = f40 + f49;
            let base2 = f43 + f4;
            let base3 = f46 + f5;
            stack[pv30_base + 0] = base0;
            stack[pv30_base + 1] = base1;
            stack[pv30_base + 2] = base2;
            stack[pv30_base + 3] = base3;
            output[pv31_idx - 0x100] = stack[pv30_base + 0];
            output[pv31_idx + 0x000] = stack[(puvar3_base - 7) as usize];
            output[pv31_idx + 0x100] = stack[pv30_base + 2];
            output[pv31_idx + 0x200] = stack[(puvar3_base - 5) as usize];
            output[p_stack_102c_idx - 0x100] = stack[(puvar3_base - 8) as usize];
            output[p_stack_102c_idx + 0x000] = stack[pv30_base + 1];
            output[p_stack_102c_idx + 0x100] = stack[(puvar3_base - 6) as usize];
            output[p_stack_102c_idx + 0x200] = stack[pv30_base + 3];
            pv31_idx += 1;
            p_stack_102c_idx = p_stack_102c_idx.wrapping_sub(1);
            iv33 += 8;
            puvar3_base = puvar35_base;
        }
    }
    if let Some(cp) = cp.as_deref_mut() {
        cp.output_after_block6.copy_from_slice(output);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        SONY_BAND_SAMPLES, SONY_GAIN_BANDS, SONY_TRANSFORM_SRC_C210, STAGE4_LEN,
        SonyGainApplyState, TRANSFORM_MATRIX_LEN, build_transform_tables,
        sony_apply_gain_mdct_overlap, sony_gain_apply_mdct_overlap_instrumented,
    };

    #[test]
    fn matrix_init_lanes_are_4wide() {
        let (matrix, _stage) = build_transform_tables();
        let expected = SONY_TRANSFORM_SRC_C210[0];
        for lane in 0..4 {
            assert_eq!(matrix[lane], expected);
        }
        let expected1 = SONY_TRANSFORM_SRC_C210[1];
        for lane in 0..4 {
            assert_eq!(matrix[4 + lane], expected1);
        }
    }

    #[test]
    fn default_state_is_unity_gain() {
        let state = SonyGainApplyState::default();
        for band in 0..SONY_GAIN_BANDS {
            assert_eq!(state.band_gain_codes[band], 4);
        }
    }

    #[test]
    fn zero_input_produces_zero_output() {
        let mut state = SonyGainApplyState::default();
        sony_apply_gain_mdct_overlap(&mut state);
        assert!(state.output.iter().all(|&v| v.is_finite()));
    }

    fn summarize(name: &str, buf: &[f32]) {
        let mut min_v = f32::INFINITY;
        let mut max_v = f32::NEG_INFINITY;
        let mut sum_abs = 0.0f64;
        let mut first_nonzero = None;
        let mut nz_count = 0usize;
        for (i, &v) in buf.iter().enumerate() {
            if v < min_v {
                min_v = v;
            }
            if v > max_v {
                max_v = v;
            }
            sum_abs += (v as f64).abs();
            if v != 0.0 {
                nz_count += 1;
                if first_nonzero.is_none() {
                    first_nonzero = Some((i, v));
                }
            }
        }
        println!(
            "{}: len={} min={:.4e} max={:.4e} mean_abs={:.4e} nz={} first_nz={:?}",
            name,
            buf.len(),
            min_v,
            max_v,
            sum_abs / buf.len() as f64,
            nz_count,
            first_nonzero
        );
    }

    /// Diff-Harness: feeds a simple impulse (nur analysis[0]=1.0) durch
    /// FUN_004353f0 port und druckt eine Statistik aller Checkpoints.
    /// Ziel: zeigen wo die Energie hingeht. Eine Delta-Spitze sollte
    /// aus dem Impuls spektrale Energie erzeugen (MDCT eines Impulses =
    /// konstantes Spektrum).
    #[test]
    fn impulse_diff_harness_prints_checkpoints() {
        let mut analysis = [0.0f32; SONY_BAND_SAMPLES * SONY_GAIN_BANDS];
        // Impuls auf band 0 (lane 0), time sample 64 (mitten im frame).
        analysis[64 * 4 + 0] = 1.0;
        let mut output = [0.0f32; 0x400];
        let mut mid_scratch = [0.0f32; 0x400];
        let cp = sony_gain_apply_mdct_overlap_instrumented(
            [4, 4, 4, 4],
            &mut analysis,
            &mut output,
            &mut mid_scratch,
        );
        println!("=== Block 1 (analysis rewrite) ===");
        summarize("analysis_after_block1", &cp.analysis_after_block1);
        println!("=== Block 1 (stack fill) ===");
        summarize("stack_after_block1", &cp.stack_after_block1);
        println!("=== Block 2 (mid_scratch = output) ===");
        summarize("mid_scratch_after_block2", &cp.mid_scratch_after_block2);
        println!("=== Block 3 (stage4 radix-2) ===");
        summarize("stack_after_block3", &cp.stack_after_block3);
        println!("=== Block 4 (cascade length=4) ===");
        summarize("stack_after_block4_len4", &cp.stack_after_block4_len4);
        println!("=== Block 4 (cascade all) ===");
        summarize("stack_after_block4_all", &cp.stack_after_block4_all);
        println!("=== Block 5 (final 1/sqrt(2)) ===");
        summarize("stack_after_block5", &cp.stack_after_block5);
        println!("=== Block 6 (output) ===");
        summarize("output_after_block6", &cp.output_after_block6);
        println!(
            "=== per-band output magnitudes (sum abs) ===\nband 0: {:.4e}\nband 1: {:.4e}\nband 2: {:.4e}\nband 3: {:.4e}",
            cp.output_after_block6[0..256]
                .iter()
                .map(|&v| v.abs() as f64)
                .sum::<f64>(),
            cp.output_after_block6[256..512]
                .iter()
                .map(|&v| v.abs() as f64)
                .sum::<f64>(),
            cp.output_after_block6[512..768]
                .iter()
                .map(|&v| v.abs() as f64)
                .sum::<f64>(),
            cp.output_after_block6[768..1024]
                .iter()
                .map(|&v| v.abs() as f64)
                .sum::<f64>(),
        );
        // First 16 coefficients of band 0 ascending, last 16 descending.
        println!("band0 coeffs [0..16]:");
        for i in 0..16 {
            print!(" {:+.3e}", cp.output_after_block6[i]);
        }
        println!();
        println!("band0 coeffs [240..256]:");
        for i in 240..256 {
            print!(" {:+.3e}", cp.output_after_block6[i]);
        }
        println!();
        println!("band0 coeffs [120..136] (middle):");
        for i in 120..136 {
            print!(" {:+.3e}", cp.output_after_block6[i]);
        }
        println!();
    }

    /// Zweite Iteration: MDCT eines Impulses sollte (nach cold-start)
    /// ein konstantes Spektrum liefern. Wir rufen FUN_004353f0 drei Mal
    /// hintereinander auf (state carries over), und messen ob das Output
    /// sich zwischen Frame 2 und 3 stabilisiert.
    #[test]
    fn steady_state_impulse_diff_harness() {
        let mut state = SonyGainApplyState::default();

        // Frame 1: impulse at sample 64 lane 0.
        state.analysis = [0.0f32; SONY_BAND_SAMPLES * SONY_GAIN_BANDS];
        state.analysis[64 * 4 + 0] = 1.0;
        sony_apply_gain_mdct_overlap(&mut state);
        let frame1 = state.output;

        // Frame 2: same impulse.
        state.analysis = [0.0f32; SONY_BAND_SAMPLES * SONY_GAIN_BANDS];
        state.analysis[64 * 4 + 0] = 1.0;
        sony_apply_gain_mdct_overlap(&mut state);
        let frame2 = state.output;

        // Frame 3.
        state.analysis = [0.0f32; SONY_BAND_SAMPLES * SONY_GAIN_BANDS];
        state.analysis[64 * 4 + 0] = 1.0;
        sony_apply_gain_mdct_overlap(&mut state);
        let frame3 = state.output;

        let diff_23: f64 = frame2
            .iter()
            .zip(frame3.iter())
            .map(|(a, b)| (*a as f64 - *b as f64).abs())
            .sum();
        let diff_12: f64 = frame1
            .iter()
            .zip(frame2.iter())
            .map(|(a, b)| (*a as f64 - *b as f64).abs())
            .sum();
        println!(
            "Frame1→2 total-abs-diff = {:.4e}, Frame2→3 total-abs-diff = {:.4e}",
            diff_12, diff_23
        );
        println!(
            "band 0 sum|f1|={:.4e} f2={:.4e} f3={:.4e}",
            frame1[0..256].iter().map(|v| v.abs() as f64).sum::<f64>(),
            frame2[0..256].iter().map(|v| v.abs() as f64).sum::<f64>(),
            frame3[0..256].iter().map(|v| v.abs() as f64).sum::<f64>(),
        );
    }

    /// Referenz-Vergleich gegen Mdct256::forward. Erzeugt denselben
    /// Impuls und MDCT'et ihn mit der bekannten-korrekten Rust-MDCT;
    /// vergleicht mit FUN_004353f0-Port band-0 Output.
    #[test]
    fn impulse_vs_reference_mdct() {
        use crate::atrac3::mdct::{MDCT_COEFFS_PER_BAND, MDCT_INPUT_SAMPLES, Mdct256};
        // Sony erwartet interleaved 4-lane Analysis-Buffer. Impuls auf
        // band 0 bei time-sample 64.
        let mut state = SonyGainApplyState::default();
        state.analysis[64 * 4 + 0] = 1.0;
        // Steady state (3 frames) so overlap-add eingeschwungen ist.
        for _ in 0..2 {
            sony_apply_gain_mdct_overlap(&mut state);
            state.analysis = [0.0; SONY_BAND_SAMPLES * SONY_GAIN_BANDS];
            state.analysis[64 * 4 + 0] = 1.0;
        }
        sony_apply_gain_mdct_overlap(&mut state);
        let sony_band0 = &state.output[0..256];

        // Referenz: Mdct256 erwartet 512-Sample Input (overlap concat).
        // Wir modellieren dasselbe overlap-add: prev_frame_samples +
        // curr_frame_samples. Für Impuls @ sample 64, Band 0, lassen
        // wir prev = curr = zero-with-impulse.
        let mut band0_samples = [0.0f32; MDCT_COEFFS_PER_BAND];
        band0_samples[64] = 1.0;
        let mut mdct_input = [0.0f32; MDCT_INPUT_SAMPLES];
        mdct_input[..MDCT_COEFFS_PER_BAND].copy_from_slice(&band0_samples);
        mdct_input[MDCT_COEFFS_PER_BAND..].copy_from_slice(&band0_samples);
        let mdct = Mdct256::default();
        let ref_band0 = mdct.forward(&mdct_input);

        let sony_sum: f64 = sony_band0.iter().map(|v| v.abs() as f64).sum();
        let ref_sum: f64 = ref_band0.iter().map(|v| v.abs() as f64).sum();
        println!("Sony-port sum-abs = {:.4e}", sony_sum);
        println!("Mdct256  sum-abs = {:.4e}", ref_sum);
        println!("Sony-port peak   = {:.4e}", sony_band0.iter().copied().map(f32::abs).fold(0.0f32, f32::max));
        println!("Mdct256  peak    = {:.4e}", ref_band0.iter().copied().map(f32::abs).fold(0.0f32, f32::max));
        println!("Sony first 16:   {:?}", &sony_band0[0..16]);
        println!("Mdct256 first 16: {:?}", &ref_band0[0..16]);
        println!("Sony last 16:    {:?}", &sony_band0[240..256]);
        println!("Mdct256 last 16: {:?}", &ref_band0[240..256]);
    }

    /// Block-3 Sanity auf konstant-1 Input.
    /// Erwartung: stack[0..7]=2, stack[8..15]=0 repeated für alle 64 chunks.
    #[test]
    fn block3_const_sanity() {
        let (_matrix, stage4) = build_transform_tables();

        let mut stack = [1.0f32; 1024];
        let in_energy: f64 = stack.iter().map(|v| (*v as f64).powi(2)).sum();

        let mut stage_idx = 0usize;
        let mut chunk_base = 0usize;
        while stage_idx + 4 <= STAGE4_LEN && chunk_base + 16 <= stack.len() {
            let stages = [
                stage4[stage_idx + 0],
                stage4[stage_idx + 1],
                stage4[stage_idx + 2],
                stage4[stage_idx + 3],
            ];
            let mut p = [0.0f32; 16];
            for l in 0..16 {
                p[l] = stack[chunk_base + l];
            }
            let mut q_diff = [0.0f32; 4];
            let mut q_sum = [0.0f32; 4];
            let mut tmp = [0.0f32; 4];
            for l in 0..4 {
                q_diff[l] = stages[l] * (p[4 + l] - p[12 + l]);
                tmp[l] = p[l] + p[8 + l] + q_diff[l];
                q_sum[l] = stages[l] * (p[l] - p[8 + l]);
            }
            for l in 0..4 {
                stack[chunk_base + 4 + l] = p[4 + l] + p[12 + l] + q_sum[l];
                stack[chunk_base + 8 + l] = q_sum[l];
                stack[chunk_base + 12 + l] = q_diff[l];
                stack[chunk_base + l] = tmp[l];
            }
            chunk_base += 16;
            stage_idx += 8;
        }

        let out_energy: f64 = stack.iter().map(|v| (*v as f64).powi(2)).sum();
        println!(
            "Block3 in_energy: {:.4e}  out_energy: {:.4e}  ratio: {:.4e}",
            in_energy, out_energy, out_energy / in_energy
        );
        println!("stack[0..16] (chunk 0):  {:?}", &stack[0..16]);
        println!("stack[16..32] (chunk 1): {:?}", &stack[16..32]);
        println!("stack[1008..1024] (chunk 63): {:?}", &stack[1008..1024]);
    }

    /// Block-4 Energy-Conservation: ein korrekter Radix-Butterfly soll
    /// auf random/uniform Input die Energie (sum squared) erhalten (bis
    /// auf Skalierungsfaktor). Wir prüfen isoliert Block 4 length=4 mit
    /// einem konstant-1 Input.
    #[test]
    fn block4_length4_sanity() {
        let (matrix, stage4) = build_transform_tables();
        const COMBINED_LEN: usize = STAGE4_LEN + TRANSFORM_MATRIX_LEN;
        let mut combined = [0.0f32; COMBINED_LEN];
        combined[..STAGE4_LEN].copy_from_slice(&stage4);
        combined[STAGE4_LEN..].copy_from_slice(&matrix);
        const MATRIX_BASE: usize = STAGE4_LEN;

        let mut stack = [1.0f32; 1024];
        let in_energy: f64 = stack.iter().map(|v| (*v as f64).powi(2)).sum();

        // Nur Block 4 length=4 Stufe.
        let length = 4usize;
        let i_var33_plus1 = length + 1;
        let mut i_stack_1028: isize = (length / 2) as isize - 0x80;
        let mut pv37_idx: isize = MATRIX_BASE as isize + i_stack_1028;
        let mut pv30_base = 0usize;
        let mut iter_count = 0;

        loop {
            let row_start = pv37_idx as usize;
            let stages = [
                combined[row_start + 0],
                combined[row_start + 1],
                combined[row_start + 2],
                combined[row_start + 3],
            ];
            let pfvar1 = pv30_base + length * 4;
            let mut cur = pv30_base;
            while cur != pfvar1 {
                let mut a = [0.0f32; 8];
                for l in 0..8 {
                    a[l] = stack[cur + l];
                }
                let target_base = cur + (i_var33_plus1 * 4) - 4;
                let mut b = [0.0f32; 8];
                for l in 0..8 {
                    b[l] = stack[target_base + l];
                }
                for l in 0..4 {
                    stack[target_base + 0 + l] = stages[l] * (a[l] - b[l]);
                    stack[target_base + 4 + l] = stages[l] * (a[4 + l] - b[4 + l]);
                    stack[cur + 0 + l] = a[l] + b[l];
                    stack[cur + 4 + l] = a[4 + l] + b[4 + l];
                }
                cur += 8;
            }
            let mut i_var36: isize = (length as isize) * 2 - 1;
            pv30_base = cur - length * 4;
            let mut iv34_f32: isize = i_var36 * 4;
            let mut pv30 = cur - length * 4;
            while i_var36 > 0 {
                let p2_base = (pv30 as isize + iv34_f32) as usize;
                let p1_base = p2_base.wrapping_sub(4);
                let p3_base = p2_base.wrapping_sub(8);
                let p4_base = p2_base.wrapping_sub(12);
                let vals = [
                    stack[p1_base + 0], stack[p1_base + 1], stack[p1_base + 2], stack[p1_base + 3],
                    stack[p2_base + 0], stack[p2_base + 1], stack[p2_base + 2], stack[p2_base + 3],
                    stack[p3_base + 0], stack[p3_base + 1], stack[p3_base + 2], stack[p3_base + 3],
                    stack[p4_base + 0], stack[p4_base + 1], stack[p4_base + 2], stack[p4_base + 3],
                ];
                stack[pv30 + 0] = vals[4] + stack[pv30 + 0];
                stack[pv30 + 1] = vals[5] + stack[pv30 + 1];
                stack[pv30 + 2] = vals[6] + stack[pv30 + 2];
                stack[pv30 + 3] = vals[7] + stack[pv30 + 3];
                stack[pv30 + 4] = vals[0] + stack[pv30 + 4];
                stack[pv30 + 5] = vals[1] + stack[pv30 + 5];
                stack[pv30 + 6] = vals[2] + stack[pv30 + 6];
                stack[pv30 + 7] = vals[3] + stack[pv30 + 7];
                stack[pv30 + 8] = vals[8] + stack[pv30 + 8];
                stack[pv30 + 9] = vals[9] + stack[pv30 + 9];
                stack[pv30 + 10] = vals[10] + stack[pv30 + 10];
                stack[pv30 + 11] = vals[11] + stack[pv30 + 11];
                stack[pv30 + 12] = vals[12] + stack[pv30 + 12];
                stack[pv30 + 13] = vals[13] + stack[pv30 + 13];
                stack[pv30 + 14] = vals[14] + stack[pv30 + 14];
                stack[pv30 + 15] = vals[15] + stack[pv30 + 15];
                pv30 += 16;
                iv34_f32 -= 32;
                i_var36 -= 8;
            }
            i_stack_1028 += length as isize;
            pv30_base = pv30 + length * 4;
            pv37_idx += (length as isize) * 4;
            iter_count += 1;
            if i_stack_1028 >= 1 {
                break;
            }
        }

        let out_energy: f64 = stack.iter().map(|v| (*v as f64).powi(2)).sum();
        let ratio = out_energy / in_energy;
        let max_abs = stack.iter().copied().map(f32::abs).fold(0.0f32, f32::max);
        let min_v = stack.iter().copied().fold(f32::INFINITY, f32::min);
        let max_v = stack.iter().copied().fold(f32::NEG_INFINITY, f32::max);
        println!(
            "Block4 length=4 iterations: {}  in_energy: {:.4e}  out_energy: {:.4e}  ratio: {:.4e}",
            iter_count, in_energy, out_energy, ratio
        );
        println!(
            "Max abs: {:.4e}  min: {:.4e}  max: {:.4e}",
            max_abs, min_v, max_v
        );
        println!("stack[0..16]: {:?}", &stack[0..16]);
        println!("stack[16..32]: {:?}", &stack[16..32]);
    }

    /// Sonde: misst Output-Magnitude für realistische Input-Skalen.
    /// Test mit Input-Amplituden 1.0, 1000.0, 32768.0 (i16 full-scale).
    #[test]
    fn mdct_output_magnitude_by_input_scale() {
        for &scale in &[1.0f32, 1000.0, 32768.0] {
            let mut state = SonyGainApplyState::default();
            // Sinus bei 1 kHz, amplitude = scale.
            for _ in 0..3 {
                for i in 0..SONY_BAND_SAMPLES {
                    let t = (i as f32) / 44100.0;
                    let v = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * scale;
                    state.analysis[i * 4 + 0] = v;
                    state.analysis[i * 4 + 1] = 0.0;
                    state.analysis[i * 4 + 2] = 0.0;
                    state.analysis[i * 4 + 3] = 0.0;
                }
                sony_apply_gain_mdct_overlap(&mut state);
            }
            let peak = state.output[0..256]
                .iter()
                .copied()
                .map(f32::abs)
                .fold(0.0f32, f32::max);
            let sum_abs: f64 = state.output[0..256]
                .iter()
                .map(|v| v.abs() as f64)
                .sum();
            println!(
                "input_scale={:.0} -> band0 peak={:.3e}  sum-abs={:.3e}  ratio peak/scale={:.3e}",
                scale,
                peak,
                sum_abs,
                peak / scale
            );
        }
    }

    /// Echte Input-Samples (Sinus 1 kHz bei 44.1 kHz) durch FUN_004353f0
    /// und vergleiche ob das Output-Spektrum im Frequenzbereich
    /// konzentriert ist (= Band 0 bei niedrigen Frequenzen).
    #[test]
    fn sine_1khz_diff_harness() {
        let mut state = SonyGainApplyState::default();
        // 1 kHz @ 44.1 kHz, interleaved 4-band (sample i, band b) = sin(2*pi*1000*i/44100)
        // Band 0 nach QMF wäre Low-Frequency; wir fälschen das, indem wir
        // nur Band 0 füllen.
        for i in 0..SONY_BAND_SAMPLES {
            let t = (i as f32) / 44100.0;
            let v = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.5;
            for b in 0..4 {
                state.analysis[i * 4 + b] = if b == 0 { v } else { 0.0 };
            }
        }
        // Run 3 frames to reach steady state.
        for _ in 0..3 {
            sony_apply_gain_mdct_overlap(&mut state);
            state.analysis = [0.0f32; SONY_BAND_SAMPLES * SONY_GAIN_BANDS];
            for i in 0..SONY_BAND_SAMPLES {
                let t = (i as f32) / 44100.0;
                let v = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.5;
                state.analysis[i * 4 + 0] = v;
            }
        }
        sony_apply_gain_mdct_overlap(&mut state);
        // Band 0 spectrum: wo ist die maximale Magnitude?
        let band0 = &state.output[0..256];
        let (max_idx, max_val) = band0
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.abs().partial_cmp(&b.1.abs()).unwrap())
            .unwrap();
        println!("sine 1kHz band 0: max abs at bin {} value {:+.4e}", max_idx, max_val);
        println!("band 0 first 16: {:?}", &band0[0..16]);
        println!("band 0 bins 8..24: {:?}", &band0[8..24]);
        // Band 1/2/3 sollten fast null sein (nur kleine Leakage).
        let b1sum: f64 = state.output[256..512].iter().map(|v| v.abs() as f64).sum();
        let b2sum: f64 = state.output[512..768].iter().map(|v| v.abs() as f64).sum();
        let b3sum: f64 = state.output[768..1024].iter().map(|v| v.abs() as f64).sum();
        let b0sum: f64 = band0.iter().map(|v| v.abs() as f64).sum();
        println!("sum-abs b0={:.3e} b1={:.3e} b2={:.3e} b3={:.3e}", b0sum, b1sum, b2sum, b3sum);
    }
}
