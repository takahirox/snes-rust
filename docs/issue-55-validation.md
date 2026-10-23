# Issue #55: non-interlaced rectangular OBJ geometry

## Specification contract

Read [Issue #55](https://github.com/takahirox/snes-rust/issues/55) and the
following specifications before changing production geometry. Retrieved the
source files on 2026-10-24; their SHA-256 values match the prior
[specification record](issue-53-validation.md#specification-contract-recorded-before-pixel-expectations).

- [Anomie's Register Doc](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt),
  revision 1157, 2007-07-12; SHA-256
  `19dfdf3f1c206655b8ede11bcd6dc053e36634f876819e40bb39cfb88a237dfd`.
  Sections OBSEL ($2101) and SPRITES (OAM, Character table in VRAM): size
  selection, wrapped Y, flips, and the explicit exception that rectangles
  reverse rows independently within their two stacked squares.
- [Fullsnes: SNES PPU Sprites (OBJs)](https://problemkaputt.de/fullsnes.htm#snesppuspritesobjs)
  and [Video Memory: 16x16 (and bigger) Tiles](https://problemkaputt.de/fullsnes.htm#snesppuvideomemoryvram),
  no$sns v1.5 extraction; SHA-256
  `3853b65a7331bf12eea431ff494a2948a1c78ef22f7e819db4b139fd5a79f6c6`.
  The size table explicitly documents selectors 6/7 despite the introductory
  reserved label. The OAM high table supplies the individual size bit.
  Character column and row carries stay within their four-bit fields and do
  not change the name-table bit.

Selected dimensions, in pixels (width x height):

| OBSEL selector | Size clear | Size set |
| --- | --- | --- |
| 0 | 8x8 | 16x16 |
| 1 | 8x8 | 32x32 |
| 2 | 8x8 | 64x64 |
| 3 | 16x16 | 32x32 |
| 4 | 16x16 | 64x64 |
| 5 | 32x32 | 64x64 |
| 6 | 16x32 | 32x64 |
| 7 | 16x32 | 32x32 |

For destination `(col,row)`, inclusion uses `row=(screen_y-Y) mod 256 < H`,
while horizontal iteration uses `0 <= col < W`. H flip selects `W-1-col`.
Write the row as `q*W+r`; vertical flip selects `q*W+(W-1-r)`, preserving `q`.
Since W is a power of two, this equals `row XOR (W-1)`. The rectangular tile-row
orders are therefore `1,0,3,2` and `3,2,1,0,7,6,5,4`; a square has only one block.
These equations are derived from Anomie's exception, not a full-height reversal.

Character addressing retains the existing separate low/high-nibble increments,
OBSEL base/name gap, OAM name bit and 16-bit VRAM byte wrap. Only the dimensions,
vertical inclusion, horizontal extent and flip coordinates change. Palettes,
transparency, priorities, routing, ports, scheduler and existing evaluation
approximations are unchanged.

## Independent coverage and CPU cartridges

Original MIT tests: `crates/snes-core/tests/ppu_obj_sizes.rs` and
`crates/snes-core/tests/common/obj_sizes.rs`. All eight selectors and both size
bits are covered with all four flip combinations, including square controls.
The geometry oracle constructs an asymmetric linear image, reverses the row
lists of separately cropped squares and/or each row's pixels, then compares
all pixels and the one-pixel backdrop border. It never calls production
addressing/rendering helpers or computes the production XOR transform.
This checks within-tile pixels, every tile and half boundary, last included
columns/rows, and immediately adjacent excluded pixels.

Independent signed image crops cover X=-5, X=249 and Y=248 (local row 8 appears
at display row 0 after 255-to-0 wrap), for all rectangular sizes/flips. Only one
object intersects the sampled rows. Explicit character matrices starting at
$FF cover both nibble wraps, both name tables and all flips of a 32x64 object.
For OBSEL=$DF the unwrapped base is $1C000 and the second-table gap is $8000;
the byte addresses reduce to $C000 and $4000. Distinct images in both tables
make accidental name-bit carry or wrong base/gap selection observable.

The source-built 32 KiB LoROMs contain original 65C816 LDA/STA initialization,
planar pixels, palette, header and vectors. CPU instructions initialize relevant
registers, VRAM, CGRAM and all 544 OAM bytes under forced blank. Each cartridge
selects 6 or 7 and displays four objects: small, large, small V-flipped and
large V-flipped. The highest active row has four objects and twelve character
columns; unused small objects start at Y=224 and do not wrap into visible rows.
No commercial ROM, firmware, font or external graphics were acquired.

The host supplies only the cartridge, steps execution and observes markers and
pixels. Core tests require five frames within 300,000 instructions, reject STP,
and require `$30=$55`, `$31=selector`. Browser tests load the same cartridges
through the local file chooser and compare literal vectors against both actual
Wasm framebuffer bytes and canvas ImageData. They have a 30-second test timeout
and five-second polling bound; neither host patches PPU/CPU/memory state.
The vectors include the lower halves, the 16/32-row flip boundary, final rows
and backdrop. For example small V-flipped local row 16 takes source row 31,
while a full-height reversal would take source row 15 and a different color.

Build and focused browser reproduction (after the web build and Chromium setup):

```sh
cargo run --locked -p snes-core --example obj-sizes-fixture -- .cache/obj-sizes
SNES_TEST_PORT=18085 npm --prefix web test -- --grep 'OBJ sizes:'
```

Both browser tests also build these cartridges directly from the shared source.
Generated outputs stay ignored; source and generated code/data are MIT. See
[PROVENANCE](../tests/roms/PROVENANCE.md#original-rectangular-obj-guests-and-graphics-approved).

| Cartridge | SHA-256 |
| --- | --- |
| `obj-sizes-6.sfc` | `7e199c553feb40a0b8d31219678fc47e18a0f95836c872b754b21854d642a846` |
| `obj-sizes-7.sfc` | `ea0975ba98501585e928a96e1a42288bd891b44d53fd9d2c74cfe2d6a39b11e6` |

## Exact baseline behavioral counterexamples

Created a detached worktree at the exact evaluation baseline
`de7796f9d82c01b321f906a7b974df76eb23d4fc`. Copied only the final new test and
shared cartridge builder, leaving baseline production and pixel expectations
unchanged. Compilation succeeded. The test command returned exit 101:
**six behavioral failures, one passing square-control test**.

| Test/vector | Baseline observed | Required |
| --- | --- | --- |
| Selector 6 small, (24,40), local row 16 | black backdrop | cyan `[0,255,255,255]` |
| Selector 7 small, (24,40), local row 16 | black backdrop | cyan `[0,255,255,255]` |
| Selector 6 large, (24,56), local row 32 | black backdrop | green `[0,255,0,255]` |
| Nibble/name/address matrix, (24,56) | black backdrop | blue `[0,0,255,255]` |
| Left-clipped crop, (0,40) | black backdrop | magenta `[255,0,255,255]` |
| CPU cartridge 6, (24,40) | black backdrop | cyan `[0,255,255,255]` |

The separate `square_selectors_and_all_flips` test passed unchanged, including
selector 7's large 32x32 control. All seven tests pass with the implementation.
These are pixel failures, not a baseline build/toolchain failure.

```sh
git worktree add --detach .cache/issue-55/baseline de7796f9d82c01b321f906a7b974df76eb23d4fc
cp crates/snes-core/tests/ppu_obj_sizes.rs .cache/issue-55/baseline/crates/snes-core/tests/
cp crates/snes-core/tests/common/obj_sizes.rs .cache/issue-55/baseline/crates/snes-core/tests/common/
# Expected exit 101 on the exact baseline:
cargo test --manifest-path .cache/issue-55/baseline/Cargo.toml --locked -p snes-core --test ppu_obj_sizes
```

## Mandatory validation (2026-10-24)

All Rust checks explicitly select the CI toolchain bin directory to avoid the
Homebrew Cargo/Clippy precedence documented in Issue #53:

```sh
RUST99_BIN="$(dirname "$(rustup which --toolchain 1.99.0 cargo-clippy)")"
export PATH="$RUST99_BIN:$PATH"
export RUSTC="$RUST99_BIN/rustc"
cargo --version
rustc --version
cargo clippy --version
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo test --locked -p snes-core --test ppu_obj_sizes
cargo test --locked -p snes-core --lib oam
cargo test --locked -p snes-core --test ppu_oam_access --test ppu_oam_reload --test ppu_hires --test ppu_pseudo_hires --test direct_color
python3 tools/make-fixture.py .cache/smoke.bin
cmp tests/roms/smoke.bin .cache/smoke.bin
python3 tools/build-test-tools.py --spc
python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"
python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"
python3 tools/build-cleared-games.py
python3 tools/build-cleared-games.py
npm --prefix web ci
npm --prefix web run build
(cd web && npx playwright install chromium)
SNES_TEST_PORT=18085 npm --prefix web test -- --grep 'OBJ sizes:'
SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'
```

Selected versions: Cargo `1.99.0 (5f94df478 2026-08-27)`, rustc
`1.99.0 (b940084d7 2026-09-28)` and Clippy
`0.1.99 (b940084d7e 2026-09-28)`. Formatting and warning-denying Clippy passed.
Workspace: **283 tests passed**. Focused: **7 OBJ geometry/CPU tests**, **17 OAM
unit tests**, and **32 PPU/direct-color regression tests** passed.

The first guest run caught three incorrectly transcribed literal colors in the
new test; they were corrected from the original image coordinates, before the
baseline comparison. No production pixel output was used as the oracle.
The initial Rust 1.99 Wasm link failed because its local `rust-lld` could not
find the installed `libLLVM.dylib`; an environment-only override did not
resolve it, and the first wrapper attempt supplied a redundant linker flavor. The final local wrapper sets the library path inside
the linker process and forwards rustc's original arguments. The Wasm build and
two focused browser tests then passed using the same Rust 1.99 compiler and
linker. The initial browser command lacked a built site and timed out during
server startup; it did not execute pixel assertions. No tracked build tooling
was changed to accommodate this host setup.

Exact local wrapper reproduction, before `npm --prefix web run build`:

```sh
mkdir -p .cache/issue-55
python3 - <<'PY'
from pathlib import Path
import shlex, subprocess
compiler = Path(subprocess.check_output(
    ['rustup', 'which', '--toolchain', '1.99.0', 'rustc'], text=True).strip())
root = compiler.parent.parent
wrapper = Path('.cache/issue-55/wasm-ld')
wrapper.write_text('#!/bin/sh\nexport DYLD_LIBRARY_PATH=' +
    shlex.quote(str(root / 'lib')) + '\nexec ' +
    shlex.quote(str(root / 'lib/rustlib/aarch64-apple-darwin/bin/rust-lld')) +
    ' "$@"\n')
wrapper.chmod(0o755)
PY
export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER="$PWD/.cache/issue-55/wasm-ld"
npm --prefix web run build
```

Smoke regeneration matched byte-for-byte, SHA-256
`73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb`.
Pinned assembler builds passed. The auxiliary SPC assembler and native release
runner retained the existing `rust-objcopy`/`libLLVM.dylib` strip warning; their
builds and execution succeeded. Hardware suites passed: full CPU last test
$0649 / PC $0081A2, basic CPU $0452 / PC $0081A2, SPC $0557 / PC $008060.
HDMA passed eleven 16-row grayscale bands and the final shade through row 223.
Both cleared-game builds passed with unchanged hashes:
Castle `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`,
Memory `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
npm installation/audit (zero vulnerabilities), final release Wasm build and
Chromium setup passed. The focused actual Wasm/canvas checks passed **2 tests**.

The full required browser filter passed **23 tests in 2.9 minutes**, including
Castle/Memory five-minute replays, Castle route/credits, music/effects, Web Audio
queue/drift checks, freeze negative control, persistence failure/recovery,
ordinary-hires and SETINI transitions, Pages subpath loading, and both new OBJ
Wasm/canvas checks. No mandatory check is skipped or remains failing. Optional
external-game tests remain excluded by the requested `301 seconds` filter.

Logs remain under `.cache/issue-55/`; hardware evidence is in
`.cache/hardware-tests/results.json` and `.cache/ppu-test/results.json`.
Browser artifacts are under `web/test-results`. The baseline's expected
behavioral failures and resolved local-toolchain failures are separate from
the final passing gates.

## Remaining uncertainty and inventory

All 24 categories retain their complete scopes and statuses from the
[Issue #53 inventory](issue-53-validation.md#remaining-uncertainty-and-inventory)
and its linked prior records. This change adds bounded non-interlaced OBJ
geometry evidence within `ppu-objects`. It does not narrow any category to the
tested subset or promote any category to verified.

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
| `ppu-objects` | partial | OAM attributes, rendering, priorities and simple screen/window routing; complete overflow, rotation, size and mode interactions remain |
| `ppu-color` | partial | Palettes, direct color, composition, brightness and math; comprehensive windows, clipping/prevention, subtraction/saturation, halving and physical DAC behavior remain |
| `ppu-ports` | partial | VRAM/OAM/CGRAM ports, access ownership, latches and counters; exact rendering-owned corruption, latch retention, access latency and pixel-fetch arbitration remain |
| `ppu-raster` | unimplemented | Dot-level pipeline, mid-scanline writes, fetch arbitration and physical mode-switch timing |
| `ppu-display` | partial | Packed framebuffer, geometry, blanking and display shape; interlace/overscan, physical scanout and switching timing remain |
| `spc` | partial | SPC700 instructions, timers and communication; full timing and register/RAM races remain |
| `ipl` | partial | Firmware-free high-level upload; IPL instruction timing and general firmware execution remain |
| `dsp-voices` | implemented_unverified | BRR decode, voice mixing, interpolation and key-on behavior; exhaustive BRR filters, interpolation phases and hidden behavior remain |
| `dsp-noise` | partial | Shared noise generator/rates and NON routing; DSP subcycle sampling and write races remain |
| `dsp-pitch` | partial | Pitch/PMON, interpolation advancement and noise interaction; subcycle PMON sampling and torn/live writes remain |
| `dsp-envelope` | partial | ADSR/GAIN shared rates, release/retrigger and boundaries; hidden/bent GAIN, exhaustive transitions and live interactions remain |
| `dsp-echo` | partial | FIR/echo and feedback; reference saturation and exhaustive echo behavior remain |
| `dsp-timing` | partial | Deterministic sample cadence and browser queues; DSP subcycles, exact hardware timing and audible fidelity remain |
| `pal` | unimplemented | PAL timing, video geometry and audio synchronization |


`ppu-objects` remains partial: overflow flags, exact evaluation/fetch limits,
priority-rotation oddities, X=-256, OBJ interlace, hires interactions and fetch/
raster timing remain outstanding. Complete size/mode interactions also include
domains outside these static non-interlaced vectors. Hires math and clipping,
mode interactions, and every other inventory scope above remain unresolved.
Passing these checks establishes neither complete hardware accuracy nor
commercial compatibility and does not complete Issue #3. Existing Castle/Memory
sources, pins, output hashes, replay inputs, durations, gameplay/audio assertions
and negative controls are unchanged. Optional extra BG/window scenes and
unaudited external games were not required or run. No post-merge verification
is required by this Issue.
