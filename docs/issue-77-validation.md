# Issue #77 stable live S-DSP directory validation

Implemented on 2026-10-24 for [Issue #77](https://github.com/takahirox/snes-rust/issues/77),
evaluation baseline `3a6e051ad265ecd4dca9f456cf519e884c863926`.
Production changes are confined to directory selection in `Dsp::sample`.
The existing scalar BRR decoder, interpolation, envelope, mixing and public API
are preserved. `dsp-voices` remains partial; no inventory scope becomes verified.

Issue [#89](issue-89-validation.md#source-reconciliation) supersedes this
record's empty startup waveform, initial nibble-demand counts and interpolation
windows. Its staged KON reconciliation establishes three decoded groups and
first window 0–3 at phase zero. Earlier executed results below are historical;
live selection, terminal release and silent processing remain covered.

## Specification reconciliation

Consulted on 2026-10-24:

- [Anomie/jwdonal S-DSP documentation, revision 1212](https://gist.github.com/nyanpasu64/a0d916ce6924912a7116682bf778e9a0#file-apudsp_jwdonal-txt),
  dated 2015-09-28. **DIR ($5d)** defines four-byte entries containing start
  and restart words; DIR changes affect subsequent loops/key-ons. **VxSRCN
  ($x4)** likewise says a live change affects the next loop/key-on, without
  immediate redirection. **SOUND GENERATION**, S1/S2, describes staged SRCN
  and pointer loads using cached DIR/SRCN; its preamble allows unconditional
  pointer loading with conditional use. DIR is sampled at cycle 28. S4 handles
  block progression/ENDX. **BRR DECODING** associates end blocks with the selected
  loop pointer and distinguishes end-without-loop release from looping.
  **KON** describes restarting and five empty output samples.
- [Fullsnes, SNES APU DSP BRR Samples](https://problemkaputt.de/fullsnes.htm#snesapudspbrrsamples),
  currently served text, extracted from no$sns v1.5 (no separate text revision
  supplied). **VxSRCN/DIR** gives `SRCN*4 + DIR*100h`, start bytes 0–1 and
  loop bytes 2–3, and postpones effects until loop/key-on. **Loop/End bits**
  distinguishes sequential blocks (codes 0/2), end+mute (1), and end+loop (3),
  with ENDX for either end code. **SNES APU Low Level Timings**, register/RAM
  chart, lists voice directory-word fetch slots within each sample period.
  **BRR Pitch / 4-Point Gaussian Interpolation** supplies the coefficient data
  used below. These descriptions establish live selection, but do not alone
  resolve every staging detail.
- Shay Green/blargg's original research implementation, **snes_spc 0.9.0,
  copyright 2007**, at the already cited pin
  [`8028d3b6ca994f21fdff220d513a08382e37afc3`, SPC_DSP.cpp](https://github.com/snes9xgit/snes9x/blob/8028d3b6ca994f21fdff220d513a08382e37afc3/apu/bapu/dsp/SPC_DSP.cpp#L759),
  especially **misc_28, V1, V2, V3c and V4**. V1 forms a 16-bit entry address
  from staged DIR/SRCN and loads the next SRCN. V2 reads a directory word on
  each voice pass: start during KON delay, otherwise the word at offset 2.
  V3c uses the start pointer during initialization; V4 uses the refreshed
  pointer on end-block completion. Thus this research model refreshes a
  pointer every output sample, before its possible loop use; it does not
  retain KON's loop word for the voice lifetime. The author's
  [research account](https://www.slack.net/~ant/libs/audio.html#snes_spc)
  is also cited by the repository's [Issue #75 reconciliation](issue-75-validation.md#specification-reconciliation).
  This LGPL source was consulted for attributed hardware research facts. No
  implementation, binary, trace, output or fixture was imported or executed.

Resolved stable-write contract: the aligned entry is
`((DIR << 8) + 4*SRCN[v]) mod 65536`; read its little-endian start word for
initialization, and its live selected loop word for looping. A stable edit to
loop RAM, DIR or that voice's SRCN can select a new loop on every later end+loop
block. Editing a start word alone cannot restart playback. These selection
changes do not trigger KON: the current block, decoding/interpolation history, pitch and
envelope continue. ENDX remains sticky until cleared by a write or that voice's
KON. These are supported stable effects, not a physical loop-boundary RAM-read
claim. In particular, the baseline lifetime snapshot is contradicted by both
register descriptions and the research model's pointer refresh.

Anomie's S4 wording flags a loop address for the next S2; the research model
fetches it in V2 before V4 selects it. We use the latter as evidence for eager
per-sample refresh, without claiming to settle that fine-stage wording
difference. Stable writes completed before both windows select the same word.

## Sample API conventions and remaining uncertainty

Historical note: [Issue #79](issue-79-validation.md) supersedes the terminal
control approximation described below. Header `01` now zeros/releases the
envelope when the eager decoder processes the header, and silent BRR work
continues through live end-bit redirects and later ENDX events. The directory
selection evidence and its timing limits remain applicable.

`Dsp::sample` now refreshes each voice's selected loop word once per call, using
current DIR/SRCN, before its existing scalar decode work. Only KON reads the
start word. The decoder still consumes the cached loop word when crossing an
end+loop boundary. Writes in these tests are complete many decoded nibbles
before that boundary; guest changes are followed by many full samples. Thus
hardware's earlier staged refresh and this collapsed refresh have the same
selected address for these cases. This does not establish behavior for a write
near either implementation's fetch window.

The existing five silent calls, eager nibble consumption, block-boundary ENDX
visibility, register publication and envelope-update ordering are sample-API
conventions. We do not reinterpret their tick numbers as hardware DSP subcycles.
In particular, references describe end-without-loop muting/release upon header
processing and continuing internal BRR fetching; this core's existing scalar
convention silences/deactivates at the subsequent boundary. The control tests
preserve its observable terminal silence/ENDX behavior; they do not establish
physical header timing or internal silent fetches. Those are outside this
Issue's directory selection scope, as are changes during KON delay, torn words,
SPC/DSP races, echo/directory collisions and reset retention.

## Independently derived numerical tests

All 12 public-API tests are in
[`dsp_directory.rs`](../crates/snes-core/tests/dsp_directory.rs).
No private DSP fields or new inspection API are used. Original range-10,
filter-0 blocks contain constant nibbles: A=4, B=-4 (`$c`), C=2, D=1,
and a distinct start-only target=6. Decoded 15-bit values are respectively
2048, -2048, 1024, 512 and 3072, from `nibble*1024/2`.

At phase zero the documented Gaussian taps are `[370,1305,374,0]/1024`;
at phase `$800` they are `[56,965,969,58]/1024`. Each product is shifted
separately before summation and even truncation. All values are safely below
intermediate overflow. The test arithmetic consumes an explicitly specified
sequence of decoded values; it contains no directory lookup, BRR decoder or
loop-state implementation. At pitch `$1000`, count equals the number of
post-delay calls; at `$3000` it is three times that number, including a call
which decodes both sides of a block boundary. The fractional test uses
`floor(3*t/2)` and alternating `$800/$000` phases.

Direct GAIN 64 gives E=1024 and ENVX=64. With voice volumes +64/-128 and
master volumes +64/+64, post-envelope output O produces stereo
`(O >> 2, (-O) >> 1)` and OUTX=`O >> 8` as a byte. Independent settled anchors:

| Block | Gaussian source | Even O | Stereo PCM | OUTX |
| --- | ---: | ---: | --- | ---: |
| A | 4098 | 2048 | (512, -1024) | 8 |
| B | -4098 | -2050 | (-513, 1025) | 247 |
| C | 2048 | 1024 | (256, -512) | 4 |
| D | 1024 | 512 | (128, -256) | 2 |
| Start-only | 6146 | 3072 | (768, -1536) | 12 |

After A's 16 nibbles, the first loop nibble is consumed at count 17 and sets
ENDX. Its zero fourth tap keeps A's PCM for that call. At count 18, retained
history is `[2048,2048,new,new]`: A→B gives source 2602, O=1300 and
PCM **(325,-650)**; A→C gives source 3724, O=1862 and
PCM **(465,-931)**. This catches premature redirection, erased history and
incorrect use of the new entry's start word. Before the second boundary, a
second live update selects another distinct destination. A later KON verifies
five silent calls, cleared ENDX and decoding through the current start entry.
The SRCN and DIR cases have distinct new start and loop targets.

The fractional test uses linear GAIN rate 31: `E=32*t`, ENVX=`2*t` for
32 post-delay calls. It checks every stereo sample and OUTX/ENVX/ENDX through
two live loops, so a phase or envelope restart cannot hide behind a held level.

Additional vectors cover:

- Start-word-only edits through multiple loops, without restarting.
- Entry carry `$20:$40 -> $2100`, wrap `$ff:$40 -> $0000`, and last aligned
  entry `$ff:$3f -> $fffc`, whose live loop word occupies `$fffe/$ffff`.
  All entries are four-byte aligned; no impossible unaligned word is invented.
- A block at `$fffb` whose eight payload bytes occupy `$fffc..$0003`, followed
  by a live loop change. Echo writes are disabled so this is isolated.
- Two simultaneous voices with distinct SRCN: a voice-0 SRCN change leaves
  voice 1 unchanged; shared DIR selects distinct per-voice entries/destinations;
  an edit to voice 0's new entry leaves voice 1 unchanged. Every PCM and both
  OUTX/ENVX values are checked through four blocks. ENDX write-to-clear,
  subsequent reassertion and voice-selective KON clearing are checked.
- Sequential blocks with flags 0 or 2, then an end-without-loop control with
  terminal silence/ENDX and clearing. These preserve the existing convention
  described above, including no audible revival through live directory edits.

## Original uploaded guest and APU partitions

The original 212-byte guest configures one ordinary-pitch voice, writes KON
once, and waits on `$F4`. Before output sample 80, upload/setup must complete.
Commands at host sample checkpoints 100/200/300/400 cause the guest to record
OUTX/ENVX/ENDX triples in `$1000..$100b`, then respectively edit the loop
word at `$2002/$2003`, change DIR to `$21`, change SRCN to 1, and finish.
After recording and changing selection, it acknowledges the command; the final
instruction stops the SPC. Each port wait is bounded to 1000 advances of
21 master clocks. No subsequent KON occurs.

The four recorded triples must be **(8,64,1), (247,64,1), (4,64,1), (2,64,1)**.
Every stereo host sample in windows **81..100, 181..200, 281..300, 381..400**
must equal the corresponding A/B/C/D PCM anchor above, normalized by 32768.
The waits and windows allow multiple complete loops after each guest write;
no torn/racing pointer behavior is inferred. The host verifies the modified
RAM word, selected registers, bounded final halt and absence of APU errors.
The [provenance record](../tests/roms/PROVENANCE.md#original-live-directory-spc700-guest-and-brr-data-approved)
licenses the original guest, directory and BRR data under MIT.

A second run uploads the same guest into two APUs, then advances one in whole
intervals and one with repeating irregular master-clock chunks
`[1,7,666,1001,21,8193]`. Absolute target master time for output count N is
`ceil(N*32*21477272/1024000)`, accounting for upload advances. At
100/110/200/210/300/310/400/410 both APUs must have exactly N stereo samples.
The host sends identical commands at 100/200/300/400, and requires the expected
acknowledgement/result bytes by the following ten-sample checkpoint. It compares
**every PCM sample, all 128 DSP registers, all 64 KiB RAM (including guest
code/results, directory and BRR), and output ports** at each boundary.
The same independent PCM and recorded-register anchors are asserted. Equality
establishes deterministic partitioning, not physical timing; the anchors supply
separate numerical evidence.

## Baseline failures

With the final test file present, temporarily replaced only `src/dsp.rs` using
`git show 3a6e051ad265ecd4dca9f456cf519e884c863926:crates/snes-core/src/dsp.rs`,
ran `cargo test --locked -p snes-core --test dsp_directory`, and restored the
fixed source in a Python `finally` block. It compiled and exited 101:
**2 passed, 10 failed**, zero ignored. The start-word-only and sequential/
end-without-loop controls passed. Exact behavioral failures:

| Tests | Baseline observation | Required independent result |
| --- | --- | --- |
| Loop RAM, SRCN, carry/wrap, wrapping BRR, three-nibble pitch | Count 18 stereo (512,-1024) | (325,-650) |
| DIR | Count 18 stereo (512,-1024) | (465,-931) |
| Two voices | Count 18 stereo (512,-512) | (325,-512), voice 1 unaffected |
| Fractional pitch/running envelope | Post-delay tick 12 (192,-384) | (121,-243) |
| Uploaded guest and guest partitions | Sample 181 (512,-1024) | (-513,1025) |

These failures occur because the baseline continues using KON's captured loop
pointer. All 12 tests pass with per-sample refresh. During test authoring an
incorrect guest-size assertion (213 rather than 212) and an ambiguous integer
type in the new fractional test were corrected before this final baseline run;
those setup/compile failures are not counted as behavioral evidence.

## Mandatory validation

Host: macOS arm64; native Rust 1.94.0; rustup stable Rust 1.99.0 selected
for the SPC assembler and Wasm as documented by README; Node 24.12.0,
Python 3.13.13 and Chromium/Playwright.

| Command | Actual result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --workspace --locked` | Passed: 413 tests, zero failed/ignored, including existing DSP, controller, DMA/HDMA and PPU regressions |
| `cargo test --locked -p snes-core --test dsp_directory` | Passed: all 12 tests |
| `python3 tools/make-fixture.py .cache/smoke.bin` and `cmp tests/roms/smoke.bin .cache/smoke.bin` | Passed, byte-identical |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Passed: cc65, bass-untech and spcasm from unchanged pins |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Passed on fixed source: CPU full/basic and SPC, 600 frames each, terminal markers 0649/0452/0557 |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Passed: 11 grayscale bands, 16 scanlines each, final shade through line 223 |
| `python3 tools/build-cleared-games.py` twice | Passed: fresh builds with identical expected Castle/Memory hashes |
| `npm --prefix web ci` | Passed, no audit vulnerabilities |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Passed |
| `cd web && npx playwright install chromium` | Passed |
| `cd web && SNES_TEST_PORT=18085 npm test -- --grep-invert '301 seconds'` | Passed: all 37 selected checks, zero skips, 3.1 minutes |
| `git diff --cached --check` | Passed with all five changed/new files staged |

The browser suite built all eight README CPU fixtures (hires, pseudo-hires,
obj-sizes, obj-limits, mode7-scroll, mode7-extbg, mode7-transform and mode7-row).
It passed both 18,100-frame cleared-game replays, Castle's five-map/credits
route and freeze negative control, Memory wins/restarts and numerical/live
Web Audio assertions, diagnostic audio/control/pause/reset/save checks,
controller and PPU canvas checks, and Pages subpath boot. Optional unaudited
checks named `301 seconds` were excluded by the required command. No mandatory
gate remains failed, skipped or unperformed.

Measured SHA-256 values match the unchanged fixture/build assertions:

| Artifact | SHA-256 |
| --- | --- |
| Original smoke | `6e5518b3959883cc22e914621fc5b5d9f5c0fe4eb96ee61000769afdbb68d6e5` |
| CPU full | `1727e7c46ebb5df55711598264ba19005986bd34e915621383966adf674b3cfa` |
| CPU basic | `514d58950ae1ca940d6673c2a028b258b2ab17293bbc7cbd7cb54dded32abc5b` |
| SPC | `279d6a92d051ff8a98e190ce73e40482ff2cc981812509c7bca35a047484507e` |
| HDMA | `aef339e5d4a7319db31de577b253d977b45f5c45f09adfaa99a302ca300a957a` |
| Cleared Castle | `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869` |
| Cleared Memory | `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1` |

Assembler pins remain cc65 `555282497c3ecf8b313d87d5973093af19c35bd5`,
bass-untech `9db6088a378061afc7b82f50997a5b9a1d951175` and
spcasm `c2d57b7aa8ddd6b22a18424047b8c2d93f0373e7`.

## Remaining limits and inventory

Exact DSP subcycles, torn pointer writes, writes racing a fetch, physical
sample-directory/key-event timing and KON-delay changes remain unresolved here.
No DSP pipeline rewrite, public inspection API, frontend change, firmware or
commercial ROM acquisition is included. Independent BRR filters 1–3 evidence,
exhaustive interpolation phases and simultaneous key/mixing combinations remain
`dsp-voices` work. Passing these bounded checks establishes neither full hardware
accuracy nor commercial compatibility and does not complete Issue #3.

All 24 inventory scopes retain their status: partial `cpu`, `bus`, `scheduler`,
`dma`, `hdma`, `controllers`, `ppu-bg`, `ppu-offset`, `ppu-hires`, `ppu-mode7`,
`ppu-objects`, `ppu-color`, `ppu-ports`, `ppu-display`, `spc`, `ipl`, `dsp-voices`,
`dsp-noise`, `dsp-pitch`, `dsp-envelope`, `dsp-echo`, `dsp-timing`; unimplemented
`ppu-raster`, `pal`. None becomes verified. Cleared-game source pins, licenses,
ROM hashes, replay routes/durations, freeze controls and numerical/live audio
assertions remain unchanged. Optional unaudited games and physical-device tests
are not required or performed. No post-merge verification is required.
