# Issue #81: initial terminal-header recognition at zero pitch

Evaluation baseline: `f4ca3ce3d8c10850da72924e0926ec5f40b2a68d`.
Stable initial BRR `01` now selects zero envelope and release independently of
pitch and NON. This closes the pitch-gated omission left by Issue #79 while
retaining its later silent decoding and live-directory behavior. It does not
complete Issue #3 or establish physical DSP timing.

Issue [#89](issue-89-validation.md#source-reconciliation) supersedes this
record's empty startup waveform, initial nibble-demand counts and interpolation
windows. Its staged KON reconciliation establishes three decoded groups and
first window 0–3 at phase zero. Earlier executed results below are historical;
live selection, terminal release and silent processing remain covered.

## Source reconciliation

The [Issue #79 reconciliation](issue-79-validation.md#source-reconciliation)
remains applicable. These sources were fetched and inspected again on 2026-10-24:

- [Anomie/jwdonal revision 1212](https://gist.github.com/nyanpasu64/a0d916ce6924912a7116682bf778e9a0#file-apudsp_jwdonal-txt),
  SOUND GENERATION S3b/S3c/S4 and BRR DECODING: header reading and terminal
  envelope handling precede demand-driven decoding. Header `01` zeros the
  envelope and selects release. Silence does not stop internal processing.
- [Fullsnes, BRR Samples, Loop/End bits](https://problemkaputt.de/fullsnes.htm#snesapudspbrrsamples):
  `00/10` continue sequentially, `01` releases with zero envelope, and `11`
  loops without that muting. NON replaces the source before envelope scaling.
- [Shay Green/blargg's pinned research implementation](https://github.com/snes9xgit/snes9x/blob/8028d3b6ca994f21fdff220d513a08382e37afc3/apu/bapu/dsp/SPC_DSP.cpp),
  commit `8028d3b6ca994f21fdff220d513a08382e37afc3`: V3b/V3c read the header
  and select terminal release separately from V4's interpolation-demand
  decoding. V4–V7 separate the block-end event from ENDX publication.
  The LGPL source was inspected as attributed research only; no implementation
  code or emulator-generated expected output was imported or executed.

These sources support removing indefinite pitch-gated header omission. They
do not establish the exact scalar call on which hardware prefetch or register
publication occurs. The NON/PMON progression dispute recorded in earlier
validation remains unresolved; PMON is disabled in all new vectors.

## Scalar API boundary

After the fifth existing silent KON call, `sample` inspects the stable initial
header at the selected start address. Flags `01` set E=0 and release. Therefore
the first subsequent ordinary call publishes zero PCM/ENVX/OUTX even at pitch
$0000 or $0001. The inspection does not advance the BRR address, interpolation
phase, history or nibble count, preload an ordinary waveform, or set ENDX.
It adds no decoder state or artificial pitch increment. The existing decoder
still reads the header with the first demanded nibble and handles later blocks.

This explicitly supersedes the earlier assumption that **initial** header
visibility requires nibble consumption. Later-header eager decoding and scalar
ENDX boundaries retain Issue #79's mapping. Five silent calls and the first
ordinary publication are API conventions, not physical subcycle measurements.
Only directory entries and initial headers stable before KON and throughout
initialization are covered. Live-header edits and writes racing KON are excluded.

Release persists across direct GAIN/ADSR edits, NON changes and pitch changes;
KON initializes a new voice. Raising pitch resumes silent BRR consumption,
end-bit redirection through the live loop word, and sticky ENDX clear/reassert.
No initialization or release behavior of another voice is changed.

## Independent vectors and arithmetic

[`dsp_brr_end.rs`](../crates/snes-core/tests/dsp_brr_end.rs) contains four new
original public-API tests alongside the six retained Issue #79 tests.
Expected values use attributed semantics and explicit arithmetic, never
production helpers, private state or captured emulator output.

The initial-header matrix covers all 32 combinations of flags `00/01/10/11`,
pitch $0000/$0001, direct GAIN/fast ADSR and NON off/on. Every call in the
4095-call post-delay hold checks PCM, ENVX, OUTX and ENDX. Pitch 1 cannot demand
its first nibble until call 4096; pitch 0 never demands one. The five preceding
KON calls separately require zero PCM and registers, including ENDX.

FLG $20 holds noise seed $4000, whose signed source is -32768. Direct GAIN 64
sets E=1024; fast ADSR $8f/$e0 sets E=1024 on the first ordinary call and E=2047
thereafter (sustain rate zero). After envelope scaling and even truncation,
O=-16E. Voice volumes +64/-128 and master +64/+64 give `(O>>2, (-O)>>1)`.
ENVX is E>>4 and OUTX is the byte O>>8:

| Initial flags/source | E | O | Stereo PCM | ENVX | OUTX | ENDX during hold |
| --- | ---: | ---: | --- | ---: | ---: | ---: |
| `01`, either source/envelope mode | 0 | 0 | (0,0) | 0 | 0 | 0 |
| `00/10/11`, NON, GAIN64 or first ADSR call | 1024 | -16384 | (-4096,8192) | 64 | 192 | 0 |
| `00/10/11`, NON, later ADSR calls | 2047 | -32752 | (-8188,16376) | 127 | 128 | 0 |
| `00/10/11`, BRR before demand, GAIN64 | 1024 | 0 | (0,0) | 64 | 0 | 0 |
| `00/10/11`, BRR before demand, later ADSR | 2047 | 0 | (0,0) | 127 | 0 | 0 |

The audible nonterminal controls reject blanket zero-pitch muting. NON-off
controls distinguish silent interpolation history from terminal envelope release.

The persistence test holds terminal voice 0 at zero pitch for 64 calls while
editing GAIN/ADSR and toggling NON. An independent nonterminal voice 1 remains
at E=1024 with held noise, contributing right PCM 8192 and ENVX/OUTX 64/192.
After raising voice 0 to $1000, original blocks form
`A01 -> B00/B+9:11 -> C10/C+9:11 -> D11`. Stable loop-word edits at counts 24
and 56 select C and D. ENDX is required exactly at counts **17,49,81,97** and
cleared after each event. The first 16 consumed nibbles still belong to A;
only the following demand reports its completion. These counts also reject
manufactured nibble/phase advancement during the zero-pitch hold.

Returning to pitch zero for 64 calls preserves release and clear ENDX. A new
KON selects a nonterminal start at D+9, distinct from loop D. The five-call delay
affects only voice 0, after which NON again yields left PCM -4096/ENVX 64/OUTX
192. Turning NON off and raising pitch checks every sample of the distinct
start transient and following loop using original filter-0 values +3072/+1024
and independent phase-zero Gaussian weights `[370,1305,374,0]/1024`. Individual
products shift before summation and even truncation. Returning to zero pitch
and NON, ordinary KOFF checks all 128 steps E=1024-8t through zero. Clearing
KOFF and changing GAIN/ADSR cannot revive it. Voice 1 stays audible throughout.

## Original uploaded SPC700 guest

`zero_pitch_guest` emits original instruction bytes uploaded to $0200 through
the existing firmware-free protocol. It sets zero pitch, NON, PMON=0 and other
DSP registers through $F2/$F3, selecting original terminal A and distinct loop
B data. B has 64 sequential blocks, the last `11`, leaving wide stable ENDX
windows. C is a distinct `11` loop; D is the distinct nonterminal start selected
by SRCN 1 on retrigger. BRR/directory bytes are original fixture data in RAM.

At each absolute output sample checkpoint the host sends the next command.
Before acting, the guest reads ENVX/OUTX/ENDX through $F2/$F3 into successive
triples at $1100..$1117:

| Sample | Recorded triple | Action after recording | Preceding 20 PCM samples |
| ---: | --- | --- | --- |
| 100 | (0,0,0) | GAIN127, fast ADSR, NON off | (0,0) |
| 200 | (0,0,0) | NON on, pitch $1000 | (0,0) |
| 300 | (0,0,1) | Clear ENDX, change live loop word to C | (0,0) |
| 400 | (0,0,0) | Pitch zero, direct GAIN64 | (0,0) |
| 500 | (0,0,0) | Resume pitch $1000 | (0,0) |
| 1500 | (0,0,1) | Pitch zero, SRCN 1, new KON | (0,0) |
| 1600 | (64,192,0) | NON off, pitch $1000 | (-4096,8192) |
| 1700 | (64,12,1) | STOP | (768,-1536) |

Before sample 300, more than 16 but fewer than 1040 post-delay nibbles have
been consumed, so A has ended while long B has not. The pause at 400–500 does
not consume BRR data. By sample 1500 the resumed total exceeds 1040 nibbles, so
B has completed and C is looping. These wide bounds allow instruction execution
around the commands without asserting physical fetch alignment. On retrigger,
zero pitch leaves ENDX clear; pitch resumption passes start D into loop C before
1700. C's nibble +6 decodes to 3072, giving settled interpolated source 6146,
O=3072, ENVX=64, OUTX=12 and PCM (768,-1536). Noise and terminal anchors follow
the independent arithmetic above; stored audio is normalized by 32768.

The uploader requires fewer than 512 bytes, exact uploaded code and readiness
before sample 100. Each acknowledgement is bounded to 1000 advances of 21
master clocks; command acknowledgements and expected records must arrive by
the ten-sample follow-up. Both CPUs must STOP by sample 1710 without errors.
The test also checks current DSP registers before commands, the edited loop
word and selected SRCN. See [MIT authorship/provenance](../tests/roms/PROVENANCE.md#issue-81-original-zero-pitch-terminal-header-vectors-and-spc700-guest).

Two APUs run whole intervals versus repeating irregular chunks
`[1,7,666,1001,21,8193]`, targeting
`ceil(N*32*21477272/1024000)` master clocks including upload. At all eight
checkpoints and their ten-sample follow-ups they compare **every PCM sample,
all 128 DSP registers, all 64 KiB RAM and all four output ports**. The same
independent PCM/register anchors must pass. Equality proves determinism only.

## Executed baseline evidence

An isolated detached worktree was created at the evaluation baseline. Only the
public test file was copied into it; its production code was unmodified:

```sh
git worktree add --detach .cache/issue-81/baseline f4ca3ce3d8c10850da72924e0926ec5f40b2a68d
cp crates/snes-core/tests/dsp_brr_end.rs .cache/issue-81/baseline/crates/snes-core/tests/dsp_brr_end.rs
CARGO_TARGET_DIR="$PWD/.cache/issue-81/baseline-target" cargo test \
  --manifest-path .cache/issue-81/baseline/Cargo.toml --locked \
  -p snes-core --test dsp_brr_end zero_pitch_terminal_noise_is_silent_before_any_decode
# Also run the entire file with the same command, omitting the final filter.
```

The filtered discriminator compiled and **failed behaviorally**, reporting on
the sixth KON call:

```text
actual:   ((-4096, 8192), 64, 192, 0)  # PCM, ENVX, OUTX, ENDX
required: ((0, 0), 0, 0, 0)
```

The complete file compiled: **6 retained tests passed, all 4 new tests failed**.
The matrix failed at pitch 0/GAIN64/NON off/flags 1/first ordinary call:
actual `((0,0),64,0,0)`, required `((0,0),0,0,0)`. The two-voice persistence
test produced `(-4096,8192)` instead of `(0,8192)`. The uploaded guest produced
normalized PCM `(-0.125,0.25)` instead of `(0,0)` at sample 81. After correction,
all ten tests pass, including the exact sixth-call zero tuple. These are
behavioral discriminators, not compilation failures or hardware oracle captures.

## Mandatory validation

Host: macOS arm64; native Rust 1.94.0; rustup stable Rust 1.99.0 selected for
spcasm and Wasm using the README `RUSTC` override; Node 24.12.0, Python 3.13.13.
Detailed local logs are in `.cache/issue-81`; results are preserved here.

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --locked -p snes-core --test dsp_brr_end` | Passed: 10 tests, zero failed/ignored |
| `cargo test --workspace --locked` | Passed: 424 tests, zero failed/ignored; retained BRR, directory, noise, pitch, envelope, mixing, echo and other regressions |
| `python3 tools/make-fixture.py .cache/smoke.bin` and `cmp tests/roms/smoke.bin .cache/smoke.bin` | Passed, byte-identical |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Passed: all three tools built from unchanged pins |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Passed: CPU full/basic and SPC, 600 frames each, terminal markers 0649/0452/0557 |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Passed: 11 grayscale bands of 16 scanlines, final shade persists through line 223 |
| `python3 tools/build-cleared-games.py` twice | Passed: two fresh builds with unchanged expected Castle/Memory hashes |
| `npm --prefix web ci` | Passed: zero audit vulnerabilities |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Passed |
| `cd web && npx playwright install chromium` | Passed |
| `cd web && RUSTC="$(rustup which rustc)" SNES_TEST_PORT=18085 npm test -- --grep-invert '301 seconds'` | Passed: all 37 selected checks, zero skips, 2.8 minutes |
| `git diff --cached --check` | Passed |

No mandatory gate failed, was skipped or remains unperformed. The intentional
baseline failures above are the regression evidence. The browser gate also built
all eight README CPU fixtures. Fixture/tool source pins, provenance, expected
hashes and browser assertions are unchanged. Smoke, CPU full/basic, SPC, HDMA and cleared-game hashes match
the [Issue #79 hash table](issue-79-validation.md#mandatory-validation).
The browser gate retains Castle/Memory's 18,100-frame replays, Castle's complete
five-map/credits route and freeze control, Memory gameplay assertions, numerical
audio and live Web Audio checks. The grep excludes only the existing optional
unaudited tests named `301 seconds`; no commercial ROM or firmware was acquired.

## Inventory and remaining limits

All 24 categories and their full inherited scopes/outstanding work remain as
listed below, retained from [Issue #73](issue-73-validation.md#inventory-and-remaining-work).
There are **22 partial categories, two unimplemented, none verified**.

| Category | Status | Retained scope and limitations |
| --- | --- | --- |
| `cpu` | partial | Complete 65C816 semantics, interrupt edges, bus order/count and revision-specific timing; existing licensed suites do not prove all edges |
| `bus` | partial | Cartridge maps, mirrors, I/O and open bus; unsupported maps/chips and electrical floating values remain |
| `scheduler` | partial | CPU/PPU/APU/DMA synchronization and NTSC events; exact access alignment, arbitration and physical blanking latency remain |
| `dma` | partial | Transfer modes, direction, access restrictions and WRAM conflicts; full timing, electrical values and interaction matrix remain |
| `hdma` | partial | Initialization, descriptors, counters, direct/indirect reloads and phases; exact startup/overheads, arbitrary enabling and same-channel contention remain |
| `controllers` | partial | Serial and automatic joypad reads, busy/completion and input; complete device/protocol/timing coverage remains |
| `ppu-bg` | partial | Planar tiles/maps, scroll, priorities, direct color and mosaic; complete mode interactions and timed mosaic state remain |
| `ppu-offset` | partial | Modes 2/4/6 H/V descriptors, sizes and boundaries; mosaic interactions and exact fetch/raster behavior remain |
| `ppu-hires` | partial | Ordinary Mode 5/6 and math-disabled SETINI output; hires-math dispute, color-window clipping/prevention, ordinary-hires composition, mosaic, full OBJ behavior, interlace/overscan and switching/fetch timing remain |
| `ppu-mode7` | partial | Transform/direct color and identity pseudo-hires control; EXTBG and complete transform/overflow/edge behavior remain |
| `ppu-objects` | partial | OAM attributes, rendering, priorities and simple screen/window routing; physical evaluation/fetch and overflow assertion timing, rotation oddities, complete size/mode interactions, interlace and hires behavior remain |
| `ppu-color` | partial | Palettes, direct color, composition, brightness and math; comprehensive windows, clipping/prevention, subtraction/saturation, halving and physical DAC behavior remain |
| `ppu-ports` | partial | VRAM/OAM/CGRAM ports, access ownership, latches and counters; exact rendering-owned corruption, latch retention, access latency and pixel-fetch arbitration remain |
| `ppu-raster` | unimplemented | Dot-level pipeline, mid-scanline writes, fetch arbitration and physical mode-switch timing |
| `ppu-display` | partial | Packed framebuffer, geometry, blanking and display shape; interlace/overscan, physical scanout and switching timing remain |
| `spc` | partial | SPC700 instructions, timers and communication; full timing and register/RAM races remain |
| `ipl` | partial | Firmware-free high-level upload; IPL instruction timing and general firmware execution remain |
| `dsp-voices` | partial | BRR decode, ordered dry/send mixing, interpolation and key-on behavior; independent BRR-filter 1–3 evidence, exhaustive interpolation phases and simultaneous key/mixing combinations remain |
| `dsp-noise` | partial | Shared noise generator/rates and NON routing; DSP subcycle sampling and write races remain |
| `dsp-pitch` | partial | Pitch/PMON, interpolation advancement and noise interaction; subcycle PMON sampling and torn/live writes remain |
| `dsp-envelope` | partial | ADSR/GAIN shared rates, release/retrigger and boundaries; hidden/bent GAIN, exhaustive transitions and live interactions remain |
| `dsp-echo` | partial | Fixed-register FIR/feedback arithmetic, saturation, even writeback and numerical PCM/RAM vectors; live ESA/EDL sampling, subcycle register/RAM races, SPC bus synchronization, physical startup/reset retention remain |
| `dsp-timing` | partial | Deterministic sample cadence and browser queues; DSP subcycles, exact hardware timing and audible fidelity remain |
| `pal` | unimplemented | PAL timing, video geometry and audio synchronization |

These scopes include all prior Issue #75/#77/#79 qualifications. Physical
KON/header lookahead and publication timing, live-header sampling, directory/key
races (including KON-delay selection changes), torn directory/pitch writes,
SPC/DSP/echo RAM interactions and reset retention remain unresolved. Independent
BRR-filter 1–3 evidence, exhaustive interpolation phases and simultaneous-key
combinations remain outstanding. Ordinary zero-pitch waveform preloading and a
complete lookahead pipeline are not implemented. NON/PMON progression remains a
separate dispute. Hidden-envelope corrections and physical audio fidelity are
not established by these tests.

No DSP subcycle/SPC bus scheduler, firmware execution, frontend change or unrelated
refactoring is included. Passing these bounded tests establishes neither full
hardware accuracy nor commercial compatibility and does not complete Issue #3.
No optional validation or post-merge verification is required by Issue #81.
