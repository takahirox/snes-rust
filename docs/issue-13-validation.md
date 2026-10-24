# Issue #13 shared DSP envelope rate validation

Validated on 2026-10-23 for [Issue #13](https://github.com/takahirox/snes-rust/issues/13),
against evaluation baseline `8f0c5ed2c8614fe8661eadcccdc914989197d904`.
The implementation advances one cyclic counter before each stereo sample's
voice processing and uses it for all three ADSR/dynamic-GAIN eligibility gates.
Envelope arithmetic and the existing unclocked direct-GAIN/release paths are
unchanged. No voice operation or register write resets that counter.

Issue #75 subsequently corrects the arithmetic/phase contract that this historical
record left unchanged. In particular, decay can enter sustain on a held candidate
before the committed envelope changes; the original tick-69/70/536 vectors below
are superseded by the [Issue #75 reconciliation](issue-75-validation.md).
The counter convention and rate-event vectors remain unchanged.

## Specification and counter convention

Sources consulted during implementation:

- [Fullsnes, SNES APU DSP ADSR/Gain Envelope](https://problemkaputt.de/fullsnes.htm):
  the VxADSR1/2 and VxGAIN entries define rate selection and arithmetic;
  **ADSR/Gain (and Noise) Rates** gives the 32 period values and notes that
  entering a phase need not coincide with a rate event. **SNES APU DSP Control
  Registers**, KON/KOFF and FLG, describes the five empty key-on samples,
  per-sample release and continued internal processing while muted.
- The currently served Fullsnes text does **not** give the exact shared-counter
  modulus or offsets. Those facts must not be attributed to its nominal-period
  table. [Anomie's S-DSP Doc, updated by jwdonal, revision 1212 (2015-09-28),
  COUNTERS](https://gist.github.com/nyanpasu64/a0d916ce6924912a7116682bf778e9a0#file-apudsp_jwdonal-txt-L260)
  is a reproduced hardware research document. Its COUNTERS section specifies
  initialization at zero, decrement from `$77ff` to zero, the period/offset tables
  and the modulo event condition. Its SOUND GENERATION timeline identifies the
  global counter update. Only the specification facts were used; no emulator
  implementation or alternate counter algorithm was imported.
- The Issue fixes this emulator's **sample-level ordering**: initialize `C=0`,
  then decrement before polling, `C=(C+30719)%30720`. This is an explicit
  convention for the existing sample API, not a claim of subcycle execution.

The fixed tick vectors follow from that convention: rate 2 events are
`1040, 2576, 4112, ...` (spacing 1536); rate 3 events are
`536, 1816, 3096, ...` (spacing 1280). Rate 29 fires at
`2, 5, 8, 11, ...`, rather than `3, 6, 9, ...`. At tick 30720 the counter
is zero; at 30721 it is 30719. The same boundary repeats at 61440/61441.
Rate 0 never fires, including at counter zero; rate 31 fires every sample.

## Original numerical evidence

- `every_rate_has_exact_positions_and_spacing_across_two_counter_wraps` samples
  the actual DSP through tick 61441, including silence and default FLG reset/mute.
  Each rate's **entire ordered event vector** is compared with an independently
  listed first-tick/spacing pair. Expected vectors use no production constants
  or helper. The test checks spacing and explicit initialization/wrap states,
  plus rate-2/3 vectors and rate-29 negative controls.
- DSP integration checks every attack and decay register-to-rate mapping,
  sustain, and all four dynamic GAIN modes with held values immediately before
  and after events. A keyed ADSR sequence goes from attack through decay to
  sustain: envelopes at ticks 5/6/7/8/9 are `0/1024/2047/2039/2039`, decay
  reaches 1791 at tick 70, and sustain holds until tick 536 changes it to 1784.
  Dynamic GAIN checks include both branches of the existing bent-increase
  arithmetic, without claiming hidden-envelope correctness.
- Staggered keys at ticks 501/525 share the tick-536 event. A voice keyed at 533
  misses it during its five-sample delay. Tests check rate-zero holds, rate-31
  updates, KOFF release, silence, FLG reset/mute and rate changes without phase
  restart. Existing direct-GAIN stereo/release/retrigger assertions are retained.
- The [original SPC700 guest](../crates/snes-core/tests/dsp_envelope.rs) is built
  in memory and uploaded through the existing IPL port protocol. It writes DSP
  registers via `$F2/$F3`, uses host commands via `$F4`, and stores ENVX pairs
  in RAM. Two voices start at rates 3/2; command 2 changes voice 0 to rate 2
  without KON. Absolute DSP ticks include upload and launch time:

  | DSP tick | ENVX voice 0 | ENVX voice 1 | Check |
  | --- | --- | --- | --- |
  | 540 | 2 | 0 | Rate-3 event at 536, before rate-2 event |
  | 1050 | 2 | 2 | Rate-2 event at 1040; change voice 0 to rate 2 |
  | 2100 | 2 | 2 | Reject old rate-3 event at 1816 and restarted rate-2 phase at 2090 |
  | 2585 | 4 | 4 | Shared rate-2 event at 2576 |

  Each acknowledgement has a 1,000-iteration limit; each checkpoint has a
  100,000-iteration limit, with 21 master clocks per iteration. Setup must finish
  before DSP tick 100. The guest must stop without a core error. Provenance and
  MIT redistribution decisions are in [PROVENANCE.md](../tests/roms/PROVENANCE.md#original-envelope-rate-spc700-guest-approved).
- The new APU test compares full DSP registers and **every PCM sample** between
  whole and irregularly partitioned ticks at 13 checkpoints through tick 61440.
  It checks numerical ENVX before/on/after rate-3 and rate-2 events, changes the
  rate at tick 537, crosses both counter wraps and requires nonzero finite PCM.
  The original one-second 32 kHz cadence/timer test is retained unchanged.

## Baseline failures

Before changing production scheduling, the new `advance` test helper and the two
focused integration tests were added to the baseline DSP file. These commands
both exited 101:

```sh
cargo test -p snes-core dsp::tests::gain_rate_three_first_event_is_tick_536 -- --exact --nocapture
cargo test -p snes-core dsp::tests::rate_29_updates_on_two_modulo_three_not_zero -- --exact --nocapture
```

The first failed at tick 536: actual envelope **0**, expected **32**. The second
failed at tick 6, just after key-on delay: actual **32**, expected **0**.
An initial short-name invocation with `--exact` selected zero tests; the fully
qualified commands above selected one test each and established the failures.

The DSP source was also temporarily replaced with the exact evaluation-baseline
file and `cargo test -p snes-core --test dsp_envelope --locked` was run. It exited
101 at tick 540: guest-recorded ENVX **[0, 0]**, expected **[2, 0]**. The fixed
source was restored before the final checks. All three tests pass with the fix.
No existing assertion, fixture hash, duration or provenance requirement was weakened.

## Validation commands and outcomes

Host: macOS arm64, native Rust/Cargo 1.94.0, rustup Rust 1.99.0 for the SPC
assembler and Wasm, Node 24.12.0, Python 3.13.13, Playwright 1.61.1 / Chromium.

| Command | Actual result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --workspace --locked` | Passed: 27 unit tests, 7 direct-color tests, 1 SPC envelope guest, 39 existing regressions; no ignored tests |
| `python3 tools/make-fixture.py .cache/smoke.bin` | Passed |
| `cmp tests/roms/smoke.bin .cache/smoke.bin` | Passed: byte-identical original diagnostic |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Passed: all three pinned source-built assemblers |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Passed: CPU full/basic and SPC suites, 600 frames each, expected terminal PCs/last-test markers 0649/0452/0557 |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Passed: 11 grayscale bands, 16 scanlines each, final shade through line 223 |
| `python3 tools/build-cleared-games.py` twice | Passed: fresh build directories, identical pinned Castle/Memory hashes |
| `npm --prefix web ci` | Passed |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Passed |
| `npx --prefix web playwright install chromium` | Passed |
| `cd web && SNES_TEST_PORT=18085 npm test -- --grep-invert '301 seconds'` | Passed: all 18 selected tests, no skips (2.6 minutes) |

The browser gate passed both 18,100-frame cleared-game replays, Castle's
five-map/credits route and freeze negative control, Memory's wins/restarts and
numerical/live Web Audio checks, diagnostic controls/audio/pause/reset/save
checks, and the Pages subpath smoke test. Optional unaudited game tests named
`301 seconds` were excluded as requested. No mandatory check remains failed,
skipped or unperformed.

The initial unqualified assembler build selected Homebrew Rust 1.94 and failed
on the tool's experimental syntax. The documented rustup compiler selection
above built it successfully without source changes. The initial browser run
could not bind occupied port 8080; the run above uses the README's alternate port.

## Fixture integrity and limits

All prior ROM licensing decisions, allowlists and inventory scopes remain in
force. Source-built fixtures retain their pins, font replacements and notices.
Measured SHA-256 values:

| Artifact | SHA-256 |
| --- | --- |
| Original smoke | `73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb` |
| CPU full | `1727e7c46ebb5df55711598264ba19005986bd34e915621383966adf674b3cfa` |
| CPU basic | `514d58950ae1ca940d6673c2a028b258b2ab17293bbc7cbd7cb54dded32abc5b` |
| SPC | `279d6a92d051ff8a98e190ce73e40482ff2cc981812509c7bca35a047484507e` |
| HDMA | `aef339e5d4a7319db31de577b253d977b45f5c45f09adfaa99a302ca300a957a` |
| Cleared Castle | `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869` |
| Cleared Memory | `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1` |

`dsp-envelope` remains **partial**. Exhaustive envelope transitions,
hidden-envelope/bent-GAIN behavior, live register interactions and DSP subcycle
sampling remain unverified beyond the tests described here. Noise generation,
pitch modulation and reference echo saturation remain outstanding. BRR,
interpolation, echo, SPC bus timing and other DSP behavior were not extended.
The prior CPU/bus, timing/DMA, PPU rendering/color/memory, SPC/upload/timer,
frontend/audio/save, cartridge/chip/region and compatibility categories retain
their documented scope and gaps in the [README](../README.md#implemented-features-and-limits)
and prior validation records. Memory's direct-GAIN audio remains preservation
evidence, not new ADSR-phase evidence. These checks establish neither full
hardware accuracy nor commercial compatibility and do not complete partial
Issue #3 work. No post-merge verification is required by Issue #13.
