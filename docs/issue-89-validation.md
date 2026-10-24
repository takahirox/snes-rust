# Issue #89: pitch-independent KON waveform priming

Implemented for [Issue #89](https://github.com/takahirox/snes-rust/issues/89).
Evaluation baseline: `4d904fef0b60f7204a44681221cf17eb43ad75fd`.
KON now decodes the initial waveform independently of programmed pitch. Pitch
zero holds initialized data; changes resume without restarting or decoding the
preloaded samples twice. This is a bounded sample-level correction.

## Source reconciliation

Inspected on 2026-10-24:

- [Anomie/jwdonal revision 1212](https://gist.github.com/nyanpasu64/a0d916ce6924912a7116682bf778e9a0#file-apudsp_jwdonal-txt),
  **BRR DECODING**, numbered KON sequence #0–#6, ring-buffer description and
  interpolation pseudocode; **SOUND GENERATION**, S2/S3b/S3c/S4/S7–S9;
  **KON ($4c)/KOFF ($5c)**. The explicit sequence, rather than buffer capacity,
  establishes the three preload groups and zero initial interpolation position.
- [Fullsnes, BRR Samples](https://problemkaputt.de/fullsnes.htm#snesapudspbrrsamples),
  **DIR/VxSRCN**, **Loop/End**, **BRR Decoding**; **BRR Pitch / 4-Point Gaussian
  Interpolation**; [ADSR/GAIN](https://problemkaputt.de/fullsnes.htm#snesapudspadsrgainenvelope)
  and **DSP Control Registers / KON, KOFF, ENDX**. These establish directory
  words, signed filter-0 scaling, coefficients and envelope controls.
- The repository's historical [#79 reconciliation](issue-79-validation.md#source-reconciliation)
  and [#81 reconciliation](issue-81-validation.md#source-reconciliation).

Anomie's detailed sequence decodes groups at #2/#3/#4 despite the shorter KON
register summary (also present in Fullsnes) saying decoding begins after five
empty samples. We use the detailed staged account: empty output does not imply
an empty decoder. S3 prepares output before S4 updates position. With index zero,
interpolation selects entries 0–3, not the last four of the twelve decoded values.
The ENDX wording conflict in #79 remains resolved by block completion and staged
publication, rather than header recognition. Twelve startup nibbles complete no
16-nibble block, so priming cannot generate ENDX.

This explicitly supersedes #77/#79's zero-filled startup interpolation and
#81's promise that initialization consumes no nibbles. Their historical results
are retained and marked with links here. A four-sample-only preload and a
last-four-of-twelve window are both inconsistent with the staged account.
No external emulator implementation or output was used as code or an oracle.

## Hardware stages and this core's call convention

The hardware account distinguishes preparation, decoding, position updates and
register publication. This core has one synchronous `Dsp::sample` call per output
frame, with no DSP subcycle scheduler. The following mapping is deliberate:

| Specification stage | Documented operation | Sample-level mapping |
| --- | --- | --- |
| #0 | Prepare final pre-KON output; reset envelope/position; final old decode | A register write queues KON; next call replaces the selected voice. Prior final decode/filter retention is outside this Issue. |
| #1 | First silence; S2 reads start; no decode or header check | Call 1 selects the stable directory start and emits zero. |
| #2 | First four-sample group at S4; position held | Call 2 decodes nibbles 0–3, emits zero. |
| #3 | Second group; position held | Call 3 decodes nibbles 4–7, emits zero. |
| #4 | Third group; position held | Call 4 decodes nibbles 8–11, restores selected window 0–3, emits zero. |
| #5 | Envelope updating begins; previously prepared output remains zero; position zero | Call 5 still publishes PCM/ENVX/OUTX zero; the existing output-aligned envelope evaluation is retained. |
| #6 | First data output; first position update at S4 afterward | Call 6 evaluates the first ordinary envelope and outputs window 0–3 at phase zero. |
| Following frames | Interpolate at updated position | Call 7 and later apply the current pitch increment before output. |

In particular, the #5 envelope operation is folded into the first audible API
call, as before. This does not implement preceding-envelope application timing
or separate ENVX/OUTX publication stages. Register writes between calls are
sample-level operations, not measurements of S3/S4 or the 16 kHz key poll.
Stable DIR/SRCN/start/loop words and RAM throughout initialization are required.
Selecting start on call 1 is equivalent under that restriction, not evidence for
live KON-delay selection. Five zero calls do not claim five physical cycles
from an arbitrary SPC write.

`Voice` retains twelve initial decoded values plus a cursor. After priming,
its four-element interpolation window is samples 0–3 and the next eight values
are queued. The filter history stays at decoded samples 11/10. Integer position
advancement shifts queued values 4–11 into the window; subsequent advancement
resumes the ordinary scalar decoder at nibble 12. No value is skipped or
redecoded. KON replaces the entire voice, including this queue. The global
noise/envelope clock and the programmed pitch registers are untouched.

After startup, let `a` be accumulated pitch advancement and `k=floor(a/4096)`.
The interpolation window is samples `k..k+3`; phase is `a mod 4096`. The existing
bounded group-position mask/saturation and PMON arithmetic remain in force.
At fixed ordinary pitch, the first post-delay call t=1 has `a=0`, then
`a=(t-1)*pitch`. Pitch zero holds both position and waveform indefinitely.

Later physical group lookahead is deliberately not added. Existing scalar
header/end processing occurs when the window needs a new nibble beyond its
right edge. Consequently at pitch $1000 a subsequent header is recognized on
post-delay call **14** (window 13–16), instead of the old unsupported call 17;
completion of a following terminal block appears on call **30**, formerly 33.
Initial end-bit blocks likewise first report scalar ENDX on call 14, absolute
KON call 19. These are core API boundaries, not physical publication claims.
Initial `01` selects zero envelope/release while priming. Both end-bit flags
continue silent processing using the live loop word, and ENDX stays sticky
until acknowledged or the selected voice is keyed on.

## Independently derived public vectors

[`dsp_kon_preload.rs`](../crates/snes-core/tests/dsp_kon_preload.rs) adds five
original public-API tests. They use signed filter-0 inputs and frozen numerical
expectations, with no production decode/interpolation helpers, private state,
production coefficient array or captured emulator output. Primary startup tests
have NON/PMON off, echo writes disabled, GAIN 64 (E=1024), voice volumes
+64/-128 and master volumes +64/+64.

For signed nibble n at range 10, decoded value is `n*512`. Each Gaussian product
shifts separately by ten for this half-amplitude representation; the sum is
truncated even. Output O is `(source*E >> 11) & !1`; stereo is
`(O>>2, (-O)>>1)`, ENVX=E>>4, OUTX=O>>8. The first three taps wrap at 16 bits;
these new input magnitudes avoid overflow.

| Data/phase-zero window | Gaussian source | O | Stereo | ENVX | OUTX | ENDX |
| --- | ---: | ---: | --- | ---: | ---: | ---: |
| constant +4 | 4098 | 2048 | (512,-1024) | 64 | 8 | 0 during hold |
| constant -4 | -4098 | -2050 | (-513,1025) | 64 | 247 | 0 during hold |
| constant +6 after retrigger | 6146 | 3072 | (768,-1536) | 64 | 12 | 0 during hold |
| initial `01`, either sign | arbitrary | 0 | (0,0) | 0 | 0 | 0 during hold |

All flags `00/01/10/11`, pitches $0000/$0001 and both constant signs are checked
through all five silent calls, sixteen ordinary calls and a further 128-call
zero-pitch hold. At pitch 1 the first sixteen outputs still use coefficient bin
zero. Thus initial silence, ordinary preloading, terminal release and absence of
fabricated ENDX are separate assertions.

The nonuniform nibble sequence is
`[4,-4,2,-1,6,1,-3,3,-8,5,-2,7,0,-7,2,-5]`, followed by loop nibble -4.
Phase-zero weights are `[370,1305,374,0]/1024`. Frozen O values for windows
starting at positions 0–16 are:

```text
-748,188,418,1958,600,-606,-48,-1866,704,464,2098,-8,
-2098,-464,-1822,-2142,-2050
```

The first window yields stereo (-187,374), unlike empty, last-group or one-step
advanced windows. Position 0 holds for 32 calls; pitch $1000 visits 1–8; zero
holds position 8 for 32 calls; resuming visits 9–16. ENDX first appears at
position 13, never during priming/hold. After clearing ENDX and another hold,
retrigger selects SRCN 1's distinct +6 start and +1 loop. Its O transition at
positions 13–16 is `3072,2606,974,512`; the loop must not replace the start early.
An independently running voice continues its own +1 loop, right PCM -256,
ENVX/OUTX 64/2 and sticky ENDX bit 1 during voice 0's retrigger/silence/output.

A separate pitch-1 test reaches all 4096 fractional increments and checks:

| Post-delay call | Position / coefficient bin | O | Stereo | OUTX |
| ---: | --- | ---: | --- | ---: |
| 1 | 0 / 0 | -748 | (-187,374) | 253 |
| 17 | 0 / 1 | -750 | (-188,375) | 253 |
| 2049 | 0 / 128 | -440 | (-110,220) | 254 |
| 4081 | 0 / 255 | 186 | (46,-93) | 0 |
| 4097 | 1 / 0 | 188 | (47,-94) | 0 |

Bin 1 uses `[366,1305,378,0]`, bin 128 `[56,965,969,58]`, bin 255
`[0,374,1305,370]`, from the published table. Every call requires ENVX 64/ENDX
0. Returning to zero holds O=188; resuming at $1000 gives O=418 exactly once.

## Reconciled legacy regressions

No test or discriminator was removed. Changes follow the selected first window
and S3-before-S4 sequence above:

- `dsp_directory` retains every live DIR/SRCN/RAM edit, carry/wrap, retrigger,
  fractional-envelope and simultaneous-voice case. Its virtual right-edge count
  is now `4+floor((t-1)*pitch/4096)` rather than zero-filled demand count.
  Independent sequences were extended at their tails to cover the same elapsed
  intervals. ENDX clear/reassert waits to the next actual scalar completion.
- `dsp_brr_end` retains all ten tests, including its full 4095-call flags,
  ADSR/GAIN and NON matrix. BRR now uses initialized constant -2048, with
  coefficient sums 2047/2048/2049 calculated from literal phase-bin sets derived
  from the source table. This retains signed rounding at every checked phase.
  Silent-chain ENDX calls become 14/46/78/94 immediately after delay, or
  13/45/77/93 when a zero-pitch ordinary output has already occurred.
- `dsp_pitch` retains all PMON/NON arithmetic and guest coverage. The BRR-pair
  reproduction now selects windows 4–7 versus 5–8 at phase $ff0: PCM
  (320,-640) versus (263,-526), OUTX 5 versus 4. The noise-predecessor partition
  vector starts with window 0–3. Its later bin-254 taps are
  `[0,378,1305,366]/1024`; independent PMON steps select window 6 at tick 647,
  12 at 653 and 13 at 654. Added 653/654 anchors bracket ENDX while retaining
  the old checkpoints 656/657.
- Private terminal history tests now distinguish the interpolation window's
  right edge from the decoder's twelve preloaded samples. Noise and all-eight-
  voice ENDX regression boundaries move from absolute 22 to 19. Terminal release,
  silent history, sticky clear/reassert, current loop selection and voice isolation
  assertions remain. Existing global counter/noise, envelope, mixing and echo
  tests pass without other behavior changes.

Initial full/targeted runs exposed the above obsolete expectations. Two newly
calculated PMON boundary literals initially had arithmetic transcription errors;
recomputing per-product shifts corrected them. Final suites pass. These changes
do not resolve the existing NON/PMON physical progression dispute.

## Original bounded SPC700 guest

The test's original instruction builder uploads through the existing firmware-free
protocol at $0200 and configures/reads DSP registers using $F2/$F3. Six host
commands record ENVX/OUTX/ENDX at $1100–$1111. The independent anchors are:

| Absolute PCM checkpoint | Recorded triple | Action after record | Previous 20 PCM samples |
| ---: | --- | --- | --- |
| 100 | (64,8,0) | Resume pitch $1000 | (512,-1024) |
| 200 | (64,247,1) | Pitch zero, clear ENDX | (-513,1025) |
| 300 | (64,247,0) | SRCN 1, KON while pitch zero | (-513,1025) |
| 400 | (64,12,0) | Resume pitch $1000 | (768,-1536) |
| 500 | (64,2,1) | Pitch zero | (128,-256) |
| 600 | (64,2,1) | STOP | (128,-256) |

Each 100-sample gap provides ample instruction time and, when running, exceeds
the first block plus interpolation transition. The start-only holds are thus
independent startup discriminators, and the running checkpoints select the
specified distinct loops. Guest code must be below 512 bytes, match uploaded
RAM and be ready before sample 100. Each acknowledgement has a fixed bound of
1000 ticks of 21 master clocks and must arrive by the ten-sample follow-up.
Both CPUs must STOP by sample 610 without errors.

Two APUs use whole intervals versus irregular `[1,7,666,1001,21,8193]` chunks,
including upload and target `ceil(N*32*21477272/1024000)` master clocks. At all
six checkpoints and six follow-ups they compare every PCM sample, all 128 DSP
registers, all 64 KiB RAM and four output ports. Current registers and subsequent
guest records must meet the literal anchors as well. Partition equality alone
proves determinism. See [MIT provenance](../tests/roms/PROVENANCE.md#issue-89-original-kon-preload-vectors-and-spc700-guest).

## Exact-baseline behavioral evidence

Only the new public test file was copied into an isolated detached baseline
worktree; its production code was unchanged:

```sh
git worktree add --detach .cache/issue-89/baseline 4d904fef0b60f7204a44681221cf17eb43ad75fd
cp crates/snes-core/tests/dsp_kon_preload.rs .cache/issue-89/baseline/crates/snes-core/tests/dsp_kon_preload.rs
CARGO_TARGET_DIR="$PWD/.cache/issue-89/baseline-target" cargo test \
  --manifest-path .cache/issue-89/baseline/Cargo.toml --locked \
  -p snes-core --test dsp_kon_preload
```

All five tests compiled and failed behaviorally, zero passed/ignored:

| Test | Baseline actual | Required |
| --- | --- | --- |
| Signed constant, pitch zero, flags `00`, first ordinary output | (0,0) | (512,-1024) |
| Nonuniform hold/resume | (0,0) | (-187,374) |
| Low-pitch nonuniform, first ordinary output | (0,0) | (-187,374) |
| Retrigger with independently running voice | (0,-256) | (768,-256) |
| Uploaded SPC startup hold | normalized PCM (0,0) | (0.015625,-0.03125) |

With the implementation, the exact new five-test target passes. Compilation
failures and captured emulator outputs are not counted as behavioral evidence.

## Mandatory validation actually executed

Host: macOS arm64; native Rust 1.94.0, rustup stable Rust 1.99.0 for spcasm
and Wasm via the README `RUSTC` override; Node 24.12.0, Python 3.13.13.
Local raw logs are under `.cache/issue-89`; durable results are recorded here.

| Command | Actual result |
| --- | --- |
| `cargo test --locked -p snes-core --test dsp_kon_preload` | Passed: 5 tests, zero failed/ignored |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --workspace --locked` | Passed: 448 tests, zero failed/ignored |
| `python3 tools/make-fixture.py .cache/smoke.bin` then `cmp tests/roms/smoke.bin .cache/smoke.bin` | Passed: byte-identical |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Passed: cc65, bass-untech and spcasm built from unchanged hash pins |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Passed: full/basic CPU and SPC, 600 frames each, markers 0649/0452/0557 |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Passed: 11 grayscale bands of 16 scanlines, final shade through line 223 |
| `python3 tools/build-cleared-games.py` twice | Passed: both builds match the unchanged expected Castle/Memory hashes |
| `npm --prefix web ci` | Passed: zero audit vulnerabilities |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Passed: Wasm and web assets built |
| `cd web && npx playwright install chromium` | Passed |
| `cd web && RUSTC="$(rustup which rustc)" SNES_TEST_PORT=18085 npm test -- --grep-invert '301 seconds'` | Passed: all 37 selected tests, zero skips, 2.8 minutes |
| `git diff --cached --check` | Passed |

All required gates completed; none remains skipped or unperformed. The initial
obsolete-vector failures and intentional exact-baseline failures are described
above; final checks pass. Browser coverage retains the 18,100-frame Castle and
Memory replays, Castle's complete five-map/credits route and freeze control,
Memory gameplay, numerical audio, Web Audio and queue assertions. The browser
gate builds all eight README CPU fixtures. Only the existing optional unaudited
`301 seconds` tests are excluded. No browser assertions, gameplay routes,
fixture/game source, tool pin, license or expected hash was changed.

| Artifact | SHA-256 reproduced |
| --- | --- |
| Original smoke | `6e5518b3959883cc22e914621fc5b5d9f5c0fe4eb96ee61000769afdbb68d6e5` |
| CPU full | `1727e7c46ebb5df55711598264ba19005986bd34e915621383966adf674b3cfa` |
| CPU basic | `514d58950ae1ca940d6673c2a028b258b2ab17293bbc7cbd7cb54dded32abc5b` |
| SPC | `279d6a92d051ff8a98e190ce73e40482ff2cc981812509c7bca35a047484507e` |
| PPU/HDMA | `aef339e5d4a7319db31de577b253d977b45f5c45f09adfaa99a302ca300a957a` |
| Cleared Castle, both builds | `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869` |
| Cleared Memory, both builds | `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1` |

Only original or explicitly licensed source-built fixtures were used. No
commercial ROM or firmware was acquired. No optional physical-device checks
were performed; Issue #89 requires no post-merge verification.

## Inventory and remaining limitations

Every category retains its existing scope and outstanding work from the
[full inherited inventory](issue-81-validation.md#inventory-and-remaining-limits).
There are still 22 partial categories, two unimplemented and none verified:

- Partial: `cpu`, `bus`, `scheduler`, `dma`, `hdma`, `controllers`, `ppu-bg`,
  `ppu-offset`, `ppu-hires`, `ppu-mode7`, `ppu-objects`, `ppu-color`, `ppu-ports`,
  `ppu-display`, `spc`, `ipl`, `dsp-voices`, `dsp-noise`, `dsp-pitch`,
  `dsp-envelope`, `dsp-echo`, `dsp-timing`.
- Unimplemented: `ppu-raster`, `pal`.

Within `dsp-voices`, this Issue closes stable-entry ordinary startup priming
and sample-level zero-pitch hold/resume. Remaining voice work includes independent
filters 1–3 evidence, exhaustive interpolation phases, simultaneous key/mixing
combinations, live KON-delay selection, physical header/lookahead/publication
timing, preceding-envelope application timing, register/RAM and SPC/DSP bus
races, torn writes and reset retention. Other inventory gaps retain their full
inherited scope. The original NON/PMON progression dispute remains separate.

No complete DSP subcycle scheduler, new filter algorithm, firmware, enhancement
chip or UI change is included. Passing these diagnostics establishes neither
full hardware accuracy nor commercial compatibility and does not complete
Issue #3.
