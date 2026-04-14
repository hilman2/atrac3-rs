use std::f64::consts::PI;

pub const MDCT_COEFFS_PER_BAND: usize = 256;
pub const MDCT_INPUT_SAMPLES: usize = MDCT_COEFFS_PER_BAND * 2;
const PRE_ROTATION_BINS: usize = MDCT_COEFFS_PER_BAND / 2;
const FFT_TWIDDLES: usize = PRE_ROTATION_BINS / 2;
const OUTPUT_SCALE: f32 = 1.0 / 128.0;

pub fn atrac3_analysis_window_half() -> [f32; MDCT_COEFFS_PER_BAND] {
    let mut out = [0.0; MDCT_COEFFS_PER_BAND];
    for (i, slot) in out.iter_mut().enumerate() {
        let phase = (((i as f64 + 0.5) / MDCT_COEFFS_PER_BAND as f64) - 0.5) * PI;
        *slot = ((phase.sin() + 1.0) * 0.5) as f32;
    }
    out
}

pub fn symmetric_window_from_half(half: &[f32; MDCT_COEFFS_PER_BAND]) -> [f32; MDCT_INPUT_SAMPLES] {
    let mut out = [0.0; MDCT_INPUT_SAMPLES];
    for i in 0..MDCT_COEFFS_PER_BAND {
        out[i] = half[i];
        out[MDCT_COEFFS_PER_BAND + i] = half[MDCT_COEFFS_PER_BAND - 1 - i];
    }
    out
}

fn build_pre_rotation_cos() -> [f32; PRE_ROTATION_BINS] {
    let mut out = [0.0; PRE_ROTATION_BINS];
    for (index, slot) in out.iter_mut().enumerate() {
        *slot = ((index as f64) * PI / PRE_ROTATION_BINS as f64).cos() as f32;
    }
    out
}

fn build_pre_rotation_neg_sin() -> [f32; PRE_ROTATION_BINS] {
    let mut out = [0.0; PRE_ROTATION_BINS];
    for (index, slot) in out.iter_mut().enumerate() {
        *slot = -((index as f64) * PI / PRE_ROTATION_BINS as f64).sin() as f32;
    }
    out
}

fn build_fft_cos() -> [f32; FFT_TWIDDLES] {
    let mut out = [0.0; FFT_TWIDDLES];
    for (index, slot) in out.iter_mut().enumerate() {
        *slot = ((index as f64) * PI / FFT_TWIDDLES as f64).cos() as f32;
    }
    out
}

fn build_fft_neg_sin() -> [f32; FFT_TWIDDLES] {
    let mut out = [0.0; FFT_TWIDDLES];
    for (index, slot) in out.iter_mut().enumerate() {
        *slot = -((index as f64) * PI / FFT_TWIDDLES as f64).sin() as f32;
    }
    out
}

fn build_post_rotation_tables() -> (
    [f32; PRE_ROTATION_BINS],
    [f32; PRE_ROTATION_BINS],
    [f32; PRE_ROTATION_BINS],
    [f32; PRE_ROTATION_BINS],
) {
    let mut alpha_minus = [0.0; PRE_ROTATION_BINS];
    let mut beta_minus = [0.0; PRE_ROTATION_BINS];
    let mut beta_plus = [0.0; PRE_ROTATION_BINS];
    let mut alpha_plus = [0.0; PRE_ROTATION_BINS];

    for index in 0..PRE_ROTATION_BINS {
        let alpha = ((2 * index + 1) as f64) * PI / 1024.0;
        let beta = 5.0 * alpha;

        alpha_minus[index] = (0.5 * (beta.cos() - alpha.sin())) as f32;
        alpha_plus[index] = (0.5 * (beta.cos() + alpha.sin())) as f32;
        beta_minus[index] = (0.5 * (alpha.cos() - beta.sin())) as f32;
        beta_plus[index] = (0.5 * (alpha.cos() + beta.sin())) as f32;
    }

    (alpha_minus, beta_minus, beta_plus, alpha_plus)
}

fn build_bit_reverse_table() -> [usize; PRE_ROTATION_BINS] {
    let mut out = [0usize; PRE_ROTATION_BINS];
    for (index, slot) in out.iter_mut().enumerate() {
        *slot = (index.reverse_bits() >> (usize::BITS - 7)) as usize;
    }
    out
}

