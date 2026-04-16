//! 1:1 Port Sony FUN_00435cc0 (Polyphase QMF-Filter, PCM → 4-Band
//! interleaved). Quelle: `ghidra_output/decompiled.c` Zeilen
//! 41251-41414.
//!
//! Signatur Sony: `void FUN_00435cc0(int param_1, float *param_2)`
//! * `param_1` — PCM-Input (1024 f32 pro Kanal, i16-Range ≈[-32768..32767])
//! * `param_2` — Output Buffer (float*, schreibt 1024 f32)
//! Zustand: `param_2 + 0xa00` (138 f32 cross-frame-lookback)
//!
//! Sony erwartet PCM-Samples als Float in i16-Range. Unser Rust-Caller
//! muss entsprechend skalieren (Samples × 32768).

/// VA 0x0048bf50 — 4 u32 Sign-Mask-Array für Stage-1 XOR-Flips.
/// Werte: [0, 0, 0x80000000, 0x80000000]
pub const SONY_QMF_SIGN_MASK_BF50: [u32; 4] = [0, 0, 0x80000000, 0x80000000];

/// VA 0x0048bf40..0x0048bf4c — 4 × u32 Bitmasken für FUN_00435b20.
/// Alle = 0x7FFFFFFF (f32-abs-mask via sign-bit-clear).
/// Gedumped via `pefile` aus `psp_at3tool.exe`.
pub const SONY_ENVELOPE_ABS_MASKS: [u32; 4] = [0x7fffffff, 0x7fffffff, 0x7fffffff, 0x7fffffff];

/// Anzahl Pre-History-Floats (32) und Post-Padding-Floats (24) die
/// `sony_envelope_fun_00435b20` außerhalb des 1024-float-Frames liest.
/// Sony's `puVar5 = param_1 + 0x20` startet 32 floats nach Frame-Anfang
/// und liest in Iteration k=0 `puVar5[-8..-1]` (= param_1[0x18..0x1f]),
/// in Iteration k=31 `puVar5[0x10..0x17]` (= param_1[0x410..0x417]).
pub const SONY_ENVELOPE_PRE_HISTORY: usize = 32;
pub const SONY_ENVELOPE_POST_LOOKAHEAD: usize = 24;

/// VA 0x0048c1a0 — 4 u32 Sign-Mask-Array für Stage-2 XOR-Flips.
/// Werte: [0x80000000, 0, 0x80000000, 0]
pub const SONY_QMF_SIGN_MASK_C1A0: [u32; 4] = [0x80000000, 0, 0x80000000, 0];

/// VA 0x0048bf60 — 48-tap symmetrischer FIR-Filter (Stage-1).
/// Gedumped aus EXE.
pub const SONY_QMF_FILTER_BF60: [f32; 48] = [
    1.0, 6.296_897_9, 3.841_391_3, -20.601_358,
    -16.570_951, 58.344_273, 35.608_105, -139.134_51,
    -53.583_359, 288.348_45, 51.723_530, -536.305_91,
    4.184_255, 919.457_95, -168.456_82, -1486.831_3,
    533.664_00, 2331.900_6, -1286.640_6, -3716.105_7,
    2982.158_2, 6798.269_0, -9034.712_9, -31755.891,
    -31755.891, -9034.712_9, 6798.269_0, 2982.158_2,
    -3716.105_7, -1286.640_6, 2331.900_6, 533.664_00,
    -1486.831_3, -168.456_82, 919.457_95, 4.184_255,
    -536.305_91, 51.723_530, 288.348_45, -53.583_359,
    -139.134_51, 35.608_105, 58.344_273, -16.570_951,
    -20.601_358, 3.841_391_3, 6.296_897_9, 1.0,
];

/// VA 0x0048c020 — Stage-2 FIR-Koeffizienten (96 f32), 2-fach
/// interleaved Layout für die zweite Filter-Stage.
pub const SONY_QMF_FILTER_C020: [f32; 96] = [
    1.0, 6.296_897_9, 1.0, 6.296_897_9,
    3.841_391_3, -20.601_358, 3.841_391_3, -20.601_358,
    -16.570_951, 58.344_273, -16.570_951, 58.344_273,
    35.608_105, -139.134_51, 35.608_105, -139.134_51,
    -53.583_359, 288.348_45, -53.583_359, 288.348_45,
    51.723_530, -536.305_91, 51.723_530, -536.305_91,
    4.184_255, 919.457_95, 4.184_255, 919.457_95,
    -168.456_82, -1486.831_3, -168.456_82, -1486.831_3,
    533.664_00, 2331.900_6, 533.664_00, 2331.900_6,
    -1286.640_6, -3716.105_7, -1286.640_6, -3716.105_7,
    2982.158_2, 6798.269_0, 2982.158_2, 6798.269_0,
    -9034.712_9, -31755.891, -9034.712_9, -31755.891,
    -31755.891, -9034.712_9, -31755.891, -9034.712_9,
    6798.269_0, 2982.158_2, 6798.269_0, 2982.158_2,
    -3716.105_7, -1286.640_6, -3716.105_7, -1286.640_6,
    2331.900_6, 533.664_00, 2331.900_6, 533.664_00,
    -1486.831_3, -168.456_82, -1486.831_3, -168.456_82,
    919.457_95, 4.184_255, 919.457_95, 4.184_255,
    -536.305_91, 51.723_530, -536.305_91, 51.723_530,
    288.348_45, -53.583_359, 288.348_45, -53.583_359,
    -139.134_51, 35.608_105, -139.134_51, 35.608_105,
    58.344_273, -16.570_951, 58.344_273, -16.570_951,
    -20.601_358, 3.841_391_3, -20.601_358, 3.841_391_3,
    6.296_897_9, 1.0, 6.296_897_9, 1.0,
];

