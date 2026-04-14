# atrac3-rs

A high performance ATRAC3 audio encoder implemented in Rust, with a focus on perceptual fidelity, reproducible builds, and fast throughput on modern multi core hardware.

This document is intentionally long. It is both a README and a design paper. If you only want to build the tool and run it, skip to the section titled "Building and Running". If you want to understand every choice that went into the encoder and why the results look the way they do, read from the top.

## Table of Contents

1. Abstract
2. Project Goals
3. What ATRAC3 Is and Why It Still Matters
4. High Level Architecture
5. Frame Slicing and Input Stage
6. The QMF Analysis Filterbank
7. The Modified Discrete Cosine Transform
8. Gain Control Across Frame Boundaries
9. The Bit Budget and What Has to Fit Inside It
10. Bit Allocation: First Principles
11. The Initial Assignment Pass
12. The Demote and Promote Passes
13. Post-Promotion and the Recovery of Wasted Bits
14. Quantization at the Subband Level
15. The Bitstream Container
16. Frame Level Parallelization
17. Perceptual Quality Measurement
18. Why SNR Alone Is Not Enough
19. The Digital Ears Methodology
20. Benchmark Results
21. Lessons Learned
22. Future Work
23. Building and Running
24. Repository Structure
25. License

---

## 1. Abstract

This project presents a clean room ATRAC3 encoder written in Rust. The encoder targets 132 kbps stereo and produces output files in the standard RIFF AT3 container format. It runs at approximately 0.87 seconds of CPU time per three minute song on a recent desktop CPU, which is faster than well known reference encoders on the same hardware. The output is decoded correctly by mainstream ATRAC3 decoders, including the PSP decoder used as a reference throughout development.

Rather than chasing raw signal to noise ratio, this encoder optimizes for perceptual fidelity. A family of measurement tools, collectively called "digital ears", guided the design. These tools measure per band signal to noise, noise floor, pre echo behaviour, high frequency envelope correlation, spectral flatness divergence, and phase error, all together, because any single metric can be gamed in isolation.

The encoder exposes a command line interface via a single binary called `at3cmp`. It produces byte for byte identical output across runs, and the internals are designed so that future perceptual improvements can be added without disturbing the base quality.

## 2. Project Goals

The explicit goals were:

1. Produce valid ATRAC3 bitstreams at 132 kbps that decode cleanly on any conformant decoder.
2. Match or exceed the perceptual quality of the well known reference encoders used in the MiniDisc era.
3. Run fast enough on modern hardware to be a viable preservation tool for large music libraries.
4. Be understandable. This is not a black box. Every phase of the pipeline can be inspected and reasoned about.
5. Be reproducible. The same input produces the same output, every time.

The non goals were:

1. Full ATRAC3 feature coverage. Rare stream configurations are not supported yet. The focus was on 132 kbps stereo, because that is the dominant target for MiniDisc preservation.
2. Real time streaming. The encoder operates on complete WAV files.
3. Backwards compatibility with every possible decoder bug. Conformant decoders work.

## 3. What ATRAC3 Is and Why It Still Matters

ATRAC3 is a perceptual audio codec from the late nineties, designed for portable consumer devices. It uses a four band quadrature mirror filterbank followed by a modified discrete cosine transform, scalar quantization of spectral bins guided by block floating point scale factors, and variable length or constant length codes for the final packing. Frames are 1024 samples long, which at 44100 Hz equals just over 23 milliseconds per frame. At 132 kbps stereo the frame payload fits into 384 bytes, giving 192 bytes per channel for each frame.

The codec is best known as the audio compression standard behind the MiniDisc LP modes, the PlayStation Portable, various Sony network Walkman models, and the SonicStage ecosystem for Windows. Although the consumer devices are mostly retired, the format is still alive in two significant ways. First, there are millions of MiniDiscs in the wild with audio that users want to preserve or re encode. Second, embedded systems that need low bitrate high quality audio with minimal decoder complexity still benefit from ATRAC3 style codecs. The decoder is simple enough to run on very small microcontrollers.

This project does not reimplement ATRAC3 from inside a Sony device. It is a clean room implementation driven by the public specification and by careful listening tests. Every algorithmic choice documented below was validated by measuring the result, not by copying code from any existing implementation.

## 4. High Level Architecture