#[derive(Debug, Clone)]
pub struct Mdct256 {
    analysis_window: [f32; MDCT_INPUT_SAMPLES],
    pre_rotation_cos: [f32; PRE_ROTATION_BINS],
    pre_rotation_neg_sin: [f32; PRE_ROTATION_BINS],
    fft_cos: [f32; FFT_TWIDDLES],
    fft_neg_sin: [f32; FFT_TWIDDLES],
    post_alpha_minus: [f32; PRE_ROTATION_BINS],
    post_beta_minus: [f32; PRE_ROTATION_BINS],
    post_beta_plus: [f32; PRE_ROTATION_BINS],
    post_alpha_plus: [f32; PRE_ROTATION_BINS],
    bit_reverse: [usize; PRE_ROTATION_BINS],
}

impl Default for Mdct256 {
    fn default() -> Self {
        Self::new(symmetric_window_from_half(&atrac3_analysis_window_half()))
    }
}

impl Mdct256 {
    pub fn new(window: [f32; MDCT_INPUT_SAMPLES]) -> Self {
        let (post_alpha_minus, post_beta_minus, post_beta_plus, post_alpha_plus) =
            build_post_rotation_tables();

        Self {
            analysis_window: window,
            pre_rotation_cos: build_pre_rotation_cos(),
            pre_rotation_neg_sin: build_pre_rotation_neg_sin(),
            fft_cos: build_fft_cos(),
            fft_neg_sin: build_fft_neg_sin(),
            post_alpha_minus,
            post_beta_minus,
            post_beta_plus,
            post_alpha_plus,
            bit_reverse: build_bit_reverse_table(),
        }
    }

    pub fn forward(&self, input: &[f32; MDCT_INPUT_SAMPLES]) -> [f32; MDCT_COEFFS_PER_BAND] {
        let mut scratch = [0.0f32; 513];
        let mut visited = [false; PRE_ROTATION_BINS];

        for index in 0..64 {
            scratch[257 + index] = -(self.analysis_window[384 + index * 2]
                * input[384 + index * 2])
                - (self.analysis_window[383 - index * 2] * input[383 - index * 2]);
        }
        for index in 0..128 {
            scratch[321 + index] = self.analysis_window[index * 2] * input[index * 2]
                - self.analysis_window[255 - index * 2] * input[255 - index * 2];
        }
        for index in 0..64 {
            scratch[449 + index] = self.analysis_window[511 - index * 2] * input[511 - index * 2]
                + self.analysis_window[256 + index * 2] * input[256 + index * 2];
        }

        for index in 0..PRE_ROTATION_BINS {
            let source = 257 + index * 2;
            let left = scratch[source];
            let right = scratch[source + 1];
            let cos = self.pre_rotation_cos[index];
            let neg_sin = self.pre_rotation_neg_sin[index];

            scratch[1 + index] = cos * left - neg_sin * right;
            scratch[129 + index] = cos * right + neg_sin * left;
        }

        for index in 0..PRE_ROTATION_BINS {
            if visited[index] {
                continue;
            }
            let target = self.bit_reverse[index];
            let real = scratch[1 + target];
            let imag = scratch[129 + target];
            scratch[1 + target] = scratch[1 + index];
            scratch[129 + target] = scratch[129 + index];
            scratch[1 + index] = real;
            scratch[129 + index] = imag;
            visited[target] = true;
        }

        let mut span = 64usize;
        let mut stage_width = 2usize;
        for _ in 0..7 {
            let half_width = stage_width / 2;
            let mut block_start = 0usize;
            let mut block_end = half_width;

            for _ in 0..span {
                let mut twiddle = 0usize;
                let mut upper = block_start;
                let mut lower = block_end;

                for _ in 0..half_width {
                    let rotated_real = self.fft_cos[twiddle] * scratch[1 + lower]
                        - self.fft_neg_sin[twiddle] * scratch[129 + lower];
                    let rotated_imag = self.fft_cos[twiddle] * scratch[129 + lower]
                        + self.fft_neg_sin[twiddle] * scratch[1 + lower];

                    let upper_real = scratch[1 + upper];
                    let upper_imag = scratch[129 + upper];
                    scratch[1 + lower] = upper_real - rotated_real;
                    scratch[129 + lower] = upper_imag - rotated_imag;
                    scratch[1 + upper] = upper_real + rotated_real;
                    scratch[129 + upper] = upper_imag + rotated_imag;

                    twiddle += span;
                    upper += 1;
                    lower += 1;
                }

                block_start += stage_width;
                block_end += stage_width;
            }

            span /= 2;
            stage_width *= 2;
        }

        for index in 0..PRE_ROTATION_BINS {
            let left_real = scratch[256 - index];
            let right_real = scratch[1 + index];
            let right_imag = scratch[129 + index];
            let left_imag = scratch[128 - index];

            scratch[257 + index] = self.post_alpha_minus[index] * left_real
                + self.post_beta_minus[index] * right_real
                + self.post_alpha_plus[index] * right_imag
                + self.post_beta_plus[index] * left_imag;
            scratch[512 - index] = left_real * self.post_beta_plus[index]
                + (self.post_alpha_plus[index] * right_real
                    - left_imag * self.post_alpha_minus[index])
                - self.post_beta_minus[index] * right_imag;
        }

        let mut output = [0.0f32; MDCT_COEFFS_PER_BAND];
        for (index, coefficient) in output.iter_mut().enumerate() {
            *coefficient = scratch[257 + index] * OUTPUT_SCALE;
        }
        output
    }