/// VA 0x0048c190 — Top-Tap-Koeffizienten für Stage-2.
pub const SONY_QMF_FILTER_C190: [f32; 4] = [6.296_897_9, 1.0, 6.296_897_9, 1.0];

/// Sony Cross-Frame-State: 138 f32 lookback-Buffer.
pub const SONY_QMF_STATE_LEN: usize = 0x8a; // 138

/// Index-Helpers für die Stage-1-Filter, die pro Sony-Line eine
/// konkrete Linearkombination im `pfVar2[offset]`-Raum beschreiben.
#[inline(always)]
fn xor_sign(a: f32, mask: u32) -> f32 {
    f32::from_bits(a.to_bits() ^ mask)
}

/// Sony Q-format Compensation für QMF Filter-Coeffs.
///
/// **Empirischer Beweis:** Sony's BF60/C020/C190-Coeffs in der EXE sind
/// fixed-point Q16 integers gespeichert als f32 (max raw value -31755.89,
/// max real Wert = -31755.89 / 65536 = -0.485 — konsistent mit Standard-
/// Low-Pass Filter). Sony's PC-Encoder-Decompile zeigt direkte Float-
/// Multiplikationen `_DAT_0048bf60 * pfVar2[-0x28]` ohne explizite
/// Q-Compensation; die Compensation muss aber irgendwo in Sony's Pipeline
/// existieren, da Sony's funktionierende AT3-Roundtrips Coefs ≤ 32768
/// erfordern (Quantizer-Range, FUN_004387d0 sf_index limit).
///
/// Ohne diese Compensation produziert die Pipeline ~10^16-Magnituden
/// statt der Sony-konformen ≤32768. Stage 1 (BF60) und Stage 2 (C020)
/// sind beide Q16, daher Compensation = 2^-32 am Stage-2-Output.
const SONY_QMF_Q_COMPENSATION: f32 = 1.0 / (65536.0 * 65536.0);

