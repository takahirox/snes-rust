# Issue #59: independent Mode 7 offsets and shared write latch

## Specification contract (recorded before test expectations)

Retrieved 2026-10-24, matching the versions recorded for Issue #53:

- [Anomie's Register Doc](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt),
  revision 1157, 2007-07-12, SHA-256
  `19dfdf3f1c206655b8ede11bcd6dc053e36634f876819e40bb39cfb88a237dfd`.
  Sections $210D–$2114 distinguish two offset registers at each BG1 port:
  planar offsets are 10 bits; Mode 7 offsets are signed 13 bits. Each BG1
  port updates both registers. Planar H merges the previous byte's upper five
  bits with the old register's high-byte fine bits; V takes the previous byte.
  All planar offsets share that previous byte. Sections $211B–$2120 define
  a separate previous byte shared by the matrix, centers and Mode 7 offsets:
  each write commits the new high byte and previous low byte, then replaces
  the previous byte. Matrix values are signed 8.8; centers are signed 13 bits.
  Sections $211B/$211C and $2134–$2136 describe the signed 16-by-8 product
  of A and B's latest high byte, returned as 24-bit two's complement.
- [Fullsnes](https://problemkaputt.de/fullsnes.htm), no$sns v1.5 extraction,
  SHA-256 `3853b65a7331bf12eea431ff494a2948a1c78ef22f7e819db4b139fd5a79f6c6`.
  BG Scroll, Rotation/Scaling (M7HOVS/M7VOFS Port Notes and M7xx Write-twice
  mechanism), and Maths Multiply/Divide explicitly corroborate dual writes,
  separate BG_old/M7_old and sharing across $210D/$210E/$211B–$2120.
  The rotation offset/center fields are signed 13-bit integer pixel units.

Derived rules: these writes apply regardless of BGMODE, commit on every byte
without a phase toggle, and only the addressed register changes. $210F–$2114
cannot disturb M7_old; matrix/center writes cannot disturb BG_old. Preserve
signed 16-bit matrices and sign-extend only the lower 13 offset bits at use.
No source-disproved requirement or affected old assertion was found.
Multiplication observations use forced blank and settling instructions; neither
latency nor the documented Mode 7 active-rendering multiply interference is
claimed. Fullsnes describes that interference more explicitly than Anomie.

Pixel oracles use identity A=D=$0100, B=C=0, zero centers and no flips/mosaic/
interlace/EXTBG. In this bounded domain, input displacement selects the same
source displacement. Horizontal scenes are vertically uniform; vertical tests
use the existing renderer's row-origin convention and do not resolve hardware
row origin. Sparse independently drawn source stripes give different colors
at columns 21 and 69 and around fine/tile boundaries. Signed-negative checks
stay inside the map except an explicit left/top transparency probe; they do
not establish general clipping/truncation or screen-over accuracy.

## Implementation and original coverage

Production changes are local to `ppu.rs`: two independent raw offset words,
BG1 port capture through the existing `m7_latch`, and signed 13-bit consumption
from those words. Ordinary planar scroll code/fine-bit merge and the signed
16-bit matrix interpretation are retained. Default initialization is software
policy, not evidence about physical reset retention.

Two private-state unit tests cover every BGMODE, immediate single-byte commits,
repeated writes to the same port, H/V interleaving, all matrix/center registers,
unaddressed-register retention, mode-switch retention, planar H fine-bit merge,
planar V capture and separation of both latches across all relevant ports.
They expose no public inspection API.

`ppu_mode7_scroll` adds ten integration tests using only baseline public APIs:

| Test | Evidence |
| --- | --- |
| `horizontal_offset_supplies_next_matrix_low_byte_immediately` | Each BGMODE, $210D=$34 then A=$12 gives literal multiply bytes $34/$12/$00 |
| `vertical_offset_supplies_next_matrix_low_byte_immediately` | Same independently for $210E |
| `matrix_and_centers_supply_next_matrix_low_byte_immediately` | Single C/D/X/Y writes feed A across registers, every BGMODE |
| `hv_interleaving_supplies_latest_byte_to_matrix` | H=$34, V=$56, A=$12 gives $56/$12/$00 |
| `signed_16bit_matrix_multiplication_control` | A=$9234, B high=$01 gives $34/$92/$FF; A=-204 times -2 gives $98/$01/$00 |
| `all_planar_only_ports_preserve_mode7_previous_byte` | All six $210F–$2114 ports leave M7_old=$34 intact |
| `independent_horizontal_capture_survives_mode_switches` | Mode 1 capture gives H=21 rather than planar H=69; switches 7→0→3→7 retain it; literal colors at nine positions on rows 0/24/223 |
| `hv_interleaving_changes_vertical_capture_without_planar_interference` | H supplies V's low byte across an intervening planar V write; V=21 rather than planar V=66; three horizontal probes on nine rows |
| `positive_negative_offsets_fine_tile_boundaries_and_ignored_upper_bits` | Both axes: +1/+7/+8/-1/-8, four upper-bit encodings, literal fine/tile transitions and bounded negative-origin transparency |
| `cpu_guest_results_completion_and_distinguishing_scroll_scene` | Original CPU initializes everything through bus writes, compares/saves 42 readbacks, publishes sticky completion/pass, then displays distinguishing literal pixels |

The original input artwork marks columns 0/1/2 red/green/blue, 6/7/8
white/yellow/cyan, 14/15/16 magenta/red/green, 20/21/22/23/24
blue/red/blue/white/yellow, 31/32/33/34 cyan/magenta/red/green,
68/69/70/71/72 blue/green/yellow/cyan/magenta and 127 white.
Other columns are opaque black. Sixteen tiles repeat a 128-pixel strip; the
vertical test transposes it. Expectations are separately authored literal
RGBA vectors, never calls to production register/coordinate/palette helpers.
For H=21 the nine source columns are 21/22/23/24/69/70/71/72/276 (last
wraps to strip column 20); the wrong H=69 starts green. For -1, screen
positions 1/2/3 sample source 0/1/2; -8 starts at screen position 8.
The transparent-over setting makes signed-negative and ignored-upper-bit
controls discriminate a mistaken unsigned or 16-bit interpretation, rather
than letting map wrapping hide it. This is bounded identity evidence, not a
claim of general screen-over, transform clipping or truncation accuracy.

`common/mode7_scroll.rs` builds an original MIT 32 KiB LoROM. Its 65C816
program initializes forced blank, Mode 1, VRAM map/characters, CGRAM, matrix,
centers and offsets. Fourteen readback cases save actual bytes at $1800–$1829;
the guest independently compares literals with sticky pass at $31 and
completion $30=$59. Reads follow sixteen NOPs under forced blank. The final
scene switches Mode 1→7 after the interleaved H/BG2H/H writes. The native
host has an eight-frame/300,000-instruction startup deadline, rejects STP and
observes three more complete frames. The browser loads exactly the same
source-built image through the ordinary picker, observes all results/markers,
256x224/229376-byte geometry and nine literal pixels on rows 0/24/223 in both
actual Wasm framebuffer bytes and canvas ImageData. It has a 30-second test
and five-second polling deadline. Neither host patches state or injects pass
markers. See [provenance](../tests/roms/PROVENANCE.md#original-mode-7-scroll-and-shared-latch-guest-approved)
for licensing, build command and generated SHA-256; no ROM or firmware is committed.

## Exact-baseline counterexamples

Created a detached worktree at the exact Issue baseline
`1bc328f86a3c2e99c79b204f9b18b93f21790153`. Copied only the new integration test
and its common guest builder, leaving baseline production code unchanged:

```sh
git worktree add --detach .cache/issue-59/baseline 1bc328f86a3c2e99c79b204f9b18b93f21790153
cp crates/snes-core/tests/ppu_mode7_scroll.rs .cache/issue-59/baseline/crates/snes-core/tests/
cp crates/snes-core/tests/common/mode7_scroll.rs .cache/issue-59/baseline/crates/snes-core/tests/common/
(cd .cache/issue-59/baseline && cargo test --locked -p snes-core --test ppu_mode7_scroll)
```

Compilation succeeded against its existing public APIs. **Six tests failed
behaviorally**, distinct from the four passing controls:

| Failing baseline assertion | Actual | Expected |
| --- | --- | --- |
| Horizontal offset → A readback (Mode 0) | $01/$12/$00 | $34/$12/$00 |
| Vertical offset → A readback (Mode 0) | $01/$12/$00 | $34/$12/$00 |
| H/V interleave → A | $01/$12/$00 | $56/$12/$00 |
| Independent H, pixel (0,0) | green RGBA [0,255,0,255], source column 69 | red [255,0,0,255], source column 21 |
| Independent V, pixel (0,0) | black [0,0,0,255], source row 66 | red [255,0,0,255], source row 21 |
| CPU guest first actual result byte | $01 | $34 |

Passing controls: all planar-only ports preserving M7_old, matrix/center
cross-register previous bytes, signed 16-bit multiplication, and paired-write
positive/negative/fine-boundary/ignored-upper-bit scenes. The last control is
expected to pass because ordinary paired offset writes usually reconcile the
two baseline states. Private-state unit tests were not copied to baseline;
the counterexamples do not depend on a compile error or a new debug API.

## Mandatory validation

Used the README-supported environment: Node 24.12.0, Python 3.13.13,
Cargo 1.99.0 (5f94df478), rustc 1.99.0 (b940084d7) and Clippy 0.1.99.
Selected the installed 1.99.0 toolchain bin directory in PATH and RUSTC,
as in [Issue #57](issue-57-validation.md#mandatory-validation-2026-10-24),
to avoid Homebrew compiler precedence. The same documented local library-path
wrapper supplies the macOS Wasm linker at ignored `.cache/issue-59/wasm-ld`;
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER points there. No toolchain workaround
changes production files or test expectations. The initial native focused run
and exact-baseline reproduction also passed compilation with Homebrew 1.94.0.

Executed commands (logs redirected under ignored `.cache/issue-59/`):

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo test --locked -p snes-core --test ppu_mode7_scroll --test direct_color --test ppu_pseudo_hires --test ppu_offset --test ppu_mode6_offset --test ppu_hires --test ppu_obj_sizes --test ppu_obj_limits
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
SNES_TEST_PORT=18085 npm --prefix web test -- --grep 'Mode 7 scroll:'
SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'
```

Formatting and warning-denying Clippy passed. **310 workspace tests passed**,
including both private Mode 7 unit tests and all ten new integration tests.
The requested focused command passed **84 tests across eight suites**.
Smoke regeneration compared byte for byte with SHA-256
`73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb`.
All pinned assembler builds passed. The auxiliary SPC assembler and native
release builds emitted the pre-existing macOS rust-objcopy missing-libLLVM
strip warning; their builds and executions still succeeded. Hardware gates
passed full CPU $0649/PC $0081A2, basic CPU $0452/PC $0081A2, SPC
$0557/PC $008060, and all eleven 16-row HDMA grayscale bands plus the
last shade through row 223. Both fresh cleared-game builds passed with
unchanged expected SHA-256 values:

- Castle: `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`.
- Memory: `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.

npm installation/audit passed with zero vulnerabilities. The production release
Wasm build and Chromium setup passed. The focused actual Wasm/canvas test
passed in 4.7 seconds, observing the original source-built cartridge's literal
readbacks and distinguishing pixels. The cartridge was independently rebuilt
into `.cache/issue-59/rebuilt/` and compared byte for byte with the browser's
image, retaining SHA-256
`31f5383fb1b81201dab672e769a52f5450117d5c84cfc4e65755e3ce57c67610`.

The full required Chromium filter passed **28 tests in 2.7 minutes**. It
includes both cleared games' five-minute replays, Castle's all-map traversal
and credits, Memory's music/effects and gesture-activated Web Audio checks,
queue/drift/stall coverage, the freeze negative control, persistence failure/
recovery, hires and SETINI transitions, OBJ sizes/limits, Pages subpath loading
and the new Mode 7 readback/framebuffer/canvas test. No mandatory check is
skipped, unperformed or remains failing. Optional external games are excluded
by the specified filter; they were not acquired or run. The six intentional
baseline failures are separate from these final passing gates.

Staged whitespace, new documentation destinations/anchors, balanced Markdown
fences and JavaScript syntax checks passed. The inventory's 24 original rows
and complete scopes were compared verbatim with Issue #57. Sanitized command
logs remain under ignored `.cache/issue-59/`; source-built hardware evidence
is in `.cache/hardware-tests/results.json` and `.cache/ppu-test/results.json`,
and browser evidence is in `web/test-results`. Only source and documentation
are committed.

## Remaining uncertainty and inventory

All 24 categories retain the complete supplied Issue #57 scopes below:
21 partial, `dsp-voices` implemented_unverified, and `ppu-raster`/`pal`
unimplemented. None is verified or promoted by this change.

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

Mode 7 remains partial. In addition to the full retained scope above, EXTBG,
nonidentity arithmetic/truncation, signed-coordinate/screen-over edges, mosaic,
and fetch/raster interactions remain outstanding. Multiplication latency,
active-rendering multiply interference, physical reset retention, interlace,
EXTBG priorities/composition and BG1 direct color with EXTBG are not resolved.
The Issue #11 direct-color assertion does not settle that EXTBG question.
The row-origin convention is retained, not hardware-verified by these tests.
All other complete supplied category scopes remain as recorded above.
Passing these bounded checks establishes neither full hardware accuracy nor
commercial compatibility and does not complete Issue #3. Existing hardware
fixtures and cleared Castle/Memory source pins, hashes, replay inputs, durations,
gameplay/audio assertions and negative controls are preserved. No commercial
ROM acquisition is required. No post-merge verification is required.