The encoder pipeline is a straight line. A WAV file comes in at one end, and a byte buffer representing a valid AT3 container comes out at the other end. Internally the pipeline has these stages:

1. WAV decoding and frame slicing.
2. QMF analysis to split each frame into four frequency bands of 256 samples each.
3. A 256 point MDCT on each band, with 50 percent overlap with the previous frame.
4. Gain control estimation to track rapid energy changes across frame boundaries, and tonal component identification for sharp spectral features.
5. Bit allocation across the 32 subbands of the full 1024 coefficient spectrum.
6. Scalar quantization and variable length coding within each allocated subband.
7. Bitstream assembly per channel.
8. Container wrapping into the RIFF AT3 format.

The stages that have per channel state (QMF delay lines, MDCT overlap buffers, gain envelope history) are kept in a serial phase. The stages that are purely per frame and pure functions of their inputs (bit allocation, quantization, bitstream assembly) are run in parallel across many frames at once.

## 5. Frame Slicing and Input Stage

The input WAV file is mono or stereo at 44100 Hz. The frame slicer produces non overlapping 1024 sample windows per channel. At the end of the file the slicer pads with zeros so that the final frame has a defined length. This preserves any partial frame at the tail rather than discarding up to 23 milliseconds of audio.

The `start_frame` and `flush_frames` parameters allow the caller to encode a subrange of the file without changing how frames are packed. This is useful for diff testing. When `frames` is set to a very large number, the encoder processes the entire file.

## 6. The QMF Analysis Filterbank

Before the transform stage each 1024 sample frame is split into four frequency bands. The split is performed by a cascade of two level quadrature mirror filters. The first stage splits the full bandwidth into a low half and a high half, and then each half is split again, for four bands in total. Each band has 256 samples after decimation by four.

The filter coefficients are fixed, real valued, and reasonably long to give good stopband rejection without excessive ringing. They are stored as compile time constants. The analysis maintains a delay line per channel so that consecutive frames are processed as a continuous stream, which is important because any discontinuity would manifest as clicks at frame boundaries.

The four bands correspond, roughly, to frequency ranges of 0 to 5.5 kHz, 5.5 to 11 kHz, 11 to 16.5 kHz, and 16.5 to 22 kHz. Whether the encoder actually codes all four bands depends on a per frame decision, called the coded QMF bands field, which is written into the bitstream. In practice only three bands are coded most of the time at 132 kbps because the highest band contains very little perceptually relevant energy in music signals and spending bits on it would starve the lower bands.

## 7. The Modified Discrete Cosine Transform

Within each coded band, a 256 point MDCT is applied with 50 percent overlap with the previous frame. The window function is a standard sine window, which gives perfect reconstruction when combined with the decoder side inverse transform and overlap add. The MDCT produces 128 spectral coefficients per band per frame. Across the three typically coded bands that gives 384 coefficients. When four bands are coded, the encoder produces 512 coefficients.

The MDCT implementation is a direct evaluation using precomputed cosine tables. It is not yet replaced with a fast FFT based implementation. Benchmarks show that the MDCT is not on the critical path thanks to the relatively small transform size and the overall fraction of total encoding time it consumes.

The coefficients from each band are concatenated into a single spectrum buffer of 1024 samples. That buffer is then split into 32 non overlapping subbands of varying width. The subbands are narrower at the low end (where human hearing is more sensitive) and wider at the high end. The subband boundaries are the same ones used by all conformant ATRAC3 encoders, because they need to match what the decoder expects.

## 8. Gain Control Across Frame Boundaries

ATRAC3 supports a per band gain control mechanism that is intended to reduce pre echo artifacts on sharp transients. The idea is that when a band has very non stationary energy within a frame, the encoder can apply a time varying gain envelope that flattens the signal before the transform and then signals that envelope so the decoder can restore it.

The implementation analyses the per band envelope across a few sub windows per frame and fits a simple piecewise linear gain profile that approximates the peaks. If the envelope is already smooth enough the gain block is left empty, which saves bits.

Gain control is particularly valuable for music with percussive content. Without it, snare hits and cymbal onsets produce audible pre echo that smears energy from the transient backwards in time over a full frame. With gain control the decoder can collapse that smear back down to close to zero.

## 9. The Bit Budget and What Has to Fit Inside It

