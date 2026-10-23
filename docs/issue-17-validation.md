# Issue #17 sample-level S-DSP pitch modulation validation

Implemented on 2026-10-23 for [Issue #17](https://github.com/takahirox/snes-rust/issues/17),
against baseline `5d4e89486b09ba130c6b8eb2a7159eccb4f1dc20`.
The production change is confined to `src/dsp.rs`: ordered adjacent-voice routing,
signed arithmetic and retaining group bits in the existing scalar phase. PMON
writes use the existing sample boundary and register storage. No scheduler,
public state API, frontend behavior or per-game exception was added.

## Specification attribution and independent derivations

- [Fullsnes, SNES APU DSP BRR Pitch](https://problemkaputt.de/fullsnes.htm):
  **x2h/x3h VxPITCHL/H**, **2Dh PMON**, **Pitch Counter** and **4-Point Gaussian
  Interpolation** describe pitch masking, adjacent voices, pre-volume modulation
  and coefficient indexing. Its counter discussion leaves high-rate cropping
  unresolved; the explicit bound comes from Anomie/jwdonal below.
- [Anomie's S-DSP Doc, jwdonal revision 1212](https://gist.github.com/nyanpasu64/a0d916ce6924912a7116682bf778e9a0):
  **SOUND GENERATION**, S3a/S3c/S4, places modulation after the predecessor's
  source/envelope and before voice volume. **PITCH ADJUSTMENTS** specifies the
  two signed shifts and `$7FFF` saturation; **BRR DECODING** explains four-sample
  groups. The Issue's explicit sample-level contract governs this implementation.
  In particular, although the document's older pitch pseudocode gates on NON,
  the Issue requires the underlying BRR position of noise-selected targets to
  advance with PMON. Their audible source remains shared noise. Subcycle order
  and speculative decode/register races are not inferred from these tests.
- Only specification facts and coefficient data are used; no emulator code or
  external fixture was imported. Original test source/data are MIT, as recorded
  in [provenance](../tests/roms/PROVENANCE.md#original-pitch-modulation-spc700-guest-and-brr-data-approved).

For base pitch p, divide the signed even predecessor output by 32 rounding toward
negative infinity, multiply by p, divide by 1024 with the same rounding, then
add p. The literal arithmetic vectors are:

| p | predecessor | shifted predecessor | increment |
| --- | ---: | ---: | --- |
| $1000 | 0 | 0 | $1000 |
| $1000 | -32768 | -1024 | $0000 |
| $1000 | -32512 | -1016 | $0020 |
| $1000 | 32766 | 1023 | $1FFC |
| $1001 | -2 | -1 | $0FFC |
| $3FFF | 32766 | 1023 | $7FEE |
| $1001 | 34 | 1 | $1005 |
| $1001 | -34 | -2 | $0FF8 |

For example, `-4097/1024` rounds down to -5, giving `$1001-5=$0FFC`;
`1023*16383=16759809`, whose floor division by 1024 is 16367, giving 32750
(`$7FEE`). Zero pitch always gives zero. Signed endpoint vectors test arithmetic;
they do not claim all endpoints are reachable through every envelope.

Let q be the old interpolation position with its group carry removed. The new
position is `min(q+increment,$7FFF)`, with `q=old & $3FFF`. The scalar decoder
already consumed old integer steps, so it consumes `floor(new/4096)-floor(q/4096)`
additional nibbles and interpolates at `new & $FFF`. This retains the group's
integer coordinate rather than repeatedly reducing to a fractional phase. With
PMON disabled the addition cannot saturate and this difference is exactly the
old scalar consumption count. At `$3FFF+$7FEE`, saturation gives `$7FFF`, four
new nibbles (7-3), rather than eleven unrestricted steps. The next saturated
sample consumes another four, without duplicating the previous group. The test
asserts both decoded windows, PCM `(127,-255)` then `(383,-767)`, and OUTX 1/5.
A zero masked pitch then retains `$3FFF` and consumes nothing. Upper PITCHH
bits stay readable as written.

## Numerical routing and progression

Nine focused unit tests in `src/dsp_pitch_tests.rs` cover:

- Each enable bit 1–7, ignored bit 0, no wrap to voice 0 and unaffected unselected
  voices. Constant decoded 1024 at half envelope gives output 1024; pitch `$1000`
  becomes `$1080` only for the selected target. Phases and nibble counts are checked.
- Adjacent chains and a silent intervening voice. A moving interpolation impulse
  produces current outputs 184 then zero; target increments are `$1014` then
  `$1000`. The next adjacent voice receives the intermediate voice's current
  output, and its zero output breaks the following link.
- Full-resolution inputs sharing OUTX=4: constant decoded 1024 and 1058 yield
  envelope outputs 1024 and 1058, hence increments `$1080/$1084`. Zero, positive
  and negative predecessor stereo volume and FLG mute preserve target progression.
- Inactive, key-delay, zero envelope and retriggered predecessors supply zero,
  including between two nonzero voices. Release contributes actual output:
  envelope 1024 falls to 1016 and the next target receives increment `$107C`.
- Live PMON enable/disable with retained phase, envelope, key-delay, BRR nibble
  count and global counter. High increments, saturation and zero/masked pitch.
- Noise predecessor and noise target: held source 16384 with half envelope gives
  8192 and increment `$1400`. Nibble 16 is reached at tick 13; tick 14 either loops
  to nibble 1 with ENDX=2 or terminates with zero ENVX/OUTX. An advancing rate-31
  control compares audible noise and counter state while toggling PMON, checks
  literal noise states through feedback/sign changes and distinguishes underlying
  target phases `$1400/$1000` at the first tick. Existing noise tests are unchanged.

The public-API two-BRR reproduction in `tests/dsp_pitch.rs` silences voice 0 only
at its stereo volumes. After nine samples (five delay plus four decoded samples),
its constant range-12/filter-0 -8 nibbles give Gaussian output 32752 via the
existing three-tap signed wrap. Half envelope gives 16376, hence step `$17FC`.
The target pitch is held zero during settling, then set to `$1000` in both runs.
Four enabled steps total `$5FF0`, consuming five ramp nibbles with fractional
index 255. The decoded window `[256,512,768,1024]` and coefficients
`[0,374,1305,370]/1024` produce `187+978+370=1535`, rounded even to 1534;
half envelope gives 766 after even truncation. Voice volumes 64/-128 and master
64 give PCM `(191,-383)`, OUTX=2. Disabled playback consumes four nibbles at phase
zero, source 512, envelope 256, PCM `(64,-128)`, OUTX=1. These are independent
calculations, not captured emulator outputs or calls to production helpers.

## Original guest and APU integration

The 163-byte original SPC guest configures a held noise predecessor and a
non-looping ramp target through `$F2/$F3`, including PMON before KON. Seed noise
-32768 with GAIN=127 gives -32512 and target increment `$20`. Before absolute
tick 100, even 100 advances total less than `$1000`, so no target nibble can have
been consumed: OUTX/ENDX are 0/0. A PMON-disabled control reaches its seventeenth
nibble/end within 17 active samples and records 0/2 instead. Setup must finish
before tick 50, allowing ample time for that control to end.

Commands at ticks 100/110/140 record OUTX/ENDX in `$1000..$1005`. Command 2
writes PMON=1 (ignored bit 0) without KON, initially recording the same ENDX.
Within the following 30 samples independent pitch ends the enabled target too,
so command 3 records 0/2 in both runs. PMON readback and the unchanged KON
register are asserted. Upload bytes are verified. Each acknowledgement is bounded
by 1,000 calls of 21 master clocks, each checkpoint by 100,000 calls; total sample
count must stay below 150, STOP must execute, and no core error is permitted.
Every PCM value must be finite and normalized. No firmware or assembler is used.

A separate APU test uses the looping ramp and the same held noise predecessor.
With step `$20`, one nibble advances per 128 output samples after five silent
KON samples. At phase zero, independently derived checkpoints are:

| DSP tick | OUTX | PCM left/right | ENDX |
| --- | ---: | --- | ---: |
| 6 / 133 / 261 | 0 | 0 / 0 | 0 |
| 389 | 0 | 11 / -23 | 0 |
| 517 | 1 | 64 / -128 | 0 |
| 518 | 2 | 128 / -256 | 0 |
| 519 | 3 | 192 / -384 | 0 |
| 647 | 4 | 256 / -512 | 0 |
| 656 | 253 | -192 / 384 | 0 |
| 657 | 253 | -129 / 257 | 2 |

After tick 517 PMON is disabled, after 519 reenabled, after 647 disabled again,
without KON. For example tick 389 consumes the third nibble: `[0,0,256,512]`
produces source 92, envelope 46 and PCM `(11,-23)`. Tick 657 crosses the loop:
`[-768,-512,-256,0]` produces signed floors `-278-653-94=-1025`, even -1026,
envelope -514, PCM `(-129,257)`. The test compares **every PCM sample and all
128 DSP registers** against irregular chunks `[1,7,666,1001,21,8193]` at each
boundary. Identical partition runs alone are not the numerical oracle.

## Baseline failure and fixed result

The final public reproduction was run against the exact baseline DSP file:

```sh
git show 5d4e89486b09ba130c6b8eb2a7159eccb4f1dc20:crates/snes-core/src/dsp.rs
cargo test -p snes-core --test dsp_pitch --locked original_brr_pair_pmon_reproduction
```

The `git show` bytes were temporarily written to `src/dsp.rs` by a Python
`try/finally` harness, which restored the fixed file before subsequent checks.
Actual baseline result: exit 101, enabled PCM `(64,-128)` instead of expected
`(191,-383)`. The disabled assertion passed. After restoration the focused
reproduction and all three pitch integration tests pass. Initial draft-vector
arithmetic errors were corrected by explicit Gaussian/rounding calculations;
the final failure above uses the reviewed independent expectations.

## Validation commands and results

Host: macOS arm64; native Rust/Cargo 1.94.0, rustup Rust 1.99.0 for the SPC tool
and Wasm; Node 24.12.0, Python 3.13.13. Generated logs/artifacts stay ignored in
`.cache/issue-17`, `.cache/hardware-tests`, `.cache/ppu-test` and browser results.

| Exact command | Actual result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --workspace --locked` | Passed: 44 unit, 7 direct-color, 1 envelope, 4 noise, 3 pitch and 39 existing regression tests; no ignored tests |
| `cargo test -p snes-core --test dsp_pitch --locked` | Passed: all 3 tests, including uploaded guest and APU partitions |
| `python3 tools/make-fixture.py .cache/smoke.bin` | Passed |
| `cmp tests/roms/smoke.bin .cache/smoke.bin` | Passed: byte-identical |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Passed: all three pinned assemblers |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Passed: all three CPU/SPC suites, 600 frames each; terminal PCs 0081a2/0081a2/008060, last-test markers 0649/0452/0557 |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Passed: 11 grayscale bands, 16 scanlines each; final shade through line 223 |
| `python3 tools/build-cleared-games.py` (twice) | Passed: fresh builds, identical pinned hashes |
| `npm --prefix web ci` | Passed: no vulnerabilities reported |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Passed |
| `npx --prefix web playwright install chromium` | Passed |
| `cd web && SNES_TEST_PORT=18085 npm test -- --grep-invert '301 seconds'` | Passed: all 18 selected tests, no skips, 2.5 minutes |

The browser gate retains both cleared 18,100-frame (301.17 emulated seconds)
replays, Castle's five-map/credits traversal and freeze negative control,
Memory's numerical music/effect and live Web Audio checks, diagnostic controls,
audio, pause/reset and save checks, and Pages subpath checks. Only the optional
unaudited games named `301 seconds` were excluded. No mandatory check remains
failed, skipped or unperformed.

All generated fixture hashes match the existing
[Issue #13 table](issue-13-validation.md#fixture-integrity-and-limits): original
smoke `73c0bad1…b82fb`, CPU full `1727e7c4…b3cfa`, CPU basic `514d5895…bc5b`,
SPC `279d6a92…507e`, HDMA `aef339e5…957a`, cleared Castle `197a1763…c869`,
cleared Memory `6b57b344…94b1`. No existing source pin, hash, assertion, font
replacement, licensing decision or replay duration was changed.

## Scope retained

Sample-level PMON and the tested high-rate bound are implemented. Pitch and noise
coverage remain partial: DSP subcycle register sampling, writes between voice
slots, torn pitch-register writes, SPC bus timing and register/RAM races remain
outstanding. Exhaustive BRR filters/interpolation phases, hidden-envelope
corrections and reference echo saturation remain unverified. All other inventory
categories retain their supplied scope and outstanding work; preservation checks
are not new proof of full accuracy. No historical partial Issue #3 requirement,
full DSP accuracy or commercial compatibility is claimed. No additional ROM or
firmware was acquired; no post-merge verification is required.
