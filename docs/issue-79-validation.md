# Issue #79: terminal BRR headers and continued silent processing

Evaluation baseline: `64da1ba6249a451d6d7191409d74b6b0e792dd6d`.
This bounded change corrects terminal-header handling for stable BRR data and
nonzero pitch. It retains Issue #77's live-directory selection. It does not
complete Issue #3 or establish physical DSP pipeline timing.
[Issue #81](issue-81-validation.md) supersedes the initial-header nibble-demand
assumption below: stable initial terminal headers are recognized after the five
silent KON calls even at zero pitch. The original #79 results remain historical.

## Source reconciliation

The following attributed sources were fetched and inspected on 2026-10-24
before fixing the expectations. They supply hardware research facts, not copied
implementation code, executable fixtures, emulator output or an oracle trace.

- [Anomie/jwdonal revision 1212](https://gist.github.com/nyanpasu64/a0d916ce6924912a7116682bf778e9a0#file-apudsp_jwdonal-txt),
  BRR DECODING, SOUND GENERATION S3b/S3c/S4/S7, and ENDX: header `01`
  immediately zeros the envelope and selects release; decoding continues even
  after KOFF. End-bit blocks redirect through the loop pointer regardless of
  the loop bit. BRR DECODING puts ENDX at completion, whereas the ENDX register
  paragraph says start. These descriptions conflict. The staged S4/S7 account
  supports completion followed by publication. The document's 12-sample buffer
  explains why physical header visibility can mute the preceding block's tail.
- [Fullsnes, BRR Samples, Loop/End bits](https://problemkaputt.de/fullsnes.htm#snesapudspbrrsamples):
  `00/10` continue sequentially; `01` redirects, reports ENDX, enters release
  with zero envelope; `11` redirects and reports ENDX. Its concise end/mute
  terminology describes envelope silence, not a stopped fetch engine. It does
  not resolve fine header/publication timing.
- [Shay Green/blargg's pinned research implementation](https://github.com/snes9xgit/snes9x/blob/8028d3b6ca994f21fdff220d513a08382e37afc3/apu/bapu/dsp/SPC_DSP.cpp),
  commit `8028d3b6ca994f21fdff220d513a08382e37afc3`: V3b reads the header;
  V3c selects release and zero envelope for `01`. V4 still decodes according to
  interpolation demand, selects the refreshed pointer on **end bit alone**
  when the block finishes, and marks the end event. V5 merges that event with
  sticky ENDX and clears the selected bit on KON; V7 publishes it. V1/V2 refresh
  the directory selection. V3c calculates the current output before changing
  the envelope for subsequent output, illustrating its pipelined ordering.
  This LGPL source was read for attributed research evidence only; none of its
  code or output was incorporated or executed.

Chosen contract: header `01` forces zero/release, but both end-bit combinations
continue through the current loop pointer and later end boundaries can report
ENDX again. GAIN, ADSR, SRCN, DIR and loop-word edits do not leave release. KON
clears the selected ENDX and starts that voice through the current start word.
We resolve the ENDX wording conflict using BRR DECODING, SOUND GENERATION and
V4–V7 together. We do not infer a stopped decoder from the word mute.

## Scalar API mapping and implementation

For later blocks, the eager scalar decoder reads a header when consuming its
first nibble. That call zeros E and selects release **before its PCM, ENVX and
OUTX publication**. Initial-header recognition no longer requires nibble demand;
see [Issue #81's scalar mapping](issue-81-validation.md#scalar-api-boundary).
Thus the terminal payload is never played at the previous envelope. At pitch
$1000, after the existing five silent KON calls, a normal 16-nibble block followed
by `01` is muted on post-delay call 17. ENDX appears
when consumption advances beyond the terminal block, on call 33. Starting on
`01` now mutes by the first ordinary post-delay call regardless of pitch;
at pitch $1000 ENDX still appears on call 17. These call numbers describe this
API, not physical hardware timing. In particular, this core does not reproduce the physical 12-sample lookahead, V3c's already-prepared
output, physical header prefetch during KON, or separate register publication
subcycles.

No BRR history, interpolation window, phase or address is cleared by a terminal
header. At either end-bit boundary, the decoder consumes the existing live
loop word and carries on. The `active` field means initialized processing,
independent of audible envelope level. Removing deactivation at release zero
is necessary because the same release branch follows terminal headers and KOFF;
otherwise the terminal voice would stop on the next sample. Ordinary KOFF still
subtracts eight E units every sample, saturates at zero, retains hidden envelope
state, and cannot be undone by envelope-register edits. Existing FLG reset and
uninitialized-voice behavior are deliberately retained; reset retention is not
part of this change.

## Independent numerical evidence

[`dsp_brr_end.rs`](../crates/snes-core/tests/dsp_brr_end.rs) contains six original
public-API tests. No production decoder, Gaussian table, envelope helper or
private field generates their expected values. Filter-0 range-10 data decode
as signed nibble times 512. The phase-zero taps are `[370,1305,374,0]/1024`,
and phase $800 uses `[56,965,969,58]/1024`, with individual product shifts and
even truncation. The tested magnitudes avoid intermediate overflow.
For post-delay call t, the independently specified count is
`floor(t*pitch/4096)`; $1800 alternates half/zero phases, $1000 and $3000 use zero.

Direct GAIN 64 gives E=1024/ENVX=64. Fast ADSR $8f/$e0 gives E=1024 on the
first post-delay call, then 2047 throughout the tested audible interval.
Multiplying the interpolated source by E/2048 and truncating even gives O;
voice volumes +64/-128 and master +64/+64 give `(O>>2, (-O)>>1)`, OUTX=`O>>8`.
Settled independent anchors at GAIN 64:

| Signed nibble | Decoded | Source | O | Stereo | ENVX | OUTX |
| ---: | ---: | ---: | ---: | --- | ---: | ---: |
| 4 | 2048 | 4098 | 2048 | (512,-1024) | 64 | 8 |
| -4 | -2048 | -4098 | -2050 | (-513,1025) | 64 | 247 |
| 2 | 1024 | 2048 | 1024 | (256,-512) | 64 | 4 |
| 6 | 3072 | 6146 | 3072 | (768,-1536) | 64 | 12 |
| Terminal release | nonzero payload | arbitrary | 0 | (0,0) | 0 | 0 |

The normal-to-terminal matrix covers sequential flags `00/10`, positive and
negative preceding blocks, distinct signed nonzero terminal payloads, direct
GAIN and ADSR, and pitches $1000/$1800/$3000. Every sample and ENVX/OUTX/ENDX is
checked around header and completion. A simultaneous voice with its own entry
remains audible on the other channel and reports its own ENDX. The start-header
matrix covers all four flag controls with both envelope modes and all pitches,
including audible `11` looping and sequential `00/10` behavior.

The silent-chain test uses `A01 -> B00/B+9:11 -> C10/C+9:11 -> D11`.
It clears each ENDX event and requires precisely counts **17,49,81,97**, checking
zero PCM/ENVX/OUTX on every call. Stable DIR/SRCN changes select C while silent;
a later RAM loop-word edit selects D. Direct GAIN/ADSR changes cannot restore
output. Before KON the current start word alone is edited to a new block at
D+9 (nibble 6), distinct from loop D (nibble 2). KON must select that new start
and restore the five-call delay, cleared ENDX and independently calculated PCM
through both the start transient and the following loop.

The KOFF test anchors all 128 fading samples from E=1024 through zero, with
source 4098, followed by no revival when KOFF is cleared and GAIN raised. After
clearing ENDX, another 16 calls must report a new end while still silent.
A narrowly scoped private test in `dsp.rs` additionally checks every decoded
window, both history values, address, nibble, bounded phase, release and processing
state through 50 calls and a live loop change. Its explicit signed sequences
catch frozen decoding, fabricated ENDX and repeated history resets. It uses
filter 0; it does not establish independent filter 1–3 accuracy.

## Changed legacy expectations

All changes below follow the source contract above; passing old tests was not
used as specification authority. Their other numerical coverage is retained.

| Existing test | Corrected expectation |
| --- | --- |
| `dsp_directory::normal_blocks_ignore_loop_bit_and_end_without_loop_stays_silent` | Normal payload still audible; terminal header immediately zeros output/ENVX before its ENDX; silent loops reassert ENDX after clearing |
| `regression::dsp_endx_reports_brr_ends_independently_of_looping` | All eight voices/all four flags retained; both end-bit combinations reassert ENDX, with `01` silent and `11` audible |
| `dsp::envelope_tests::release_retrigger_soft_reset_and_reconstruction_isolate_hidden_state` | Release-zero remains initialized and in release; fade, hidden envelope, retrigger and reset assertions retained |
| `dsp::noise_tests::noise_pitch_is_independent_but_brr_decoding_loops_and_endx_continue` | NON is muted by the terminal header while noise and BRR decoding continue; clear/reassert is checked; shared noise-state and loop checks retained |
| `dsp::pitch_tests::noise_predecessor_and_target_keep_shared_noise_and_brr_end_processing` | Terminal target OUTX is zero from its first header; it still consumes the next loop nibble. Existing modulated phases, noise state and ENDX vectors retained |

No unrelated NON/PMON correction is included. The first workspace run exposed
the two additional legacy noise/pitch assumptions above; they were corrected
without weakening shared-noise or modulation assertions. An initial clippy run
also requested `is_multiple_of` in the new test arithmetic; that was fixed.

## Original uploaded SPC700 guest and partition checks

The guest is generated from original instruction bytes in the test, uploaded to
$0200 via the existing firmware-free port protocol. It configures and reads DSP
registers through $F2/$F3. Initial A is terminal `01`, with loop B consisting of
64 original sequential blocks, the last `11`; C is a distinct audible `11`
block and D a distinct start-only `11` block. The long B chain leaves a wide
stable interval for clearing and reading ENDX, without racing a boundary.

At absolute output checkpoints 100/200/1300/1400/1500 the host sends commands
1–5. The guest records ENVX/OUTX/ENDX triples at $1000..$100e, respectively:
**(0,0,1), (0,0,0), (0,0,1), (0,0,1), (64,12,1)**.
After command 1 it clears ENDX; after command 2 it edits the live loop word to C
and changes direct GAIN/ADSR; command 3 changes SRCN to 1 and restores GAIN 64
without KON; command 4 performs KON; command 5 stops. The final current start D
is distinct from loop C. Every PCM sample in the preceding 20-sample windows
must be zero through command 4, and **(768,-1536)** before command 5, normalized
by 32768. The independent public DSP test anchors a distinct start transient
and its subsequent loop; the guest's final window anchors the settled C loop. The host checks uploaded
code, edited RAM word, selected SRCN, final STOP and absence of core errors.
Each acknowledgement is bounded to 1000 advances of 21 master clocks, upload
is below 512 bytes and ready before sample 100; completion is before sample 1510.
See the [MIT provenance record](../tests/roms/PROVENANCE.md#issue-79-original-terminal-header-spc700-guest-and-brr-vectors).

The partition test runs the same guest in two APUs with whole intervals versus
repeating irregular chunks `[1,7,666,1001,21,8193]`, targeting
`ceil(N*32*21477272/1024000)` master clocks including upload. At each command
checkpoint and ten samples later it compares **every PCM sample, all 128 DSP
registers, all 64 KiB RAM, and output ports**. The same independent PCM and guest
register records must match; acknowledgements must arrive by the ten-sample
checkpoint. Partition equality establishes determinism, not physical timing.

## Baseline discriminators

Created an isolated detached worktree at the evaluation baseline under
`.cache/issue-79/baseline`, copied only the new public test file, and ran:

```sh
CARGO_TARGET_DIR="$PWD/.cache/issue-79/baseline-target" cargo test \
  --manifest-path .cache/issue-79/baseline/Cargo.toml --locked \
  -p snes-core --test dsp_brr_end
```

Compilation succeeded; **0 passed, 6 failed behaviorally**, zero ignored.

| Discriminator | Baseline observation | Required |
| --- | --- | --- |
| Normal-to-terminal, pitch $1000/GAIN64, tick 17 | Stereo (512,-512) | (0,-512), second voice unaffected |
| Terminal at start | ENVX/OUTX (64,0) on first decode | (0,0) |
| Silent chain | ENVX/OUTX (64,0) on first decode | (0,0) |
| KOFF at zero, clear and advance | ENVX/OUTX/ENDX (0,0,0) | (0,0,1) |
| Uploaded guest and partitions | Third record (0,0,0) after ENDX clear | (0,0,1) |

The guest failures independently expose lack of later silent ENDX even after
the baseline has already muted. The normal transition failure demonstrates that
muting waits too long. No compile failure is counted as behavioral evidence.

## Mandatory validation

Final gate results are recorded below after execution. Host: macOS arm64,
native Rust 1.94.0, rustup stable Rust 1.99.0 for spcasm and Wasm, Node 24.12.0,
Python 3.13.13, Playwright/Chromium. Local detailed command logs are under
`.cache/issue-79`; this record preserves the results in the checkpoint.

| Command | Final result |
| --- | --- |
| `cargo test --locked -p snes-core --test dsp_brr_end` | Passed: 6 tests, zero failed/ignored |
| `cargo test --locked -p snes-core --test dsp_directory` | Passed: 12 tests, zero failed/ignored |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --workspace --locked` | Passed: 420 tests, zero failed/ignored, including the private history test and existing envelope/noise/pitch/mixing/echo suites |
| `python3 tools/make-fixture.py .cache/smoke.bin` and `cmp tests/roms/smoke.bin .cache/smoke.bin` | Passed, byte-identical |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Passed: all three tools built from unchanged pins |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Passed: CPU full/basic and SPC, 600 frames each, terminal markers 0649/0452/0557 |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Passed: 11 grayscale bands of 16 scanlines, final shade persists through line 223 |
| `python3 tools/build-cleared-games.py` twice | Passed: two fresh builds, unchanged expected Castle/Memory hashes |
| `npm --prefix web ci` | Passed: zero audit vulnerabilities |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Passed |
| `cd web && npx playwright install chromium` | Passed |
| `cd web && SNES_TEST_PORT=18085 npm test -- --grep-invert '301 seconds'` | Passed: all 37 selected checks, zero skips, 2.8 minutes |
| `git diff --cached --check` | Passed with all changed/new files staged |

The browser suite also built all eight README CPU fixtures. It retained both
18,100-frame cleared-game replays, Castle's five-map/credits route and freeze
negative control, Memory wins/restarts and numerical/live Web Audio assertions,
diagnostic controls/audio/pause/reset/save checks, controller and PPU canvas
checks, and Pages subpath boot. The required grep excludes only the optional
unaudited checks named `301 seconds`. No final mandatory gate failed, was skipped
or remains unperformed. The earlier clippy and legacy-test failures are recorded
above; no game assertion or fixture was changed to obtain these passes.

Measured artifact hashes match the unchanged build assertions:

| Artifact | SHA-256 |
| --- | --- |
| Original smoke | `6e5518b3959883cc22e914621fc5b5d9f5c0fe4eb96ee61000769afdbb68d6e5` |
| CPU full | `1727e7c46ebb5df55711598264ba19005986bd34e915621383966adf674b3cfa` |
| CPU basic | `514d58950ae1ca940d6673c2a028b258b2ab17293bbc7cbd7cb54dded32abc5b` |
| SPC | `279d6a92d051ff8a98e190ce73e40482ff2cc981812509c7bca35a047484507e` |
| HDMA | `aef339e5d4a7319db31de577b253d977b45f5c45f09adfaa99a302ca300a957a` |
| Cleared Castle | `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869` |
| Cleared Memory | `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1` |

Assembler pins are unchanged: cc65
`555282497c3ecf8b313d87d5973093af19c35bd5`, bass-untech
`9db6088a378061afc7b82f50997a5b9a1d951175`, and spcasm
`c2d57b7aa8ddd6b22a18424047b8c2d93f0373e7`.

## Remaining limits and inventory

Physical header-prefetch/KON alignment, zero-pitch header-fetch timing, exact
DSP subcycles, torn writes, SPC/DSP and echo/RAM races, and reset retention remain
outside this bounded correction. Voice work still includes independent BRR-filter
1–3 evidence, exhaustive interpolation phases, simultaneous key/mixing
combinations, KON-delay selection changes and physical header/directory/key timing.
No new public inspection API, frontend work, firmware or commercial ROM acquisition
is included. No full hardware accuracy, commercial compatibility or completion
of Issue #3 is claimed.

All 24 inventory scopes retain their statuses: partial `cpu`, `bus`, `scheduler`,
`dma`, `hdma`, `controllers`, `ppu-bg`, `ppu-offset`, `ppu-hires`, `ppu-mode7`,
`ppu-objects`, `ppu-color`, `ppu-ports`, `ppu-display`, `spc`, `ipl`, `dsp-voices`,
`dsp-noise`, `dsp-pitch`, `dsp-envelope`, `dsp-echo`, `dsp-timing`; unimplemented
`ppu-raster`, `pal`. None becomes verified. Cleared Castle/Memory source pins,
licensing, ROM hashes, replay routes/durations, freeze controls and numerical/live
audio assertions remain unchanged. No optional unaudited games or physical-device
checks were performed or required. No post-merge verification is required.