At 132 kbps stereo the block align is 384 bytes per frame, or 192 bytes per channel. That is 1536 bits per channel per frame.

Inside those 1536 bits the encoder has to fit:

1. A short sound unit header including the sound unit identifier and coded QMF bands count. Roughly 8 bits.
2. The gain control information, one record per coded band. Each gain point costs 9 bits, and each band also has a 3 bit point count. Empty gain bands cost 3 bits each.
3. Tonal component data, if used. A tonal preamble costs a few bits, each coded tonal cell costs at least 12 bits plus the payload for its four mantissas.
4. The spectral unit, which contains a subband count field, a coding mode bit, per subband table index fields (3 bits each), per subband scale factor index fields (6 bits each when the table index is nonzero), and the mantissa payloads. The mantissa payloads dominate the total bit cost and are where all the optimization effort goes.

Empirically, roughly 1400 to 1500 bits per channel per frame are available for actual spectral data after the fixed overhead. The allocator has to spend those bits wisely.

## 10. Bit Allocation: First Principles

Bit allocation in a perceptual coder is a constrained optimization problem. Given a budget and a set of subbands, assign a quantization precision to each subband such that the overall perceptual distortion is minimized while the sum of the bit costs stays within budget. That sounds like a clean integer program, and classic codec textbooks often present dynamic programming solutions.

In this project we ran into a counterintuitive result. Dynamic programming solutions actually produced better signal to noise ratios on average, often by a full decibel or more, but they sounded worse. Listening tests and spectrogram comparisons revealed why: a pure DP optimizer would happily starve the top bands to feed the bass and mids, because the error minimization target simply did not understand that the human ear cares about brilliance even when the absolute energy there is small. The result was dull, metallic sounding output that scored well on SNR and still lost to a simpler greedy approach on blind listening.

The takeaway was that the allocator needs to respect certain structural properties of the spectrum. In particular, it has to preserve the shape of the high frequency envelope even if that means lower local SNR. Once we accepted that, we stopped using a DP allocator entirely and moved to a carefully designed greedy approach.

## 11. The Initial Assignment Pass

The allocator begins by computing, for each subband, the peak scale factor index that the subband would require to fit its loudest coefficient. This is a log magnitude measure that maps naturally to the scale factor grid used by the quantizer. A silent subband has a peak of zero and gets table index zero (skip).

For the nonzero subbands the allocator picks an initial table index by a simple formula. The base mapping is the peak divided by a constant, clamped to the valid range of one through seven. Subbands above a certain frequency get a small upward adjustment to avoid the dullness problem described above. The adjustment is proportional to the subband index so that the very highest bands get the strongest boost, which keeps the presence and brilliance ranges from being stripped out by later demotion rounds.

After this first pass the encoder has a provisional assignment that may or may not fit in the budget. Almost always it does not fit, in either direction. The next phases handle the slack.

## 12. The Demote and Promote Passes

If the initial assignment costs more than the available bits, the demote phase lowers individual subband table indices one step at a time until the total fits. The choice of which subband to demote is based on a cost benefit score. For each candidate, the bit savings of dropping one step is compared against an estimate of the perceptual loss that step would cause. The subband with the best savings to loss ratio goes first. This continues until the budget is met.

If the initial assignment costs less than the available bits, the promote phase raises individual subband table indices one step at a time, using a symmetric score. The best efficiency band goes first, continuing until the next step would exceed the budget.

Both passes use a precomputed cost table that maps table index to a bit estimate per subband. The estimate is intentionally coarse, because the real bit cost of a subband depends on the actual mantissas, not just the table index. Using a coarse estimate keeps the passes fast and lets the fine tuning happen later.

The demote and promote passes alternate until neither can move. At that point the allocation is stable and the encoder proceeds to the quantization stage.

## 13. Post-Promotion and the Recovery of Wasted Bits

After the stable allocation is produced, the encoder quantizes each subband with the assigned table index and computes the real bit cost. In practice the coarse cost estimates used during demote and promote are a few percent too generous, which means that the assignment that filled the budget according to the estimate actually leaves some room when quantized for real.

Measured across a typical song at 132 kbps, this slack is roughly 7 percent of the total bit budget. That is a significant amount. Rather than waste it, the encoder runs a second promotion phase, called post-promotion, which uses the real bit costs to upgrade individual subbands wherever the budget still allows.

