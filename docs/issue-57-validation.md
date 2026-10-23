# Issue #57: non-interlaced OBJ limits and STAT77

## Specification prerequisite (recorded before implementation and expectations)

Read the live [Issue #57](https://github.com/takahirox/snes-rust/issues/57).
Retrieved both raw specifications on 2026-10-24 into ignored `.cache/issue-57/`.
Their SHA-256 values exactly match the Issue #55 record:

- [Anomie's Register Doc](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt),
  revision 1157, 2007-07-12, SHA-256
  `19dfdf3f1c206655b8ede11bcd6dc053e36634f876819e40bb39cfb88a237dfd`.
  Sections STAT77 ($213E), SPRITES / Drawing the Sprites, Sprite Priority,
  OAM format, and INIDISP ($2100).
- [Fullsnes](https://problemkaputt.de/fullsnes.htm), extracted from no$sns v1.5,
  SHA-256 `3853b65a7331bf12eea431ff494a2948a1c78ef22f7e819db4b139fd5a79f6c6`.
  Sections SNES PPU Sprites (OBJs), Timers and Status / STAT77,
  SNES Timings event table, and Timing PPU Memory Accesses / PPU OAM load.

The following is the supported contract, not an inference from the old loops.
Anomie's explicit drawing algorithm supplies horizontal exceptions and ordering;
Fullsnes independently gives the three-stage 128 -> 32 -> 34 process and
qualifies the frame reset with forced blank. Its introductory reserved label
for selectors 6/7 disagrees with its explicit size table; the latter agrees with
Anomie, as resolved in Issue #55.

| Stage / condition | Selection, admission or lifetime rule | Basis |
| --- | --- | --- |
| Vertical inclusion | Local row is `(display row - OAM Y) mod 256`; compare against height, including rectangular lower halves | Anomie OAM format, OBSEL; retained Issue #55 geometry |
| Horizontal selection | Require `-width < signed X < 256`; encoded X=$100 (-256) is treated as zero for both budgets only | Anomie Drawing steps 0-1; width follows horizontal extent, independently of height |
| Object limit | Select the first 32 eligible objects; only a 33rd eligible object sets RANGE OVER ($40), and it is not selected | Anomie step 1 and STAT77; Fullsnes OAM load |
| Selection order | Start at FirstSprite, advance OAM index with 127 -> 0 wrap | Anomie Sprite Priority and step 1 |
| Stable rotation | With $2103.7 set, FirstSprite is `(internal OAM word address & $FE) >> 1`; otherwise zero | Anomie Sprite Priority; odd byte/scanline-dependent rotation remains outside scope |
| Tile admission order | Traverse selected objects in reverse selection order; within each object traverse destination character columns left to right after flip | Anomie step 2; after-flip order is spatial, so H flip changes source identity, not destination traversal |
| Horizontal tile eligibility | Tile left edge must satisfy `-8 < X < 256`; both partial edges count, even if the first pixel is clipped. X=-8 and X=256 do not count. X=-256 uses zero plus column offset for counting | Anomie steps 0 and 2 |
| Tile limit | Admit first 34 eligible eight-pixel tiles; a 35th sets TIME OVER ($80) and is rejected along with all later candidates | Anomie step 2 and STAT77; Fullsnes OAM load |
| Pixel priority | Only admitted tiles draw, at their true signed X. Earlier selected objects cover later ones regardless of OAM BG-priority bits; transparent pixels do not cover | Anomie step 3 and Sprite Priority; fetch order is not pixel priority |
| Transparency / overlap | Neither reduces either budget | Derived from geometric selection/admission before character decoding in Drawing steps 1-3 |
| Routing / windows | Neither reduces either budget, even with OBJ disabled on both screens | Explicit STAT77 enable independence plus downstream screen composition/window stages |
| Status bits | OR $40/$80 into persistent state; reaching exactly 32/34 does not set the corresponding bit | Both STAT77 descriptions |
| Reads | Repeated reads preserve flags; retain PPU1 open-bus bit 4 and version 1, update the normal PPU1 read bus | STAT77 specifies frame reset, unlike explicit read acknowledgement on STAT78 |
| Non-overflow line / VBlank | Preserve accumulated flags | Both STAT77 descriptions: reset at end of VBlank, not per line |
| Frame reset | At V=0 clear flags only if forced blank is inactive; forced blank at that boundary preserves them | Fullsnes STAT77 and timing table qualify Anomie's unqualified end-of-VBlank reset |
| Forced blank | Does not itself clear flags. No new rendering/evaluation during forced blank; resume evaluation on subsequent rendered rows | Fullsnes reset qualification; Anomie INIDISP says rendering buffers are not updated during forced blank |

Fullsnes gives assertion coordinates in its status section, but its timing table
marks the relevant H coordinates with question marks and its memory timing text
explicitly contains guesses. Anomie gives no exact assertion strobes. This change
does **not** claim those physical strobes: evaluation and both flag assertions
occur together at the existing HBlank render event for V=1..224, using framebuffer
row `V-1`. Thus framebuffer row zero (OAM Y=0) is evaluated on V=1 in this
renderer, whereas hardware can select it on V=0 for the following line.
Frame clearing occurs on scheduler entry to V=0, even though no framebuffer row
is drawn there; it is not deferred to row-zero rendering. Initial state is clear.
Forced-blank changes inside physical evaluation/fetch windows, partial fetches,
interlace, overscan, PAL and rendering-time OAM corruption are outside this model.
Tests will synchronize away from these unresolved strobes and explicitly test
the chosen scheduler convention, not present it as dot-accurate hardware.

The older Issue #55 record deliberately avoided overflow and listed it as
outstanding. Its geometry remains applicable; its old forward single-pass
evaluation is not evidence for survivor order or flag lifetime.

## Implementation and independent evidence

Selection stores at most 32 geometric candidates. Tile admission walks that list
backward, admits at most 34 destination character columns and stops globally on
candidate 35. Each admitted tile composes back to front, so the earlier selected
object's opaque pixels replace later ones without confusing BG priority with
OBJ priority. Rejected tiles never contribute pixels. The only persistent new
state is the two overflow bits; ordinary STAT77 reads keep their existing bus
update path. Scheduler frame rollover performs the conditional reset separately
from row rendering. Forced-blank rows return before evaluation.

Original MIT `ppu_obj_limits` tests use authored OAM scenes and literal pixel
survivors, not production helpers or a copied evaluator oracle. They cover:

- Independent 32/33 object and 34/35 tile boundaries, neither/both flags,
  transparent and overlapping candidates, and distinct accepted/rejected tiles.
  Range-rejected large objects do not enter the tile budget of 32 selected
  small objects.
- Reverse object admission, a budget exhausted inside an object, both H-flip
  directions, pixel priority independent of BG priority, and stable programmed
  rotation across index 127 -> 0 for both selection and admission.
- Fully offscreen X=-8/-9/-255, X=-256's special counting without drawing,
  partial left/right tiles at aligned and unaligned positions, and both
  rectangular lower halves. The left probe positions -16/-15/-8/-7/-1 and
  right positions 240/241/247/248/249/255 have literal candidate counts.
- Every OBSEL selector and both size bits; width determines the tile budget and
  horizontal inclusion, height determines vertical inclusion. Y=248 wraps into
  row zero and a lower 16x32 half. Transparent front objects reveal admitted
  opaque back objects. Disabled TM/TS or full-screen OBJ windows do not save
  either budget. Independent lines accumulate different flag bits.
- Exact STAT77 masks and complete $11/$D1 read values, PPU1 bus propagation,
  repeated reads and forced-blank retention. A timed bus-only test advances
  naturally across frames and V=0/V=1, checking row-zero evaluation at HBlank;
  it neither assigns coordinates nor injects flags.

The shared original CPU cartridge builder is
`crates/snes-core/tests/common/obj_limits.rs`; the example emits four 32 KiB
LoROMs with independent expected flags $00/$40/$80/$C0. Executed CPU
instructions initialize video through registers, synchronize with IRQ/WAI and
VBlank polling, save masked actual results at $1800..$1808, and compare literal
expected values while retaining a sticky pass byte in $31. After these checks
the guest restores its original scene, publishes $30=$57 / $32=mode, and loops.
Core startup allows fewer than eight frames and at most 300,000 instructions,
rejecting STP, followed by three complete frames for final image observation.
Hosts only load, advance and observe. Browser tests use the normal ROM picker,
a 30-second deadline and a five-second poll bound, comparing literal positive
and negative survivor vectors in actual Wasm bytes and canvas ImageData.
No debug API was added, and no host patches CPU/PPU/WRAM state.

CPU result sequence for a scene whose independent expected mask is `F`:

| WRAM | Observation | Expected |
| --- | --- | --- |
| $1800 | V=30, after overflowing row 24 | F |
| $1801 | repeated read | F |
| $1802 | V=50, non-overflow line | F |
| $1803 | VBlank | F |
| $1804 | forced blank enabled | F |
| $1805 | next V=0 with forced blank active and candidates removed | F |
| $1806 | V=10, forced blank disabled | F |
| $1807 | following V=0 with rendering enabled | $00 |
| $1808 | following V=50, no eligible objects | $00 |

All four guests and all 15 focused tests pass. Build commands and generated
SHA-256 values are recorded in
[PROVENANCE](../tests/roms/PROVENANCE.md#original-obj-limit-and-stat77-guests-and-graphics-approved).
No commercial ROM, firmware, font or external graphics were acquired.

## Exact-baseline counterexamples

Created a detached worktree at the exact Issue baseline
`a7d944c662b1bbfca3ed9856240f7b9660c7bc56`, copied only the new integration test
and shared CPU builder, and left all baseline production unchanged.
Compilation succeeded. The final test suite returned exit 101: **15 behavioral
failures, zero passing test functions**. Each function contains multiple vectors;
the neither-flag guest and several exact-boundary controls run successfully
before the failing vectors. This is not a toolchain/build failure.

| Counterexample | Baseline actual | Independent expected |
| --- | --- | --- |
| 33 small eligible objects, STAT77 & $C0 | $00 | $40 |
| 35 tiles, earliest selected object's pixel (24,24) | blue `[0,0,255,255]` | black `[0,0,0,255]` |
| Exhaustion inside first 16-wide object, pixel (32,24) | green `[0,255,0,255]` | black |
| 32 fully offscreen small objects followed by sentinel, pixel (200,24) | black | green |
| Rectangle at X=-width, lower-half sentinel pixel (200,55) | black | green |
| Left-partial tile budget, pixel (1,24) | green | black |
| Both overflows with known PPU1 bit 4, full STAT77 | $11 | $D1 |
| CPU range guest, first WRAM result | $00 | $40 |
| Natural row-zero evaluation, V=1 HBlank mask | $00 | $40 |

Reproduction (expected exit 101 only on baseline):

```sh
git worktree add --detach .cache/issue-57/baseline a7d944c662b1bbfca3ed9856240f7b9660c7bc56
cp crates/snes-core/tests/ppu_obj_limits.rs .cache/issue-57/baseline/crates/snes-core/tests/
cp crates/snes-core/tests/common/obj_limits.rs .cache/issue-57/baseline/crates/snes-core/tests/common/
cargo test --manifest-path .cache/issue-57/baseline/Cargo.toml --locked -p snes-core --test ppu_obj_limits
```

## Mandatory validation (2026-10-24)

Used the CI Rust 1.99.0 toolchain explicitly, avoiding Homebrew Cargo precedence.
The initial `cargo +1.99.0 fmt` invocation failed before running a check because
that Cargo binary is not a rustup proxy. Selecting the toolchain's bin directory
resolved it. No production workaround or test expectation changed for tool setup.
The local Wasm linker uses the same ignored library-path wrapper documented in
Issue #55, now under `.cache/issue-57/wasm-ld`:

```sh
RUST99_BIN="$(dirname "$(rustup which --toolchain 1.99.0 cargo-clippy)")"
export PATH="$RUST99_BIN:$PATH"
export RUSTC="$RUST99_BIN/rustc"
python3 - <<'PY'
from pathlib import Path
import shlex, subprocess
compiler = Path(subprocess.check_output(
    ['rustup', 'which', '--toolchain', '1.99.0', 'rustc'], text=True).strip())
root = compiler.parent.parent
wrapper = Path('.cache/issue-57/wasm-ld')
wrapper.parent.mkdir(parents=True, exist_ok=True)
wrapper.write_text('#!/bin/sh\nexport DYLD_LIBRARY_PATH=' +
    shlex.quote(str(root / 'lib')) + '\nexec ' +
    shlex.quote(str(root / 'lib/rustlib/aarch64-apple-darwin/bin/rust-lld')) +
    ' "$@"\n')
wrapper.chmod(0o755)
PY
export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER="$PWD/.cache/issue-57/wasm-ld"
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo test --locked -p snes-core --test ppu_obj_limits
cargo test --locked -p snes-core --lib oam
cargo test --locked -p snes-core --test ppu_obj_sizes --test ppu_oam_access --test ppu_oam_reload --test ppu_hires --test ppu_pseudo_hires --test direct_color
python3 tools/make-fixture.py .cache/issue-57/smoke.bin
cmp tests/roms/smoke.bin .cache/issue-57/smoke.bin
python3 tools/build-test-tools.py --spc
python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"
python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"
python3 tools/build-cleared-games.py
python3 tools/build-cleared-games.py
npm --prefix web ci
npm --prefix web run build
npx --prefix web playwright install chromium
SNES_TEST_PORT=18085 npm --prefix web test -- --grep 'OBJ limits:'
SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'
```

Selected versions: Cargo `1.99.0 (5f94df478 2026-08-27)`, rustc
`1.99.0 (b940084d7 2026-09-28)`, Clippy `0.1.99 (b940084d7e 2026-09-28)`.
Formatting, warning-denying Clippy and **298 workspace tests passed**.
Focused checks passed **15 OBJ-limit tests**, **17 OAM unit tests**, and
**39 retained geometry/PPU/direct-color tests** (including all seven
`ppu_obj_sizes` tests and `ppu_oam_access`, `ppu_oam_reload`, `ppu_hires`,
`ppu_pseudo_hires`, `direct_color`). Smoke regeneration compared byte for byte,
SHA-256 `73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb`.
Pinned assembler builds passed; the SPC assembler and native release runner
reported the pre-existing macOS `rust-objcopy` missing-`libLLVM.dylib` strip
warning, but built and executed successfully. Hardware suites passed: full CPU
$0649 / PC $0081A2, basic CPU $0452 / PC $0081A2, SPC $0557 / PC $008060.
HDMA passed all eleven 16-row grayscale bands and the final shade through row 223.
Both independent cleared-game builds passed with unchanged hashes:
Castle `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`,
Memory `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
npm install/audit passed with zero vulnerabilities. The production release Wasm
build and Chromium setup passed; all **four focused actual Wasm/canvas checks
passed in 6.5 seconds**. All four original cartridges were independently
rebuilt and compared byte for byte with the browser-loaded artifacts.

The full required Chromium filter passed **27 tests in 2.7 minutes**, including
Castle/Memory five-minute replays, Castle traversal/credits, music/effects,
Web Audio queues/drift, the freeze negative control, persistence failure/recovery,
hires/SETINI transitions, Pages subpath loading, existing OBJ geometry checks and
all four new OBJ-limit Wasm/canvas tests. No mandatory check is skipped or remains
failing. The expected baseline failures and the resolved Cargo invocation issue
are separate from these final passing gates.

Sanitized command output remains under ignored `.cache/issue-57/`; source-built
hardware evidence is in `.cache/hardware-tests/results.json` and
`.cache/ppu-test/results.json`, and browser artifacts are in `web/test-results`.

## Remaining uncertainty and inventory

All 24 categories retain their complete scopes from Issue #55 and its linked
records. This adds bounded OBJ selection/admission/status evidence without
promoting `ppu-objects` to verified or narrowing its scope.

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
| `dsp-voices` | implemented_unverified | BRR decode, voice mixing, interpolation and key-on behavior; exhaustive BRR filters, interpolation phases and hidden behavior remain |
| `dsp-noise` | partial | Shared noise generator/rates and NON routing; DSP subcycle sampling and write races remain |
| `dsp-pitch` | partial | Pitch/PMON, interpolation advancement and noise interaction; subcycle PMON sampling and torn/live writes remain |
| `dsp-envelope` | partial | ADSR/GAIN shared rates, release/retrigger and boundaries; hidden/bent GAIN, exhaustive transitions and live interactions remain |
| `dsp-echo` | partial | FIR/echo and feedback; reference saturation and exhaustive echo behavior remain |
| `dsp-timing` | partial | Deterministic sample cadence and browser queues; DSP subcycles, exact hardware timing and audible fidelity remain |
| `pal` | unimplemented | PAL timing, video geometry and audio synchronization |

`ppu-objects` remains partial: physical evaluation/fetch timing and flag
assertion strobes, rotation oddities, OBJ interlace, hires interactions and
complete size/mode interactions remain outstanding. The line-based scheduler
convention is tested explicitly; it is not physical fetch-timing evidence.
All other complete scopes in the table remain outstanding as recorded.
Passing limited tests establishes neither full hardware accuracy nor commercial
compatibility and does not complete Issue #3. Castle/Memory source pins, hashes,
replays, durations, gameplay/audio assertions and negative controls are unchanged.
Optional external games and extra licensed fixtures were not run; no post-merge
verification is required by this Issue.