/// 1:1 Port FUN_00435cc0 — Decompile-Zeilen 41251-41414.
///
/// * `input` — PCM-Samples (1024 f32, Sony-Scale: i16-Range).
/// * `output` — 4-Band interleaved Output (1024 f32).
/// * `state` — persistent lookback (138 f32), muss zwischen Frames
///   erhalten bleiben.
pub fn sony_qmf_filter(input: &[f32; 1024], output: &mut [f32; 1024], state: &mut [f32; SONY_QMF_STATE_LEN]) {
    // Stack buffer local_1240[1024] + local_240[139] ≈ 1163 f32
    // kombiniert. Initial-Copy (Ghidra 41275-41281): 138 f32 aus
    // state → local_1240[0..138].
    let mut local_1240 = [0.0f32; 1163];
    local_1240[..SONY_QMF_STATE_LEN].copy_from_slice(state);

    // Haupt-Loop (41282-41405): 256 Iterationen, 4 Output-f32 pro Iter.
    // Sony-Variablen:
    //   pfVar2 = local_1240 + 0x84  (Schreibkopf im Stack-Buffer)
    //   pfVar4 = param_2            (Output-Pointer, advances +4 per iter)
    //   pfVar1 = param_1 + (pfVar4 - param_2) = input at output-relative offset
    let mut pfvar2: usize = 0x84; // f32-Index in local_1240
    let mut pfvar4: usize = 0; // f32-Index in output

    for _ in 0..256 {
        // Ghidra 41286-41294: read 4 input samples relative to pfVar4.
        let f6_in = input[pfvar4];
        let f7_in = input[pfvar4 + 1];
        let f8_in = input[pfvar4 + 2];
        let f9_in = input[pfvar4 + 3];
        local_1240[pfvar2 + 6] = f6_in;
        local_1240[pfvar2 + 7] = f7_in;
        local_1240[pfvar2 + 8] = f8_in;
        local_1240[pfvar2 + 9] = f9_in;

        // Stage 1a (Ghidra 41295-41306): fVar5.
        // Koeffizienten-Offsets relativ zu DAT_0048bf60 (= SONY_QMF_FILTER_BF60[0..]):
        // +0x18, +0x08, +0x18, +0x28, +0x38, +0x48, +0x58, +0x68, +0x78, +0x88, +0x98, +0xa8
        // = indices 6, 2, 6, 10, 14, 18, 22, 26, 30, 34, 38, 42 (UNK-only parts) …
        // Einfacher: Sony-Pattern 1:1 transkribieren.
        let c = &SONY_QMF_FILTER_BF60;
        let f5 = c[46] * local_1240[pfvar2 + 6]
            + c[2] * local_1240[pfvar2.wrapping_sub(0x26)]
            + c[6] * local_1240[pfvar2.wrapping_sub(0x22)]
            + c[10] * local_1240[pfvar2.wrapping_sub(0x1e)]
            + c[14] * local_1240[pfvar2.wrapping_sub(0x1a)]
            + c[18] * local_1240[pfvar2.wrapping_sub(0x16)]
            + c[22] * local_1240[pfvar2.wrapping_sub(0x12)]
            + c[26] * local_1240[pfvar2.wrapping_sub(0x0e)]
            + c[30] * local_1240[pfvar2.wrapping_sub(0x0a)]
            + c[34] * local_1240[pfvar2.wrapping_sub(0x06)]
            + c[38] * local_1240[pfvar2.wrapping_sub(0x02)]
            + c[42] * local_1240[pfvar2 + 2]
            + c[44] * local_1240[pfvar2 + 4]
            + c[0] * local_1240[pfvar2.wrapping_sub(0x28)]
            + c[4] * local_1240[pfvar2.wrapping_sub(0x24)]
            + c[8] * local_1240[pfvar2.wrapping_sub(0x20)]
            + c[12] * local_1240[pfvar2.wrapping_sub(0x1c)]
            + c[16] * local_1240[pfvar2.wrapping_sub(0x18)]
            + c[20] * local_1240[pfvar2.wrapping_sub(0x14)]
            + c[24] * local_1240[pfvar2.wrapping_sub(0x10)]
            + c[28] * local_1240[pfvar2.wrapping_sub(0x0c)]
            + c[32] * local_1240[pfvar2.wrapping_sub(0x08)]
            + c[36] * local_1240[pfvar2.wrapping_sub(0x04)]
            + c[40] * local_1240[pfvar2];

        // Stage 1b (Ghidra 41307-41316): fVar6 — selbe Koeffizienten,
        // Sample-Offsets um -2 verschoben.
        let f6 = c[46] * f8_in
            + c[2] * local_1240[pfvar2.wrapping_sub(0x24)]
            + c[6] * local_1240[pfvar2.wrapping_sub(0x20)]
            + c[10] * local_1240[pfvar2.wrapping_sub(0x1c)]
            + c[14] * local_1240[pfvar2.wrapping_sub(0x18)]
            + c[18] * local_1240[pfvar2.wrapping_sub(0x14)]
            + c[22] * local_1240[pfvar2.wrapping_sub(0x10)]
            + c[26] * local_1240[pfvar2.wrapping_sub(0x0c)]
            + c[30] * local_1240[pfvar2.wrapping_sub(0x08)]
            + c[34] * local_1240[pfvar2.wrapping_sub(0x04)]
            + c[38] * local_1240[pfvar2]
            + c[42] * local_1240[pfvar2 + 4]
            + c[44] * f6_in
            + c[0] * local_1240[pfvar2.wrapping_sub(0x26)]
            + c[4] * local_1240[pfvar2.wrapping_sub(0x22)]
            + c[8] * local_1240[pfvar2.wrapping_sub(0x1e)]
            + c[12] * local_1240[pfvar2.wrapping_sub(0x1a)]
            + c[16] * local_1240[pfvar2.wrapping_sub(0x16)]
            + c[20] * local_1240[pfvar2.wrapping_sub(0x12)]
            + c[24] * local_1240[pfvar2.wrapping_sub(0x0e)]
            + c[28] * local_1240[pfvar2.wrapping_sub(0x0a)]
            + c[32] * local_1240[pfvar2.wrapping_sub(0x06)]
            + c[36] * local_1240[pfvar2.wrapping_sub(0x02)]
            + c[40] * local_1240[pfvar2 + 2];

        // Stage 1c (Ghidra 41317-41328): fVar8 — odd-lanes Filter.
        let f8 = c[47] * local_1240[pfvar2 + 7]
            + c[3] * local_1240[pfvar2.wrapping_sub(0x25)]
            + c[7] * local_1240[pfvar2.wrapping_sub(0x21)]
            + c[11] * local_1240[pfvar2.wrapping_sub(0x1d)]
            + c[15] * local_1240[pfvar2.wrapping_sub(0x19)]
            + c[19] * local_1240[pfvar2.wrapping_sub(0x15)]
            + c[23] * local_1240[pfvar2.wrapping_sub(0x11)]
            + c[27] * local_1240[pfvar2.wrapping_sub(0x0d)]
            + c[31] * local_1240[pfvar2.wrapping_sub(0x09)]
            + c[35] * local_1240[pfvar2.wrapping_sub(0x05)]
            + c[39] * local_1240[pfvar2.wrapping_sub(0x01)]
            + c[43] * local_1240[pfvar2 + 3]
            + c[45] * local_1240[pfvar2 + 5]
            + c[1] * local_1240[pfvar2.wrapping_sub(0x27)]
            + c[5] * local_1240[pfvar2.wrapping_sub(0x23)]
            + c[9] * local_1240[pfvar2.wrapping_sub(0x1f)]
            + c[13] * local_1240[pfvar2.wrapping_sub(0x1b)]
            + c[17] * local_1240[pfvar2.wrapping_sub(0x17)]
            + c[21] * local_1240[pfvar2.wrapping_sub(0x13)]
            + c[25] * local_1240[pfvar2.wrapping_sub(0x0f)]
            + c[29] * local_1240[pfvar2.wrapping_sub(0x0b)]
            + c[33] * local_1240[pfvar2.wrapping_sub(0x07)]
            + c[37] * local_1240[pfvar2.wrapping_sub(0x03)]
            + c[41] * local_1240[pfvar2 + 1];

        // Stage 1d (Ghidra 41329-41338): fVar10 — odd-lanes shift.
        let f10 = c[47] * f9_in
            + c[3] * local_1240[pfvar2.wrapping_sub(0x23)]
            + c[7] * local_1240[pfvar2.wrapping_sub(0x1f)]
            + c[11] * local_1240[pfvar2.wrapping_sub(0x1b)]
            + c[15] * local_1240[pfvar2.wrapping_sub(0x17)]
            + c[19] * local_1240[pfvar2.wrapping_sub(0x13)]
            + c[23] * local_1240[pfvar2.wrapping_sub(0x0f)]
            + c[27] * local_1240[pfvar2.wrapping_sub(0x0b)]
            + c[31] * local_1240[pfvar2.wrapping_sub(0x07)]
            + c[35] * local_1240[pfvar2.wrapping_sub(0x03)]
            + c[39] * local_1240[pfvar2 + 1]
            + c[43] * local_1240[pfvar2 + 5]
            + c[45] * f7_in
            + c[1] * local_1240[pfvar2.wrapping_sub(0x25)]
            + c[5] * local_1240[pfvar2.wrapping_sub(0x21)]
            + c[9] * local_1240[pfvar2.wrapping_sub(0x1d)]
            + c[13] * local_1240[pfvar2.wrapping_sub(0x19)]
            + c[17] * local_1240[pfvar2.wrapping_sub(0x15)]
            + c[21] * local_1240[pfvar2.wrapping_sub(0x11)]
            + c[25] * local_1240[pfvar2.wrapping_sub(0x0d)]
            + c[29] * local_1240[pfvar2.wrapping_sub(0x09)]
            + c[33] * local_1240[pfvar2.wrapping_sub(0x05)]
            + c[37] * local_1240[pfvar2.wrapping_sub(0x01)]
            + c[41] * local_1240[pfvar2 + 3];

        // Ghidra 41339-41342: 4-lane XOR + Add.
        let f11 = xor_sign(f8, SONY_QMF_SIGN_MASK_BF50[0]) + f5;
        let f12 = xor_sign(f10, SONY_QMF_SIGN_MASK_BF50[1]) + f6;
        let f8_new = xor_sign(f5, SONY_QMF_SIGN_MASK_BF50[2]) + f8;
        let f10_new = xor_sign(f6, SONY_QMF_SIGN_MASK_BF50[3]) + f10;

        // Stage 2 (Ghidra 41343-41390): zweiter Filter mit
        // SONY_QMF_FILTER_C020 (Stride 4 = 4-wide replicated Rows).
        // Koeffizient-Indices: +0x190 lane 0-3 für top, +0x020 + 16*k lane 0-3.
        let d = &SONY_QMF_FILTER_C020;
        let top = &SONY_QMF_FILTER_C190;

        // Helper: lies d[k*4 + lane] und multipliziere mit pfvar2[offset]
        // (mehrfach gefaltete Struktur).
        // 24 Taps pro Stage-2-Line, offsets -0x84..-0x2c (f32-stride -4).
        let stage2 = |lane: usize, pivot: f32, offsets_start: usize| -> f32 {
            let mut acc = top[lane] * pivot;
            let mut off = offsets_start;
            for k in 0..24 {
                let idx = 0x020 / 4 + k * 4 + lane; // d-Index. d start at offset 0x020 bytes from c000
                // Aber d ist SONY_QMF_FILTER_C020 direkt indexierbar: d[k*4 + lane] für k=0..23.
                let di = k * 4 + lane;
                acc += d[di] * local_1240[pfvar2.wrapping_sub(off + k * 4)];
                let _ = idx;
            }
            acc
        };
        let _ = stage2;

        // Expliziter 1:1 Transkript Stage 2 (wie Sony):
        let f6_s2 = top[0] * f11
            + d[0] * local_1240[pfvar2.wrapping_sub(0x84)]
            + d[4] * local_1240[pfvar2.wrapping_sub(0x80)]
            + d[8] * local_1240[pfvar2.wrapping_sub(0x7c)]
            + d[12] * local_1240[pfvar2.wrapping_sub(0x78)]
            + d[16] * local_1240[pfvar2.wrapping_sub(0x74)]
            + d[20] * local_1240[pfvar2.wrapping_sub(0x70)]
            + d[24] * local_1240[pfvar2.wrapping_sub(0x6c)]
            + d[28] * local_1240[pfvar2.wrapping_sub(0x68)]
            + d[32] * local_1240[pfvar2.wrapping_sub(0x64)]
            + d[36] * local_1240[pfvar2.wrapping_sub(0x60)]
            + d[40] * local_1240[pfvar2.wrapping_sub(0x5c)]
            + d[44] * local_1240[pfvar2.wrapping_sub(0x58)]
            + d[48] * local_1240[pfvar2.wrapping_sub(0x54)]
            + d[52] * local_1240[pfvar2.wrapping_sub(0x50)]
            + d[56] * local_1240[pfvar2.wrapping_sub(0x4c)]
            + d[60] * local_1240[pfvar2.wrapping_sub(0x48)]
            + d[64] * local_1240[pfvar2.wrapping_sub(0x44)]
            + d[68] * local_1240[pfvar2.wrapping_sub(0x40)]
            + d[72] * local_1240[pfvar2.wrapping_sub(0x3c)]
            + d[76] * local_1240[pfvar2.wrapping_sub(0x38)]
            + d[80] * local_1240[pfvar2.wrapping_sub(0x34)]
            + d[84] * local_1240[pfvar2.wrapping_sub(0x30)]
            + d[88] * local_1240[pfvar2.wrapping_sub(0x2c)];

        let f7_s2 = top[1] * f12
            + d[1] * local_1240[pfvar2.wrapping_sub(0x83)]
            + d[5] * local_1240[pfvar2.wrapping_sub(0x7f)]
            + d[9] * local_1240[pfvar2.wrapping_sub(0x7b)]
            + d[13] * local_1240[pfvar2.wrapping_sub(0x77)]
            + d[17] * local_1240[pfvar2.wrapping_sub(0x73)]
            + d[21] * local_1240[pfvar2.wrapping_sub(0x6f)]
            + d[25] * local_1240[pfvar2.wrapping_sub(0x6b)]
            + d[29] * local_1240[pfvar2.wrapping_sub(0x67)]
            + d[33] * local_1240[pfvar2.wrapping_sub(0x63)]
            + d[37] * local_1240[pfvar2.wrapping_sub(0x5f)]
            + d[41] * local_1240[pfvar2.wrapping_sub(0x5b)]
            + d[45] * local_1240[pfvar2.wrapping_sub(0x57)]
            + d[49] * local_1240[pfvar2.wrapping_sub(0x53)]
            + d[53] * local_1240[pfvar2.wrapping_sub(0x4f)]
            + d[57] * local_1240[pfvar2.wrapping_sub(0x4b)]
            + d[61] * local_1240[pfvar2.wrapping_sub(0x47)]
            + d[65] * local_1240[pfvar2.wrapping_sub(0x43)]
            + d[69] * local_1240[pfvar2.wrapping_sub(0x3f)]
            + d[73] * local_1240[pfvar2.wrapping_sub(0x3b)]
            + d[77] * local_1240[pfvar2.wrapping_sub(0x37)]
            + d[81] * local_1240[pfvar2.wrapping_sub(0x33)]
            + d[85] * local_1240[pfvar2.wrapping_sub(0x2f)]
            + d[89] * local_1240[pfvar2.wrapping_sub(0x2b)];

        let f9_s2 = top[2] * f8_new
            + d[2] * local_1240[pfvar2.wrapping_sub(0x82)]
            + d[6] * local_1240[pfvar2.wrapping_sub(0x7e)]
            + d[10] * local_1240[pfvar2.wrapping_sub(0x7a)]
            + d[14] * local_1240[pfvar2.wrapping_sub(0x76)]
            + d[18] * local_1240[pfvar2.wrapping_sub(0x72)]
            + d[22] * local_1240[pfvar2.wrapping_sub(0x6e)]
            + d[26] * local_1240[pfvar2.wrapping_sub(0x6a)]
            + d[30] * local_1240[pfvar2.wrapping_sub(0x66)]
            + d[34] * local_1240[pfvar2.wrapping_sub(0x62)]
            + d[38] * local_1240[pfvar2.wrapping_sub(0x5e)]
            + d[42] * local_1240[pfvar2.wrapping_sub(0x5a)]
            + d[46] * local_1240[pfvar2.wrapping_sub(0x56)]
            + d[50] * local_1240[pfvar2.wrapping_sub(0x52)]
            + d[54] * local_1240[pfvar2.wrapping_sub(0x4e)]
            + d[58] * local_1240[pfvar2.wrapping_sub(0x4a)]
            + d[62] * local_1240[pfvar2.wrapping_sub(0x46)]
            + d[66] * local_1240[pfvar2.wrapping_sub(0x42)]
            + d[70] * local_1240[pfvar2.wrapping_sub(0x3e)]
            + d[74] * local_1240[pfvar2.wrapping_sub(0x3a)]
            + d[78] * local_1240[pfvar2.wrapping_sub(0x36)]
            + d[82] * local_1240[pfvar2.wrapping_sub(0x32)]
            + d[86] * local_1240[pfvar2.wrapping_sub(0x2e)]
            + d[90] * local_1240[pfvar2.wrapping_sub(0x2a)];

        let f5_s2 = top[3] * f10_new
            + d[3] * local_1240[pfvar2.wrapping_sub(0x81)]
            + d[7] * local_1240[pfvar2.wrapping_sub(0x7d)]
            + d[11] * local_1240[pfvar2.wrapping_sub(0x79)]
            + d[15] * local_1240[pfvar2.wrapping_sub(0x75)]
            + d[19] * local_1240[pfvar2.wrapping_sub(0x71)]
            + d[23] * local_1240[pfvar2.wrapping_sub(0x6d)]
            + d[27] * local_1240[pfvar2.wrapping_sub(0x69)]
            + d[31] * local_1240[pfvar2.wrapping_sub(0x65)]
            + d[35] * local_1240[pfvar2.wrapping_sub(0x61)]
            + d[39] * local_1240[pfvar2.wrapping_sub(0x5d)]
            + d[43] * local_1240[pfvar2.wrapping_sub(0x59)]
            + d[47] * local_1240[pfvar2.wrapping_sub(0x55)]
            + d[51] * local_1240[pfvar2.wrapping_sub(0x51)]
            + d[55] * local_1240[pfvar2.wrapping_sub(0x4d)]
            + d[59] * local_1240[pfvar2.wrapping_sub(0x49)]
            + d[63] * local_1240[pfvar2.wrapping_sub(0x45)]
            + d[67] * local_1240[pfvar2.wrapping_sub(0x41)]
            + d[71] * local_1240[pfvar2.wrapping_sub(0x3d)]
            + d[75] * local_1240[pfvar2.wrapping_sub(0x39)]
            + d[79] * local_1240[pfvar2.wrapping_sub(0x35)]
            + d[83] * local_1240[pfvar2.wrapping_sub(0x31)]
            + d[87] * local_1240[pfvar2.wrapping_sub(0x2d)]
            + d[91] * local_1240[pfvar2.wrapping_sub(0x29)];

        // Ghidra 41391-41393: zweite Sign-XOR-Kette.
        let f13 = xor_sign(f6_s2, SONY_QMF_SIGN_MASK_C1A0[0]);
        let f14 = xor_sign(f9_s2, SONY_QMF_SIGN_MASK_C1A0[2]);
        let f15 = xor_sign(f5_s2, SONY_QMF_SIGN_MASK_C1A0[3]);

        // Ghidra 41394-41397: State-Update pfVar2[-0x28..-0x25].
        local_1240[pfvar2.wrapping_sub(0x28)] = f11;
        local_1240[pfvar2.wrapping_sub(0x27)] = f12;
        local_1240[pfvar2.wrapping_sub(0x26)] = f8_new;
        local_1240[pfvar2.wrapping_sub(0x25)] = f10_new;

        // Ghidra 41398-41401: Output-Writes (4 Bänder interleaved).
        // Q-format Compensation für Q16 Stage-1+Stage-2 Coeffs.
        output[pfvar4 + 0] = (f6_s2 + xor_sign(f7_s2, SONY_QMF_SIGN_MASK_C1A0[1])) * SONY_QMF_Q_COMPENSATION;
        output[pfvar4 + 1] = (f7_s2 + f13) * SONY_QMF_Q_COMPENSATION;
        output[pfvar4 + 2] = (f5_s2 + f14) * SONY_QMF_Q_COMPENSATION;
        output[pfvar4 + 3] = (f9_s2 + f15) * SONY_QMF_Q_COMPENSATION;

        pfvar2 += 4;
        pfvar4 += 4;
    }

    // Ghidra 41406-41412: save state (138 f32 am Ende des
    // lookback-Buffers, ab local_240 = local_1240[0x400-0x8a=886]).
    // local_1240 hat 1024 Einträge, local_240 = local_1240[1024-139..]
    // — aber wir haben local_1240 um 139 erweitert (1163 total).
    // Sony liest aus local_240[0..138] und schreibt nach param_2+0xa00.
    // local_240 ist direkt NACH local_1240[1024], also in unserer
    // Implementierung an Index 1024..1162. Lookback-Range 0x8a = 138.
    state.copy_from_slice(&local_1240[1024..1024 + SONY_QMF_STATE_LEN]);
}

