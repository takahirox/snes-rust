# Issue #15 shared S-DSP noise validation

Implemented on 2026-10-23 for [Issue #15](https://github.com/takahirox/snes-rust/issues/15),
against evaluation baseline `63a136517e53e074dede3f47a74fd2040d583ae3`.
The production change adds one initialized noise state and consumes the existing
shared rate counter. NON chooses its held signed source before the existing
voice envelope, rounding, OUTX, stereo and EON pipeline. Pitch/BRR decoding and
ENDX handling still execute. There is no additional clock or public state API.

## Specification attribution and independent derivations

- [Fullsnes, SNES APU DSP Control Registers](https://problemkaputt.de/fullsnes.htm),
  **3Dh NON** defines shared noise, feedback, source substitution, pitch
  independence and continued BRR end behavior. **6Ch FLG** defines the rate bits,
  mute and voice reset; **ADSR/Gain (and Noise) Rates** supplies nominal periods.
- [Anomie's S-DSP Doc, updated by jwdonal, revision 1212](https://gist.github.com/nyanpasu64/a0d916ce6924912a7116682bf778e9a0),
  **COUNTERS** (lines 260–307) specifies the shared cyclic counter, initialization,
  offsets and rate eligibility. **SOUND GENERATION** (lines 98–249) specifies
  source selection before envelope/volume and the global sample timeline.
  **VxOUTX** specifies publication after the envelope and before voice volumes.
- Only hardware specification facts were used, without importing emulator
  implementation code. Fullsnes' nominal periods do not establish the exact
  offsets; their attribution remains Anomie/jwdonal. The sample API retains
  [Issue #13's convention](issue-13-validation.md#specification-and-counter-convention):
  initialize counter zero, decrement before polling. Noise updates once before
  voice selection. The Issue explicitly requires seed `$4000` on construction,
  distinct from FLG voice reset. This does not claim subcycle timing.

The independent test oracle uses a bit stream over GF(2): initialize bits
`b[0..13]=0, b[14]=1`, extend with `b[k+15]=b[k]+b[k+1] mod 2`, then pack
`sum(b[n+j]*2^j, j=0..14)` for update n. It does not call production noise or
rate helpers. Literal anchors for updates 0–16 are:

```text
4000 2000 1000 0800 0400 0200 0100 0080 0040
0020 0010 0008 0004 0002 4001 6000 3000
```

Thus feedback first sets bit 14 at update 14, and signed doubled sources at
updates 0/1/14/15/16 are `-32768,16384,-32766,-16384,24576`. Further literals
through update 30 include `$4003,$2001,$5000`, exercising both low-bit feedback
cases and further sign changes. A separate actual-generator run visits every
nonzero 15-bit state once and returns to `$4000` after exactly 32,767 updates.

Every rate has an independently listed first-tick/spacing pair. For an absolute
tick t, eligibility follows from the retained convention `C=(-t) mod 30720`.
Rate 2 fires at `1040,2576`, rate 3 at `536,1816`, and rate 29 at `2,5,8`.
All 32 actual generator runs compare every held state against the independent
sequence through tick 61441, across wraps at 30720 and 61440. Rate 0 holds;
rate 31 advances every sample, including reset/mute with no active voices.
A live 3→2 change at tick 537 still advances at 1040/2576 and rejects the old
1816 event. Holding rate 0 for two wraps then enabling rate 29 retains phase.

For signed arithmetic, double the state with 16-bit sign interpretation, multiply
by envelope/2048, round down to an even sample, publish its high byte, then
apply each signed voice/master volume with arithmetic division by 128. Literal
vectors include:

| State | Signed source | GAIN | Envelope sample | OUTX | PCM left/right (voice 127/-128, master 127) |
| --- | ---: | ---: | ---: | ---: | --- |
| 4000 | -32768 | 127 | -32512 | 129 | -32006 / 32258 |
| 2000 | 16384 | 127 | 16256 | 63 | 16002 / -16129 |
| 0002 | 4 | 127 | 2 | 0 | 0 / -2 |
| 4001 | -32766 | 127 | -32512 | 129 | -32006 / 32258 |
| 6000 | -16384 | 127 | -16256 | 192 | -16003 / 16129 |
| 3000 | 24576 | 127 | 24384 | 95 | 24003 / -24194 |
| 4003 | -32762 | 127 | -32508 | 129 | -32004 / 32254 |
| 5000 | -24576 | 64 | -12288 | 208 | -12097 / 12192 |

Zero GAIN is silent. EON buffer writes are checked before master volume, with
zero FIR/feedback and values below saturation. A simultaneous BRR/noise pair, using an original range-9 filter-0 `$44` loop,
with half envelopes yields samples `1024/8192`, mixed PCM `(2304,-1152)` at
voice volumes `64/-32`, master 64. Selecting only the noise voice for EON writes
`4096/-2048`; selecting only BRR writes `512/-256`. No echo saturation claim
is inferred from these bounded values.

## Coverage and original guest

Eight focused unit tests in `src/dsp_noise_tests.rs` cover the sequence/cycle,
all-rate advancement, live rates, signed arithmetic/EON, each individual NON bit,
all eight voices sharing one update, mixed BRR/noise, NON toggles, differing
pitches, looping and terminating BRR, ENDX and continuity. The zero looping BRR
source is original; at pitch `$1000` it reaches nibble 16 at tick 21 and loops
with ENDX at tick 22 after five empty KON samples. The non-looping variant
instead stops the noise with zero envelope/OUTX at tick 22. Pitches `$1000` and
`$3000` produce identical noise while decode positions continue independently.
A BRR-only control has identical progression and ENDX to its noise counterpart.

The continuity run compares every state through ticks 1–300 while toggling NON,
muting/unmuting, releasing via KOFF, completing release, retriggering and applying
FLG voice reset. It asserts five silent samples per KON, per-sample release by
8 for 254 samples from envelope 2032, completed silence and reset silence.
Fresh DSP, fresh APU, fresh Machine and Machine reset reproduce initialization;
the Machine test also checks signed PCM through initial feedback/sign changes.

The [original SPC700 guest](../crates/snes-core/tests/dsp_noise.rs) uploads a
155-byte program through the existing firmware-free IPL protocol, verifies
uploaded bytes, configures NON/FLG via `$F2/$F3`, stores OUTX in RAM and stops.
The data is an original zero BRR loop and directory. Absolute DSP ticks include
upload/launch; setup must finish before tick 100. Commands take snapshots away
from rate events and change rate 3→2 without KON at tick 550:

| Command | Tick | RAM OUTX | Meaning |
| --- | ---: | ---: | --- |
| 1 | 540 | 63 | First noise event at 536 |
| 2 | 550 | 63 | Live rate change, same held source |
| 3 | 1050 | 31 | Next shared event at 1040 |
| 4 | 2100 | 31 | Reject old rate-3 event at 1816 or restarted rate-2 phase |
| 5 | 2585 | 15 | Next shared rate-2 event at 2576 |

Each acknowledgement is limited to 1,000 iterations; each checkpoint to
100,000 iterations, at 21 master clocks each. ENVX stays 127. The guest must
halt without errors, and all PCM must be finite/bounded. Source/data and MIT
redistribution are recorded in [PROVENANCE.md](../tests/roms/PROVENANCE.md#original-noise-spc700-guest-and-brr-data-approved).
No firmware, downloaded binary, commercial ROM or external assembler is needed.

APU integration compares every PCM sample and all 128 DSP registers between
whole and irregularly partitioned master-clock advances at 16 checkpoints
through tick 61441. It changes rate 3→2 at tick 537, mutes at 1041, unmutes at
2577, then switches to rate 31 at 2578. Literal OUTX/PCM assertions include:

| Tick | OUTX | PCM left/right |
| --- | ---: | --- |
| 535 | 129 | -32006 / 32258 |
| 536 | 63 | 16002 / -16129 |
| 1040 | 31 | 8001 / -8065 |
| 2576 | 15 | 0 / 0 (muted, generator continued) |
| 2578 | 15 | 4000 / -4033 |
| 30719 | 213 | -10826 / 10910 |
| 30720 | 105 | 26590 / -26801 |
| 30721 | 181 | -18713 / 18859 |
| 61439 | 121 | 30506 / -30748 |
| 61440 | 189 | -16755 / 16885 |
| 61441 | 222 | -8379 / 8443 |

These late checkpoints use the independent bit-stream sequence at update count
`3 + tick - 2578`; early checkpoints use the three independently scheduled
536/1040/2576 events. They are literals in the test, not recorded emulator output.
All samples are required finite with absolute value at most 1, and no core error
is allowed. Matching runs or non-silence alone would not satisfy this check.

## Baseline failures

Before changing production code, running
`cargo test -p snes-core --test dsp_noise --locked` with the initial public-API
reproduction selected one test and exited 101: actual PCM `(0,0)`, expected
`(-32006,32258)` after five silent key-on samples.

After all four integration tests were written, the production DSP file was
temporarily replaced with the exact baseline file from
`git show 63a136517e53e074dede3f47a74fd2040d583ae3:crates/snes-core/src/dsp.rs`.
The same command exited 101 with **all four tests failing**:

- Reproduction: PCM `(0,0)` rather than `(-32006,32258)`.
- Reset test: sixth sample `(0,0)` rather than `(-32006,32258)`.
- Uploaded guest at tick 540: RAM OUTX 0 rather than 63.
- Partition check at tick 535: ENVX/OUTX `[127,0]` rather than `[127,129]`.

The fixed file was restored in a `finally` block before final workspace checks.
All four integration tests and all eight unit tests pass with the implementation.
No prior test, fixture pin/hash, replay duration or assertion was weakened.

## Validation commands and results

Host: macOS arm64, native Rust/Cargo 1.94.0, rustup Rust 1.99.0 for the SPC tool
and Wasm, Node 24.12.0, Python 3.13.13, Playwright 1.61.1 / Chromium.

| Exact command | Actual result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --workspace --locked` | Passed: 35 unit, 7 direct-color, 1 envelope guest, 4 noise integration, 39 existing regression tests; no ignored tests |
| `cargo test -p snes-core --test dsp_noise --locked` | Passed: 4 tests |
| `python3 tools/make-fixture.py .cache/smoke.bin` | Passed |
| `cmp tests/roms/smoke.bin .cache/smoke.bin` | Passed: byte-identical original diagnostic |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Passed: all three pinned source-built assemblers |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Passed: full/basic CPU and SPC, 600 frames each; terminal PCs 0081a2/0081a2/008060, last-test markers 0649/0452/0557 |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Passed: 11 grayscale bands, 16 scanlines each; final shade through line 223 |
| `python3 tools/build-cleared-games.py` (twice) | Passed: fresh builds, identical pinned Castle/Memory hashes |
| `npm --prefix web ci` | Passed, no vulnerabilities reported |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Passed |
| `cd web && npx playwright install chromium` | Passed |
| `cd web && SNES_TEST_PORT=18085 npm test -- --grep-invert '301 seconds'` | Passed: all 18 selected tests, no skips (2.5 minutes) |

The browser run retains both 18,100-frame (301.17 emulated seconds) cleared-game
replays, Castle's full five-map/credits route and freeze negative control,
Memory's numerical music/effect and live Web Audio checks, diagnostic
controls/audio/pause/reset/save checks, and Pages subpath smoke checks. Only
optional unaudited games named `301 seconds` were excluded. No mandatory check
remains failed, skipped or unperformed.

## Fixture integrity and limits

All measured source-built fixture hashes match the existing
[Issue #13 hash table](issue-13-validation.md#fixture-integrity-and-limits):
smoke `73c0bad1…b82fb`, CPU full `1727e7c4…b3cfa`, CPU basic `514d5895…bc5b`,
SPC `279d6a92…507e`, HDMA `aef339e5…957a`, cleared Castle `197a1763…c869`,
and cleared Memory `6b57b344…94b1`. Original diagnostic bytes, licensed source
pins, font replacements, notices and cleared-game assertions/durations are
unchanged. Generated fixtures and browser evidence remain ignored artifacts.

Noise remains **partial**: subcycle NON/FLG sampling, register-write races and
interactions with still-unimplemented pitch modulation remain outstanding.
SPC bus timing, hidden-envelope corrections and reference echo saturation were
not extended. All other 24 inventory categories retain their supplied scope,
status and outstanding work; their README and historical validation statements
remain in force. Existing envelope/ENDX/game tests provide preservation evidence,
not new proof of those unextended features. Limited passing tests establish
neither full hardware accuracy nor commercial compatibility and do not complete
historical partial Issue #3 work. No post-merge verification is required.