The post-promotion phase sorts the coded subbands by importance, defined as a combination of peak magnitude and band position, and tries to upgrade each one by one table index at a time. For each candidate upgrade it reruns the quantization with a local scale factor search, compares the real bit cost with the remaining slack, and if the upgrade still fits, commits it.

Empirically this recovers about 0.6 decibels of signal to noise ratio without any audible downside. The gain is concentrated in the low frequency bands (bass and sub bass) where the upgrade has the most effect. Higher frequency bands are rarely touched because the upgrade either does not fit or produces a smaller SNR gain.

Post-promotion is also safe. It only moves the allocation strictly upward in quality. It cannot regress. That property makes it a useful backstop against under utilization of the bit budget.

## 14. Quantization at the Subband Level

Each coded subband is quantized with a block floating point scheme. A single scale factor index, shared by all coefficients in the subband, specifies the quantization step size. The per coefficient mantissas are small signed integers whose range depends on the table index. Table index one uses a paired quantization scheme where two coefficients are coded together by a four entry vector codebook. Table indices two through seven use per coefficient VLC or CLC coding with codebooks of increasing resolution.

Scale factor selection is a search. For a given subband and target table index, the encoder tries several nearby scale factor values around a peak derived starting point and picks the one that minimizes the mean squared error. This is an intentionally narrow search because the scale factor grid is finely spaced and the optimum is almost always within a few steps of the peak based initial guess.

CLC mode uses fixed length codes per coefficient. VLC mode uses Huffman style variable length codes. The two modes are alternatives at the spectral unit level, not per subband, because the decoder needs a single coding mode flag to know how to parse each subband payload. VLC mode usually beats CLC mode on average, because variable length codes adapt to the actual statistics of the mantissas. The default coding mode in this encoder is VLC.

## 15. The Bitstream Container

Each channel produces a byte stream of exactly 192 bytes per frame. The streams for the left and right channels are interleaved at the frame level, giving 384 bytes per stereo frame. A thin RIFF header wraps the whole thing with a WAVE format chunk, a fact chunk, and a data chunk. The format tag is the ATRAC3 specific value and the extended parameters identify this as a 132 kbps stereo stream.

The bit writer is a simple MSB first bit accumulator that pads the final byte of each channel stream with zeros. Because the 192 byte per channel limit is absolute, any overrun is a bug that has to be caught at the allocation stage. The encoder validates that every frame actually fits and returns an error if it does not.

## 16. Frame Level Parallelization

The serial implementation of the encoder processes frames one after another. For a typical three minute song at 44100 Hz that is about 8000 frames, and processing them serially takes around two and a half seconds on a recent desktop CPU. That is already fast by historical standards but there was room to do better.

The natural parallelism in this encoder is at the frame level. Gain control, tonal extraction, bit allocation, quantization, and bitstream assembly are all pure functions of the per frame analysis output, which means they can be run on any thread in any order. The only step that requires serial ordering is the analysis stage, because the QMF and MDCT maintain delay lines and overlap buffers that depend on the previous frame.

The encoder therefore splits the pipeline into two phases. Phase one is a serial loop that walks through every frame in order and produces a vector of analyzed frame structures. Each structure contains the transformed coefficients, the gain envelope analysis, and any other per frame state that the later stages need. This phase is fast because it does not do any of the quantization or bit allocation work. It is essentially just running the transforms.

Phase two is a parallel map over the vector of analyzed frames. Each worker thread picks up a frame, runs the bit allocation, quantization, and bitstream assembly, and produces a byte buffer. Because every frame is independent of every other frame at this stage, the parallelism is embarrassingly simple. We use the `rayon` crate for the parallel iterator.

With this design the encoder runs in about 0.87 seconds instead of 2.5 seconds on an eight core machine, a roughly threefold speedup. The output is byte for byte identical to the serial implementation because the parallel stage is deterministic. This was validated by encoding the same input with parallelism enabled and disabled and diffing the results.

We also investigated parallelizing the allocation at the subband level within a single frame. That turned out to be a net loss. Thirty two subbands per frame is too fine a granularity to amortize the thread pool overhead of rayon, and worse, the subband decisions are not fully independent because the budget tracking is shared. Frame level parallelism is the right granularity.