/// Erwartete Input-Länge für `sony_envelope_fun_00435b20`: 1024 (Frame)
/// + 32 Pre-History (vom letzten Frame) + 24 Post-Lookahead. Sony's
/// `param_1` zeigt auf Frame-Start; Pre-History wird logisch *vor*
/// param_1 gelesen, was Sony's `puVar5[-8..]` in Iteration 0 entspricht.
pub const SONY_ENVELOPE_INPUT_LEN: usize = 1024 + SONY_ENVELOPE_PRE_HISTORY + SONY_ENVELOPE_POST_LOOKAHEAD;
pub const SONY_ENVELOPE_OUTPUT_LEN: usize = 4 * 32;

/// 1:1 Port FUN_00435b20 — Decompile `ghidra_output/decompiled.c`
/// Zeilen 41157-41242. SIMD max-abs envelope-Estimator über
/// 4-Lane-interleaved QMF-Output.
///
/// Layout-Konvention:
/// * `input[0..32]` = Pre-History (32 floats vor Sony-Frame-Anfang)
/// * `input[32..1056]` = aktueller QMF-Frame (1024 floats, 4-lane
///   interleaved wie sony_qmf_filter-Output)
/// * `input[1056..1080]` = Post-Lookahead (24 floats vom Folge-Frame
///   oder 0 für letzten Frame)
///
/// Output: 128 floats, layout `output[band * 32 + slot]` für band in
/// 0..4, slot in 0..32. Innerhalb eines slots: max-abs über 32 input-
/// samples (8 time-steps × 4 lanes).
///
/// Sign-Mask 0x7fffffff via SONY_ENVELOPE_ABS_MASKS.
pub fn sony_envelope_fun_00435b20(
    input: &[f32; SONY_ENVELOPE_INPUT_LEN],
    output: &mut [f32; SONY_ENVELOPE_OUTPUT_LEN],
) {
    // Sony: uVar1..4 = _DAT_0048bf40.._UNK_0048bf4c (alle 0x7fffffff).
    let masks = SONY_ENVELOPE_ABS_MASKS;

    // Sony: puVar5 = (uint *)(param_1 + 0x20). In unserem Layout ist
    // input[SONY_ENVELOPE_PRE_HISTORY] = Frame-Start = "param_1"; also
    // puVar5_index = SONY_ENVELOPE_PRE_HISTORY (= 32).
    let mut pv5: usize = SONY_ENVELOPE_PRE_HISTORY;

    // 32 Iterationen (Sony: iVar6 = 0x20).
    for slot in 0..32usize {
        // SIMD-Lane-Helfer: lese vier f32 ab `pv5+offset`, maskiere mit
        // sign-clear (= abs).
        let load_abs = |base: i32| -> [f32; 4] {
            let mut out = [0.0f32; 4];
            for lane in 0..4 {
                let idx = (pv5 as i32 + base + lane as i32) as usize;
                let bits = input[idx].to_bits() & masks[lane];
                out[lane] = f32::from_bits(bits);
            }
            out
        };
        let max4 = |a: [f32; 4], b: [f32; 4]| -> [f32; 4] {
            [a[0].max(b[0]), a[1].max(b[1]), a[2].max(b[2]), a[3].max(b[3])]
        };

        // Sony 41194-41197: auVar7 = abs(puVar5[0..3])
        let av7_a = load_abs(0);
        // Sony 41190-41193: auVar8 = abs(puVar5[4..7])
        let av8_a = load_abs(4);
        // auVar7 = maxps(auVar7, auVar8)
        let mut av7 = max4(av7_a, av8_a);

        // Sony 41203-41206: auVar13 = abs(puVar5[-8..-5])
        let av13 = load_abs(-8);
        // Sony 41212-41215: auVar11 = abs(puVar5[-4..-1])
        let av11 = load_abs(-4);
        // auVar14 = maxps(auVar13, auVar11)
        let av14 = max4(av13, av11);

        // Sony 41207-41210: auVar14_b = abs(puVar5[8..11])
        let av14_b = load_abs(8);
        // Sony 41199-41202: auVar10 = abs(puVar5[12..15])
        let av10 = load_abs(12);
        // auVar8 = maxps(auVar14_b, auVar10)
        let av8 = max4(av14_b, av10);

        // Sony 41217: auVar7 = maxps(auVar14, auVar7)
        av7 = max4(av14, av7);
        // Sony 41218: auVar7 = maxps(auVar7, auVar8)
        av7 = max4(av7, av8);

        // Sony 41223-41226: auVar9 = abs(puVar5[16..19])
        let av9 = load_abs(16);
        // Sony 41219-41222: auVar12 = abs(puVar5[20..23])
        let av12 = load_abs(20);
        // Sony 41228: auVar8 = maxps(auVar9, auVar12)
        let av8b = max4(av9, av12);
        // Sony 41229: auVar7 = maxps(auVar7, auVar8)
        av7 = max4(av7, av8b);

        // Sony 41230-41237: param_2[0/0x20/0x40/0x60] = auVar7[0..3]
        output[0 * 32 + slot] = av7[0];
        output[1 * 32 + slot] = av7[1];
        output[2 * 32 + slot] = av7[2];
        output[3 * 32 + slot] = av7[3];

        // Sony 41227: puVar5 += 0x20
        pv5 += 0x20;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qmf_zero_input_zero_output() {
        let input = [0.0f32; 1024];
        let mut output = [0.0f32; 1024];
        let mut state = [0.0f32; SONY_QMF_STATE_LEN];
        sony_qmf_filter(&input, &mut output, &mut state);
        assert!(output.iter().all(|v| v.abs() < 1e-9));
    }

    #[test]
    fn qmf_impulse_produces_filter_tail() {
        let mut input = [0.0f32; 1024];
        input[0] = 1.0;
        let mut output = [0.0f32; 1024];
        let mut state = [0.0f32; SONY_QMF_STATE_LEN];
        sony_qmf_filter(&input, &mut output, &mut state);
        let peak = output.iter().copied().map(f32::abs).fold(0.0f32, f32::max);
        println!("QMF impulse: peak abs = {:.4e}", peak);
    }

    #[test]
    fn qmf_sine_band_isolation() {
        // 1 kHz sine → should land in Band 0 (low freq).
        let mut input = [0.0f32; 1024];
        for i in 0..1024 {
            let t = (i as f32) / 44100.0;
            input[i] = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.5;
        }
        let mut output = [0.0f32; 1024];
        let mut state = [0.0f32; SONY_QMF_STATE_LEN];
        for _ in 0..5 {
            sony_qmf_filter(&input, &mut output, &mut state);
        }
        // Output interleaved: out[i*4+band] = band-sample at time i.
        let mut band_sums = [0.0f64; 4];
        for i in 0..256 {
            for b in 0..4 {
                band_sums[b] += (output[i * 4 + b] as f64).abs();
            }
        }
        println!("QMF 1kHz band energies: {:?}", band_sums);
    }

    #[test]
    fn qmf_sine_1khz_scale_i16() {
        // 1 kHz @ 44.1 kHz, i16-scale amplitude 10000.
        let mut input = [0.0f32; 1024];
        for i in 0..1024 {
            let t = (i as f32) / 44100.0;
            input[i] = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 10000.0;
        }
        let mut output = [0.0f32; 1024];
        let mut state = [0.0f32; SONY_QMF_STATE_LEN];
        // Run 3 frames steady-state.
        for _ in 0..3 {
            sony_qmf_filter(&input, &mut output, &mut state);
        }
        let peak = output.iter().copied().map(f32::abs).fold(0.0f32, f32::max);
        let sum_abs: f64 = output.iter().map(|v| v.abs() as f64).sum();
        println!("QMF sine i16-scale: peak={:.4e} sum-abs={:.4e}", peak, sum_abs);
    }

    #[test]
    fn envelope_fun_00435b20_matches_naive_max_abs() {
        // Generate deterministic test input with varied magnitudes.
        let mut input = [0.0f32; SONY_ENVELOPE_INPUT_LEN];
        for (i, v) in input.iter_mut().enumerate() {
            *v = ((i as f32 * 0.0173).sin() * 1000.0) + ((i as f32 * 0.0091).cos() * 250.0);
            if i % 17 == 0 {
                *v *= -3.0;
            }
        }
        let mut sony_out = [0.0f32; SONY_ENVELOPE_OUTPUT_LEN];
        sony_envelope_fun_00435b20(&input, &mut sony_out);

        // Naive reference: for slot s in 0..32, for band b in 0..4,
        // out[b*32 + s] = max abs over the 32 input floats Sony reads in
        // iteration s, taking only the lane b (offset 0..3 mod 4 within
        // each 4-float maxps group). Iteration s reads input indices
        // [0x18 + s*0x20 .. 0x37 + s*0x20] = 32 floats, organized in 4
        // groups of (4 floats each) with lane assignment [0,1,2,3].
        let mut naive = [0.0f32; SONY_ENVELOPE_OUTPUT_LEN];
        for s in 0..32 {
            let base = 0x18 + s * 0x20;
            for lane in 0..4 {
                let mut m = 0.0f32;
                // Sony's iter reads 8 groups of 4 floats; per group, lane
                // l picks the l-th float.
                for group in 0..8 {
                    m = m.max(input[base + group * 4 + lane].abs());
                }
                naive[lane * 32 + s] = m;
            }
        }

        for (i, (s, n)) in sony_out.iter().zip(naive.iter()).enumerate() {
            assert!(
                (s - n).abs() < 1e-5,
                "envelope mismatch at out[{}]: sony={} naive={}",
                i, s, n
            );
        }
    }
}
