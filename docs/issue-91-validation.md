# Issue #91: preceding-envelope output and snapshots

Implemented for [Issue #91](https://github.com/takahirox/snes-rust/issues/91).
Evaluation baseline: `9c7d3f08a8fb6d4d36bb1ef1226ee2415d6efab1`.
The sample API now captures voice output and ENVX from the preceding committed
envelope, then advances the next envelope. KON's fifth silent call initializes
that envelope without adding silence. This was incomplete baseline behavior,
not a regression attributed to #89.

## Source reconciliation and implementation boundary

Inspected on 2026-10-24:

- [Anomie/jwdonal revision 1212](https://gist.github.com/nyanpasu64/a0d916ce6924912a7116682bf778e9a0#file-apudsp_jwdonal-txt),
  **SOUND GENERATION**, S3c and S6–S9; **BRR DECODING**, KON sequence #1–#6;
  **VxADSR/VxGAIN** and **COUNTERS**. These establish envelope application before
  update, the output feeding adjacent pitch modulation, and envelope processing
  beginning during the fifth zero sample.
- [Fullsnes ADSR/GAIN](https://problemkaputt.de/fullsnes.htm#snesapudspadsrgainenvelope),
  rates, arithmetic and release; **BRR Samples / Gaussian Interpolation** for
  the signed constant sources. The [#75 reconciliation](issue-75-validation.md)
  remains authoritative for this implementation's hidden candidates, unsigned
  bent-GAIN comparison and phase tracking; those rules are unchanged.
- Shay Green's research implementation, mirrored
  [`SPC_DSP.cpp` at immutable snes9x revision `1bcc369e89f08243e0a462882fb1f3e42e51de3a`](https://github.com/snes9xgit/snes9x/blob/1bcc369e89f08243e0a462882fb1f3e42e51de3a/apu/bapu/dsp/SPC_DSP.cpp),
  **V3c**, **V7–V9**. V3c corroborates capturing output/ENVX before running
  the envelope for the next sample; the latter stages publish the captured
  values. This was read as research evidence only. No implementation was
  imported, built or run, and no output trace supplied expected values.

For an ordinary active call, `E_apply` is the preceding committed envelope.
`O=((source*E_apply)>>11)&!1`. ENVX is `E_apply>>4`, OUTX is O's high byte.
The same O feeds adjacent-voice PMON, voice-volume scaling, ordered dry mixing
and EON-selected sends. `update_envelope` then evaluates the next candidate from
E, updates H/phase even on rate holds, and commits `E_next` only on an event
(or unconditionally for release/direct GAIN). H is a pre-clamp candidate, never
the applied envelope. Release retains H and commits `max(E_apply-8,0)`.

Register snapshots are published synchronously at the end of each sample call.
This convention does not establish physical S7/S8/S9 access timing. Existing
scalar terminal-header recognition still forces immediate zero/release before
output; FLG reset still deactivates/zeros the voice. These bounded conventions
are retained rather than being interpreted as physical header lookahead, final
pre-KON output retention, or reset retention corrections.

| KON call | Decoder / position | Envelope and externally visible result |
| --- | --- | --- |
| 1 | Select stable start word, no decode | E=H=0, PCM/ENVX/OUTX=0 |
| 2 | Decode nibbles 0–3, hold position | No envelope update, publish zero |
| 3 | Decode nibbles 4–7, hold position | No envelope update, publish zero |
| 4 | Decode nibbles 8–11, select window 0–3 | No envelope update, publish zero |
| 5 | Hold initialized window and position zero | Apply/publish zero, perform initial envelope update |
| 6 | Window 0–3 at phase zero | Apply call 5's E_next, capture O/ENVX, update envelope again |
| 7 onward | Retain #89's pitch/window progression | Apply preceding E, advance next E |

An initial terminal `01` still selects zero/release while priming; it cannot
revive on call five. Twelve primed nibbles do not complete a block or set ENDX.
[#75](issue-75-validation.md#implemented-operation-order) and
[#89](issue-89-validation.md#hardware-stages-and-this-cores-call-convention)
mark their superseded output-order and initialization statements while retaining
historical evidence. Stable directory/data selection throughout KON is assumed.

## Independent public vectors

[`dsp_envelope_order.rs`](../crates/snes-core/tests/dsp_envelope_order.rs)
contains nine original tests. All expected PCM/register values are literal
specification calculations, independent of production arithmetic, private DSP
state, production Gaussian coefficients and emulator output captures.

NON with FLG rate zero holds seed $4000, hence source=-32768. With both voice
and master volumes 64, O=-16E and PCM=-4E on each channel. OUTX is the unsigned
representation of O's high byte. The live-change anchors are:

| Before / operation | First call after write | Following call |
| --- | --- | --- |
| E=1504, direct 94→96 | PCM -6016, ENVX 94, OUTX 162; commit 1536 | -6144, 96, 160 |
| E=1504, held H=1536, bent rate 31 | -6016, 94, 162; commit 1512 | -6048, 94, 161; commit 1544 |
| E=16, KOFF | -64, 1, 255; commit 8 | -32, 0, 255; commit zero; next call zero |
| E=1024, linear decrease rate 31 | -4096, 64, 192; commit 992 | -3968, 62, 194 |
| E=1024, exponential decrease rate 31 | -4096, 64, 192; commit 1020 | -4080, 63, 192 |
| E=1024, linear/bent increase rate 31 | -4096, 64, 192; commit 1056 | -4224, 66, 190 |

Rate-zero writes hold each newly committed dynamic value for four checked calls.
Rate 29 has events at absolute 2,5,8,11 with this core's initial counter phase.
GAIN $DD therefore commits 32 on silent call five. Calls 6/7/8 output -128,
ENVX 2, OUTX 254; call 8 commits 64, calls 9/10/11 output -256, ENVX 4,
OUTX 252. Call 12 outputs -384. This rejects deferral and an extra silent call.

Fast ADSR $FF/$C3 commits 1024 on call five, 2047 on six. Call seven applies
2047 and holds rate-30 decay; call eight still applies 2047 while committing
2039. Call nine applies 2039, PCM -8156. Held decay candidates enter sustain
at call 69 with E=1799, PCM -7196, ENVX 112, OUTX 143. Sustain rate 3's event
at tick 536 still outputs that value; tick 537 applies 1791, PCM -7164, ENVX
111, OUTX 144. A separate live slow attack ($81, rate 3) holds E=1504 through
536, then applies 1536 on 537. Holds before/on/after events are asserted exactly.

Constant filter-0 BRR range 10 nibbles +4/-4 decode +2048/-2048. Phase-zero
taps `[370,1305,374,0]/1024` give source +4098/-4098. At E=1024, O=2048/-2050,
PCM=512/-513, OUTX=8/247. A direct write to 96 first retains those outputs,
then E=1536 gives O=3072/-3074, PCM=768/-769, OUTX=12/243. Zero pitch holds
waveform position throughout, separating envelope order from waveform changes.

Release is checked through zero while voice 1 continues independently on right
PCM -4096, ENVX/OUTX 64/192. Voice 0's retrigger retains exactly five zero
left samples, then -6144, ENVX/OUTX 96/160, without disturbing voice 1.

Focused consumers use the same changing envelope:

- EON, FIR/EFB zero: direct 94→96 yields send words -12032 then -12288,
  alongside dry PCM -6016 then -6144 and matching OUTX. Both stereo RAM words
  are checked, preserving even writeback.
- PMON: voice 0 is held noise at E=1024 with zero voice volumes. Its first
  post-write O remains -16384 when GAIN changes to zero, so voice 1's $1000
  pitch increment is $800; its constant first window at bin 128 produces PCM
  512. Next O0=0 gives increment $1000, window 1/bin128. Signed BRR values
  `[2048,2048,2048,-2048]` and taps `[56,965,969,58]/1024` give source
  `112+1930+1938-116=3864`, O1=1932, PCM 483, OUTX 7. The following window
  produces source -12, O1=-6, PCM -2, OUTX 255. All snapshots are checked.

## Bounded original uploaded SPC700 guest and partitions

The same test file builds original instruction bytes in memory, uploads fewer
than 512 bytes at $0200 using the firmware-free protocol, checks the uploaded
bytes, and requires ready before PCM sample 100. Eight host commands record
ENVX/OUTX pairs at $1100–$110F through actual $F2/$F3 access:

| Checkpoint | Expected PCM / recorded ENVX,OUTX | Action after record |
| ---: | --- | --- |
| 100 | -6016 / 94,162 | Direct GAIN 96 |
| 200 | -6144 / 96,160 | Linear rate-zero hold |
| 300 | -6144 / 96,160 | Bent rate 31 |
| 400 | -8188 / 127,128 | Direct GAIN 1 |
| 500 | -64 / 1,255 | KOFF |
| 600 | 0 / 0,0 | Clear KOFF, GAIN 94, retrigger |
| 30719 | -6016 / 94,162 | Direct GAIN 96 across counter wrap |
| 30731 | -6144 / 96,160 | STOP |

The preceding twenty stereo samples at ordinary checkpoints are required to
match the literal PCM value. At 30731 the checked stable window starts at
30724, allowing the preceding command's record/write instructions to complete.

For command one, the host stops at the generated address of `MOV $F3,#96`
**before** execution, after $F2 selects GAIN. This core applies instruction
effects before advancing its five whole clocks. The test records the number
of completed PCM samples at that point; the first subsequent sample must be
-6016 and the second -6144. This expectation follows the instruction-effect
convention and the independently derived envelope relationship; it is not
calibrated from observed PCM. No physical write-edge timing is claimed.

Two APUs run whole intervals versus irregular `[1,7,666,1001,21,8193]` master
clock chunks to `ceil(N*32*21477272/1024000)`. Upload and bounded command waits
use identical short ticks. Comparisons after upload, checkpoints, the precise
first write and follow-ups include every accumulated PCM sample, all 128 DSP
registers, all 64 KiB RAM (including guest results), and all output ports. The
long interval after retrigger reaches sample 30719; the following live-change
interval crosses the shared 30,720-sample counter wrap. Both guests must STOP without error. Every
acknowledgement has a 1000×21-master-clock bound. Partition equality establishes
determinism only; numeric anchors provide separate behavior evidence.

Source/data authorship and MIT terms are recorded in
[PROVENANCE.md](../tests/roms/PROVENANCE.md#issue-91-original-envelope-ordering-vectors-and-spc700-guest).

## Reconciled existing discriminators

No case is removed, ignored or weakened. Recalculations follow the applied /
next relationship above:

- The private hidden-envelope suite still checks every original E_next/H
  candidate, underflow/overflow, unsigned comparison, hold oscillation, phase
  transition and reset/reconstruction case. Its helper checks PCM/ENVX/OUTX
  from preceding E using the independent held-noise equations, retaining the
  original next-envelope numeric literals. Retrigger call five commits 32 and
  call six commits 64; release from 64 now checks all eight steps through zero.
- Private keyed ADSR vectors distinguish committed from applied E. Rate 29's
  committed sequence gains the call-five event. Rate-3/2 snapshot events in DSP
  and APU tests publish on the following call; counter event positions remain
  unchanged. The public hidden-envelope partition table retains all writes,
  adds the delayed consumed values and retains every PCM/register comparison.
- Existing uploaded hidden-envelope evidence now checks PCM -6016 on the first
  bent call (101) and -6048 on the following call (102); the first ENVX read
  remains 94 because both 1504 and 1512 shift to 94.
- Noise/PMON/echo private fixtures that directly construct active voices now
  seed their stated settled E_apply (1024, or direct GAIN×16) explicitly.
  Their source, rounding, phase, echo feedback and saturation literals remain.
  The PMON release fixture's bin-8 source is 2046, O=1022 at E=1024; its
  increment remains $107C. The zero-envelope case explicitly seeds E=0.
- Terminal-BRR release tests use `1024-8*(call-1)` for applied E, including
  call 129's zero, rather than omitting the preceding E=1024 call. The noise
  lifecycle test retains committed zero at 269 but starts zero-output checks
  at 270. Tick 269 explicitly applies E=8 to independently derived noise
  state $5801 (source -20478), O=-80, PCM -20, ENVX 0, OUTX 255. Scalar terminal silence and all terminal/loop/ENDX cases remain.
- KON preload, live directory, public PMON/noise, ordered mixing and public
  echo targets pass with their existing expectations and assertions intact.

Initial workspace runs exposed obsolete same-call and directly constructed
voice assumptions. Those failures and an initial too-early guest stable-window
checkpoint were corrected using the derivations above. Review also corrected a
provisional tick-269 noise literal using the independent bit-stream recurrence;
the final source/output values are recorded above. Final results follow.

## Exact-baseline behavioral evidence

Only the new public file was copied into a detached exact-baseline worktree;
production baseline files were unchanged:

```sh
git worktree add --detach .cache/issue-91/baseline 9c7d3f08a8fb6d4d36bb1ef1226ee2415d6efab1
cp crates/snes-core/tests/dsp_envelope_order.rs .cache/issue-91/baseline/crates/snes-core/tests/dsp_envelope_order.rs
CARGO_TARGET_DIR="$PWD/.cache/issue-91/baseline-target" cargo test \
  --manifest-path .cache/issue-91/baseline/Cargo.toml --locked \
  -p snes-core --test dsp_envelope_order
```

All nine compiled and failed behaviorally, zero passed/ignored:

| Test / discriminator | Baseline actual | Required |
| --- | --- | --- |
| Direct 94→96 first call | (-6144,-6144) | (-6016,-6016) |
| Dynamic linear decrease first call | (-3968,-3968) | (-4096,-4096) |
| Startup rate 29, call six | (0,0) | (-128,-128) |
| ADSR decay event, call eight | (-8156,-8156) | (-8188,-8188) |
| KOFF E=16 first call (voice 1 right) | (-32,-4096) | (-64,-4096) |
| Positive BRR direct change first call | (768,768) | (512,512) |
| EON direct change first call | (-6144,-6144) | (-6016,-6016) |
| PMON source ENVX/OUTX, target OUTX | 0,0,8 | 64,192,8 |
| Guest first two post-write PCM frames | -6144, -6144 | -6016, -6144 |

All nine pass with the implementation. Compilation errors and external emulator
traces are not behavioral evidence.

## Mandatory validation actually executed

Host: macOS arm64; native Rust 1.94.0, rustup stable Rust 1.99.0 for spcasm
and Wasm via the README `RUSTC` override; Node 24.12.0, Python 3.13.13.
Raw local logs are under `.cache/issue-91`; durable evidence is recorded here.

| Command | Actual result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `git diff --cached --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --locked -p snes-core --test dsp_envelope_order` | Passed: 9 tests, zero failed/ignored |
| `cargo test --workspace --locked` | Passed: 457 tests, zero failed/ignored |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Passed: cc65, bass-untech and spcasm built from unchanged pins |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Passed: full/basic CPU and SPC suites, 600 frames each, markers 0649/0452/0557 |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Passed: 11 grayscale bands of 16 scanlines, final shade through line 223 |
| `python3 tools/make-fixture.py .cache/smoke.bin` then `cmp tests/roms/smoke.bin .cache/smoke.bin` | Passed: byte-identical |
| `python3 tools/build-cleared-games.py` twice | Passed: both builds reproduce unchanged expected Castle/Memory hashes |
| `npm --prefix web ci` | Passed: zero audit vulnerabilities |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Passed: Wasm/web build |
| `cd web && npx playwright install chromium` | Passed |
| `cd web && RUSTC="$(rustup which rustc)" SNES_TEST_PORT=18085 npm test -- --grep-invert '301 seconds'` | Passed: 37 tests, zero skips, 2.8 minutes |

| Artifact | Reproduced SHA-256 |
| --- | --- |
| Original smoke | `6e5518b3959883cc22e914621fc5b5d9f5c0fe4eb96ee61000769afdbb68d6e5` |
| CPU full | `1727e7c46ebb5df55711598264ba19005986bd34e915621383966adf674b3cfa` |
| CPU basic | `514d58950ae1ca940d6673c2a028b258b2ab17293bbc7cbd7cb54dded32abc5b` |
| SPC | `279d6a92d051ff8a98e190ce73e40482ff2cc981812509c7bca35a047484507e` |
| PPU/HDMA | `aef339e5d4a7319db31de577b253d977b45f5c45f09adfaa99a302ca300a957a` |
| Cleared Castle, both builds | `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869` |
| Cleared Memory, both builds | `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1` |

All mandatory gates completed; none remains skipped or unperformed. Browser
coverage retains Castle's five-minute replay, complete five-map/credits route
and freeze control, Memory gameplay/music/effects, numerical Web Audio and queue
assertions, and all eight original CPU-built browser fixtures. Only the existing
optional unaudited `301 seconds` tests were excluded by the prescribed command.

No tool/game source, pin, source allowlist, license, expected hash, browser
assertion or gameplay route was changed. Only original or licensed source-built
fixtures were used. The baseline failures are intentional; initial legacy-vector
failures are reconciled above. Physical device tests were not performed.

## Inventory and remaining limitations

All 24 supplied inventory scopes retain their inherited bounds; none becomes
verified. Partial: `cpu`, `bus`, `scheduler`, `dma`, `hdma`, `controllers`,
`ppu-bg`, `ppu-offset`, `ppu-hires`, `ppu-mode7`, `ppu-objects`, `ppu-color`,
`ppu-ports`, `ppu-display`, `spc`, `ipl`, `dsp-voices`, `dsp-noise`, `dsp-pitch`,
`dsp-envelope`, `dsp-echo`, `dsp-timing`. Unimplemented: `ppu-raster`, `pal`.
See the [inherited inventory](issue-81-validation.md#inventory-and-remaining-limits)
for the broader outstanding work.

This correction covers ordinary envelope application order, snapshots, the
necessary KON initialization boundary and output consumers. Remaining work
includes full DSP subcycle scheduling, SPC instruction bus timing, key polling
phase, torn writes, live KON-delay directory selection, final pre-KON output /
filter retention, physical terminal-header lookahead, writable-register
publication races, exhaustive envelope transitions and physical reset retention.
Existing NON advancement disputes are unresolved. Passing these checks proves
neither full hardware accuracy nor commercial compatibility and does not complete
Issue #3. No commercial ROM or firmware acquisition is needed. No optional
validation or required post-merge verification is specified by #91.