    /// Brute-force reference forward MDCT that is mathematically the exact
    /// inverse of `Imdct256::inverse` (without the decoder window and IMDCT
    /// scale, which are handled in overlap-add).  Uses the textbook Type-IV
    /// MDCT definition:
    ///   X[k] = sum_{n=0}^{2N-1} w[n] * x[n] * cos(pi/N * (n+0.5+N/2) * (k+0.5))
    pub fn forward_reference(&self, input: &[f32; MDCT_INPUT_SAMPLES]) -> [f32; MDCT_COEFFS_PER_BAND] {
        let n = MDCT_COEFFS_PER_BAND as f64;
        let mut output = [0.0f32; MDCT_COEFFS_PER_BAND];

        for (k, slot) in output.iter_mut().enumerate() {
            let mut acc = 0.0f64;
            for (sample_index, sample) in input.iter().enumerate() {
                let windowed = *sample as f64 * self.analysis_window[sample_index] as f64;
                let angle = (PI / n)
                    * ((sample_index as f64 + 0.5 + n / 2.0) * (k as f64 + 0.5));
                acc += windowed * angle.cos();
            }
            *slot = acc as f32;
        }

        output
    }
}

#[cfg(test)]
mod tests {
    use super::{
        FFT_TWIDDLES, MDCT_INPUT_SAMPLES, Mdct256, PRE_ROTATION_BINS, atrac3_analysis_window_half,
        build_bit_reverse_table, build_fft_cos, build_fft_neg_sin, build_post_rotation_tables,
    };

    #[test]
    fn zero_frame_stays_zero() {
        let mdct = Mdct256::default();
        let input = [0.0f32; MDCT_INPUT_SAMPLES];
        let output = mdct.forward(&input);
        assert!(output.iter().all(|value| *value == 0.0));
    }

    #[test]
    fn analysis_window_matches_binary_prefix() {
        let half = atrac3_analysis_window_half();
        let expected = [
            0.000009411997f32,
            0.000084709901f32,
            0.000235291329f32,
            0.000461136398f32,
        ];
        for (observed, expected) in half.into_iter().zip(expected) {
            assert!((observed - expected).abs() < 1e-7);
        }
    }

    #[test]
    fn fft_twiddles_match_binary_prefix() {
        let cos = build_fft_cos();
        let neg_sin = build_fft_neg_sin();

        assert!((cos[1] - 0.99879545).abs() < 1e-7);
        assert!((neg_sin[1] - -0.049067676).abs() < 1e-7);
        assert_eq!(cos.len(), FFT_TWIDDLES);
    }

    #[test]
    fn post_rotation_tables_match_binary_prefix() {
        let (alpha_minus, beta_minus, beta_plus, alpha_plus) = build_post_rotation_tables();

        assert!((alpha_minus[0] - 0.498407185).abs() < 1e-6);
        assert!((beta_minus[0] - 0.492328048).abs() < 1e-6);
        assert!((beta_plus[0] - 0.507667243).abs() < 1e-6);
        assert!((alpha_plus[0] - 0.501475155).abs() < 1e-6);
        assert_eq!(alpha_minus.len(), PRE_ROTATION_BINS);
    }

    #[test]
    fn bit_reverse_matches_binary_prefix() {
        let bit_reverse = build_bit_reverse_table();
        let expected = [0usize, 64, 32, 96, 16, 80, 48, 112];
        assert_eq!(&bit_reverse[..expected.len()], &expected);
    }
}