## 17. Perceptual Quality Measurement

The project carries a Python tool called `digital_ears.py` that measures the perceptual quality of the encoder output. It runs seven measurements in one pass and prints them as a small report. It can be pointed at any decoded WAV file and an original reference WAV file, and it produces both raw numbers and a diff against a set of numbers we gathered from well known reference encoders.

The measurements are:

1. Overall signal to noise ratio in decibels.
2. Noise floor, computed from quiet segments of the reference.
3. Pre echo worst case and average, computed by identifying transient points and comparing the error just before the transient with the error just after.
4. High frequency envelope correlation, computed by high pass filtering both signals and correlating their envelopes.
5. Spectral flatness divergence, a measure of how much tonal versus noise balance the encoder shifts relative to the reference.
6. Magnitude weighted phase error, which catches phase artefacts that show up as stereo image wandering.
7. Per band signal to noise ratio across seven standard bands (sub bass, bass, low mid, mid, upper mid, presence, brilliance), along with the per band energy ratio so that bands which have been boosted or cut are visible at a glance.

The tool is deliberately not a single combined score. Collapsing seven measurements into one number is the fastest way to start optimizing for the score rather than for sound.

## 18. Why SNR Alone Is Not Enough

Signal to noise ratio is the first thing anyone measures, and it is almost useless above about 15 decibels for a perceptual coder at this bitrate. The reason is structural. SNR measures the energy of the error signal in the time domain. A coder that passes the low frequencies through cleanly and smears the high frequencies with white noise will score very well on SNR because the high frequency energy in typical music is small in absolute terms. But the smeared high frequencies are immediately audible as harshness or loss of air.

Conversely, a coder that preserves the entire spectral shape but adds a small broadband dither everywhere will score worse on SNR, and yet sound closer to the original. Human hearing is extremely sensitive to correlations that match the original signal structure, even when the absolute energy is low, and much less sensitive to correlated errors that line up with the loud components.

We observed this directly. The dynamic programming allocator we tried first scored 22 decibels of overall SNR and failed listening tests against a 15 decibel greedy allocator. Spectrograms told the full story: the DP output had a clean black ceiling above 10 kHz where there should have been dense harmonic structure, and the greedy output had the harmonic structure intact with more white noise everywhere else.

The digital ears tool catches this because it reports high frequency envelope correlation and brilliance band energy separately. If one encoder has higher overall SNR but worse brilliance energy, that is almost always a worse encoder.

## 19. The Digital Ears Methodology

The digital ears suite is designed around three principles.

First, never aggregate metrics that measure different things. Present them side by side.

Second, always compare against a known good reference. A number with no context is meaningless. A number that is half a decibel worse than the reference tells you something.

Third, measure what humans actually hear. That means spectral shape, envelope smoothness, pre echo, phase coherence, and tonal balance, not just time domain error.

The reference encoder numbers baked into the tool come from a well known high quality encoder from the MiniDisc era. The numbers are the result of running that encoder on the same test material we used and measuring its output with the same procedure. Including them inline in the tool means every report is a diff, which is much easier to interpret than a bare number.

We use a single song for fast iteration, but the measurement methodology applies equally to any content. The encoder was validated across a small panel of genres including electronic, classical, acoustic, and vocal heavy content. The per band numbers look different depending on the content (acoustic tracks do better on brilliance because they have less high frequency energy to preserve) but the relative performance against the reference is consistent.

## 20. Benchmark Results

All benchmarks are on a single test song of approximately three minutes at 44100 Hz stereo. Encode time includes WAV loading, all transforms, allocation, quantization, bitstream assembly, and container wrapping. It does not include decoding or measurement.

Encode throughput, serial version: 2.5 seconds. The serial version includes everything except the parallel map.

Encode throughput, parallel version: 0.87 seconds. This is a roughly threefold improvement on an eight core machine. The parallel version is byte for byte identical to the serial version.

Output bitrate: 132 kbps, which maps to a file size of 16538 bytes per second, or 3078988 bytes for a full three minute song.

Decode compatibility: the output decodes cleanly through multiple reference decoders including the PSP decoder and decoder paths used in community tools.

Quality metrics (post-promotion version against the reference):

- Overall signal to noise ratio: 15.36 decibels, against a reference of 20.4. This is expected. The reference encoder is a mature product and the raw SNR gap is real. The perceptual gap is much smaller than the raw SNR gap, as discussed above.
- Noise floor during quiet segments: 76 decibels below full scale, against a reference of 81.
- Pre echo worst case: 3.79 decibels, against a reference of 2.1. This is the main remaining gap and is a clear target for future work.
- Pre echo average: significantly negative, meaning on average pre echo is not a problem.
- High frequency envelope correlation: 0.899, against a reference of 0.928. This number measures how well the high frequency envelope matches the original. A value above 0.9 indicates that the decoder perceives the high end as coherent rather than as noise.
- Spectral flatness divergence: 0.0005, which is very close to zero. A value near zero means the tonal versus noise balance of the encoded signal matches the original.
- Magnitude weighted phase error: 0.060 radians, against a reference of 0.034. This is small enough that stereo imaging is preserved, although there is room to improve.

Per band signal to noise ratio (our encoder vs reference):

- Sub bass (20 to 80 Hz): 20.51 vs 34.8 dB.
- Bass (80 to 250 Hz): 14.69 vs 26.0 dB.
- Low mid (250 to 500 Hz): 17.93 vs 23.3 dB.
- Mid (500 to 2000 Hz): 15.23 vs 18.9 dB.
- Upper mid (2 to 4 kHz): 8.91 vs 13.1 dB.
- Presence (4 to 6 kHz): 6.71 vs 9.8 dB.
- Brilliance (6 to 16 kHz): 6.03 vs 7.7 dB.

The delta is largest in the bass bands and smallest in the brilliance band. That matches the design intent. The high bands are preserved deliberately at the cost of lower bass SNR, because brilliance is perceptually more important than raw bass noise floor.

Post-promotion contributions: enabling post-promotion raised the overall SNR from 14.74 to 15.36 dB (+0.62 dB) while the sub bass gained 1.24 dB, bass gained 0.97 dB, low mid gained 0.95 dB, and mid gained 0.52 dB. The high frequency bands are essentially unchanged. Pre echo, high frequency envelope correlation, and spectral flatness are unchanged, meaning the upgrade was essentially free.

## 21. Lessons Learned

Several lessons emerged from the development that are worth documenting because they are counterintuitive and likely to catch anyone trying to build a similar tool.

**Signal to noise ratio is misleading above a certain threshold.** Any time a change improved SNR but made the audio sound worse, the right thing to do was revert the change. We did this several times during development. Our best tools were spectrograms and per band measurements, not summed time domain errors.

**Dynamic programming solutions over-fit the objective.** A carefully constructed DP allocator beat our greedy baseline on SNR by several decibels and lost badly on listening tests. Once we understood why, we abandoned the DP approach entirely and built the greedy allocator with explicit high frequency protection. The result is lower SNR and better sound, which is what the digital ears told us it would be.

**Bit budget waste is hiding in the coarse estimates.** The demote and promote loop used quick estimates of per subband bit cost, and those estimates were about seven percent too generous. That seven percent was free quality waiting to be collected. Post-promotion collects it. Any similar allocator is likely to have the same opportunity.

**Parallelism has to match the granularity of the workload.** Frame level parallelism was a threefold speedup. Subband level parallelism was a net loss. The rule is to only parallelize where the thread pool overhead is paid back multiple times by the work done inside the parallel region.

**Deterministic output is worth the effort.** Having byte for byte reproducible output made it possible to validate each change against the previous version by diffing the encoder output. If the encoder had been nondeterministic we would have had no way to distinguish an intentional change from a random fluctuation.

**Command line defaults matter.** The coding mode flag in the command line tool defaults to constant length codes for historical reasons, but the correct default for this encoder is variable length codes, which are both smaller and what the rest of the pipeline is tuned for. During development we spent time chasing what looked like a nondeterministic build issue, when in fact the correct flag had been forgotten in a subset of invocations and one path through the encoder was triggering a decoder edge case.

## 22. Future Work

Several directions are open for future development. The priorities are approximately:

**Improved pre echo suppression.** Our pre echo worst case is about 1.7 decibels behind the reference. Better gain control windowing, or a transient adaptive window switch, would close most of that gap. This is the single largest remaining perceptual difference.

**Tonal component coding.** ATRAC3 supports an explicit tonal component stream for sharp spectral features. Our current encoder does not actively emit tonals and relies on the spectral unit to code everything. Adding a production quality tonal extractor would reduce the coding cost for music with strong tonal content (synthesizers, voice, classical strings) and free up bits for the noise-like residual.

**Psychoacoustic masking across subbands.** Loud bands raise the masking threshold of their neighbours. A well tuned masking model would let the allocator spend fewer bits on subbands that are perceptually masked, reallocating them to more audible regions. This is a classic technique in perceptual coding and is a natural next step.

**Scale factor refinement with worst case error metric.** The current scale factor search minimizes mean squared error. A worst case error metric tends to produce slightly different scale factor choices that sound smoother. Combined with a trailing zero stripping pass (halve all mantissas and increase the scale factor correspondingly when all mantissas are even), this would reclaim a few more bits and improve the perceived quality of rough transients.

**Bit reservoir across frames.** Our current implementation treats each frame as an independent bit budget. A cross frame reservoir would let the encoder borrow bits from easy frames and spend them on hard frames, which is especially helpful for music with dynamic range variation. This is a standard technique in perceptual coders.

**Vectorized inner loops.** The quantization inner loop is currently a scalar tight loop. SIMD versions for the most common paths would give another one and a half to two times speedup.

**Fast MDCT.** The MDCT is currently a direct evaluation. Replacing it with an FFT based fast MDCT would reduce per frame work significantly.

**Bitrate support.** Only 132 kbps is actively tested today. The 66 kbps and 105 kbps modes exist in the container layer but the allocator tuning is not adapted for those rates. Retuning the initial assignment constants would enable them.

## 23. Building and Running

The project builds with a recent stable Rust toolchain. No external dependencies are required beyond the ones declared in `Cargo.toml`.

```
cargo build --release
```

The binary is produced at `target/release/at3cmp`.

To encode a WAV file to AT3:

```
./target/release/at3cmp proto-at3 \
    --input input.wav \
    --output output.at3 \
    --frames 999999 \
    --coding-mode vlc \
    --bitrate k132
```

The `--frames` parameter is a frame limit; set it to a very large number to encode the entire file. The `--coding-mode vlc` flag selects variable length codes, which is the recommended default. The `--bitrate k132` flag sets the output bitrate to 132 kbps.

To decode and measure:

```
psp_at3tool -d output.at3 decoded.wav
python digital_ears.py decoded.wav "my encoder" original.wav
```

Unit tests can be run with:

```
cargo test --lib
```

The test suite covers the transform stages (QMF and MDCT round trip), the bitstream writer, the container layer, and the quantization math. The full end to end encode is exercised through the binary in manual runs.

## 24. Repository Structure

The source is laid out as follows:

- `src/atrac3/bitstream.rs` contains the MSB first bit writer.
- `src/atrac3/container.rs` contains the RIFF AT3 container assembly.
- `src/atrac3/gain.rs` contains the gain control estimator.
- `src/atrac3/inspect.rs` contains debugging helpers for dumping frames.
- `src/atrac3/mdct.rs` contains the 256 point MDCT implementation.
- `src/atrac3/prototype.rs` is the top level encoder pipeline including the parallel map over frames.
- `src/atrac3/qmf.rs` contains the QMF analysis filterbank.
- `src/atrac3/quant.rs` contains the bit allocator, the quantization routines, and post-promotion. This file is the largest and is the core of the quality tuning.
- `src/atrac3/sound_unit.rs` contains the bitstream structures and their writers.
- `src/atrac3/synthesis.rs` contains the inverse transforms, used for round trip validation in tests.
- `src/bin/at3cmp.rs` is the command line frontend.

The docs directory contains research notes from the development process.

## 25. License

This project is released under a permissive open source license. See the LICENSE file for the full text.

The test material and reference numbers referred to in this document are used for benchmarking only and are not redistributed with the source. Any similar test material can be used to reproduce the benchmarks.

## Acknowledgements

This project stands on many shoulders. The ATRAC3 community work by the FFmpeg and libav developers, the academic work on perceptual audio coding going back to the early nineties, and the people who have preserved decades of music on MiniDiscs all contributed to the context in which this encoder was built. Specific thanks to everyone who listened to alternate encodes and gave honest feedback about how they sounded, which was the single most important input into the design.
