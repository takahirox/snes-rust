# Issue #53: math-disabled SETINI pseudo-hires

## Specification contract (recorded before pixel expectations)

Retrieved both source files on 2026-10-24 and checked their SHA-256 against
the [Issue #49 record](issue-49-validation.md#specification-contract-recorded-before-implementation):

- [Anomie's Register Doc](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt),
  revision 1157, 2007-07-12, SHA-256
  `19dfdf3f1c206655b8ede11bcd6dc053e36634f876819e40bb39cfb88a237dfd`.
  Sections $2100, $210D–14, $212C–2F, $2133, BG Scrolling, Modes 5/6,
  Color Math and Rendering the Screen. SETINI bit 3 assigns even samples to
  sub and odd samples to main. Modes 5/6 force that output while fetching
  distinct character phases; BG scroll registers still count full dots.
- [Fullsnes](https://problemkaputt.de/fullsnes.htm), no$sns v1.5 extraction,
  SHA-256 `3853b65a7331bf12eea431ff494a2948a1c78ef22f7e819db4b139fd5a79f6c6`.
  Sections SETINI, CGRAM, TM/TS, TMW/TSW, Color Math and INIDISP. SETINI
  shifts the subscreen half a dot left. TM/TS independently enable BG/OBJ;
  TMW/TSW independently suppress layers inside their windows. CGRAM index
  zero is the main backdrop; COLDATA is the sub backdrop when no opaque
  layer is selected. A zero BG pixel color index is transparent, while a nonzero
  index whose palette entry is black is opaque.

Derived in-scope rule: for low-resolution BGMODE, dot x supplies both output
samples 2*x (sub) and 2*x+1 (main), with separate composition. SETINI does
not change character width, scroll units, map addressing, offset-table columns
or Mode 7 transforms. Windows and OBJ positions count full dots. BGMODE 5/6
OR SETINI bit 3 gives 512 samples, never 1024. No other SETINI bit enables
horizontal expansion. Identity Mode 7 retains the existing row-origin convention.

The tested domain is non-interlaced, non-mosaic, CGADSUB=$00, CGWSEL=$00
(also $02 as a selection control), with EXTBG disabled. No in-scope predecessor
rule was disproved. Fullsnes and Anomie still disagree about hires math;
Anomie additionally describes previous-main-dependent sub math and clipping,
including an unknown left-edge case. The existing renderer's math gate and
clipping are not evidence resolving that dispute. Math-enabled pseudo-hires
and color-window clipping/prevention remain unverified.

INIDISP brightness affects both displayed phases and forced blank makes the
current packed row black. The existing RGBA convention scales each five-bit
component by 255*brightness/(31*15), truncating; this is software representation
policy, not a measured analog DAC curve (Fullsnes describes a different
intermediate brightness curve). Likewise, frame promotion duplicates compact
rows, retains width through the frame and permits shrinking on low-resolution
row zero. This storage policy does not establish physical switching timing.

## Implementation and coverage

Production changes are confined to `ppu.rs`: `bg_hires` selects Mode 5/6
character width, scroll scaling and distinct BG phases; `wide_output` selects
512-sample storage and displayed sub/main phases for either Mode 5/6 or SETINI
bit 3. Existing offset, Mode 7, priority, layer-window and brightness code
receives the same full-dot coordinates as before. The math/clipping algorithm
is unchanged and is not validated by this change. In the tested domain,
CGWSEL bit 1 selects only the unused math operand, following Anomie's $2130
register description; it does not override TM/TS selection of visible phases.

`crates/snes-core/tests/ppu_pseudo_hires.rs` adds twelve original tests:

| Test | Independent evidence |
| --- | --- |
| `pseudo_width_and_literal_phases` | Exact 512x224/458752 RGBA geometry; adjacent blue/red, white/green, transparent magenta/cyan and opaque black/black pairs; both edges |
| `isolated_low_resolution_control` | SETINI clear gives 256x224/229376 bytes and literal main-only colors |
| `setini_preserves_ordinary_hires_character_phases` | Both Modes 5/6, bit 3 clear/set/clear, distinct phases and both character halves, cell boundaries and right edge |
| `independent_enables_backdrops_transparency_and_opaque_black` | All four TM/TS combinations, distinct CGRAM[0]/COLDATA backdrops, zero versus opaque black; CGWSEL=$00/$02 |
| `layer_windows_use_full_dots_and_independent_masks` | All four TMW/TSW combinations at full-dot window boundaries 6/7/8/9, both phases and image edges |
| `bg_and_obj_priority_and_screen_routing` | BG1 low versus BG2 low/high; high/low-priority OBJ at dot 5, independent main/sub routing and adjacent no-OBJ control |
| `planar_scroll_tile_page_boundaries_and_both_edges` | Modes 0/1/3/4, 8/16-dot cells, ten scroll displacements, adjacent dots/tile edges and both image edges, including final row |
| `modes_2_4_offsets_keep_full_dot_boundaries` | Fine H=3, first eligible dot 5 and next column at 13, H/V/Mode 4 axis selector, disabled later descriptors and right edge |
| `identity_mode7_keeps_full_dot_transform_coordinates` | Nonuniform identity scene, adjacent phases, transparency, opaque black, both edges and final row |
| `brightness_forced_blank_and_packed_frame_transitions` | Brightness 0/7/15 on both phases; each other SETINI bit and $F7 do not expand; mixed 256→512→256 rows, promotion preserves prior dots, low rows duplicate, blank respects packed stride |
| `cpu_static_guests_and_controller_vblank_transitions_reset_replacement` | Three original CPU-built cartridges, completion/SETINI markers, BGMODE fixed at 1, static on/off, controller-driven enable/disable, reset and replacement |
| `runner_ppm_geometry_payload_edges_and_last_row` | Actual runner with both static cartridges: P6 width/height, exact payload and literal phase/edge colors on first and final rows |

Numerical expectations are literal RGBA vectors or crops from an independently
constructed 32x32 linear coordinate image. No expected pixel calls production
addressing, composition or conversion helpers. Encoding that image into planar
VRAM is fixture setup, separate from the oracle. Brightness 7 is explicitly a
representation check (119 for a full-intensity component), not a DAC claim.
Other SETINI bits are checked only for absence of horizontal expansion; their
vertical, EXTBG or external-sync functionality is not asserted.

`common/pseudo_hires.rs` contains original MIT encoded 65C816 instructions,
characters, palette and map data. Every register/VRAM/CGRAM initialization is
executed through CPU bus writes under forced blank. Initialization publishes
`$30=$53`; `$31` records SETINI. Static variants keep bit 3 off/on. The
interactive variant polls a new VBlank and auto-joypad completion, then selects
$08 while A is held, otherwise $00. BGMODE is written once, as Mode 1. Five
startup frames and three per switch are bounded to 300,000 instructions per
batch. The host supplies only the cartridge and controller input. No machine
state is patched. See the [provenance decision](../tests/roms/PROVENANCE.md#original-setini-pseudo-hires-guests-and-graphics-approved).

`web/tests/pseudo-hires.spec.js` builds the same guests and checks actual Wasm
buffers against canvas readback, literal adjacent phase colors, both edges,
final row, width/height and exact length. It runs 256→512→256 transitions,
explicit memory growth, scale/shape controls, reset and static cartridge
replacement. Pointer and memory views are reacquired for each observation.
The existing ordinary-hires browser checks are unchanged.

Generated fixture identities (32 KiB each; no ROM binaries committed):

| Fixture | SHA-256 |
| --- | --- |
| `pseudo-hires-off.sfc` | `ac84aa56d96ac07c4403da2485977031ee59ea5391e6f181a77fa4fc05e67aaa` |
| `pseudo-hires-on.sfc` | `67ca680364c0af4dc298bf629a9a8119e2b04b88ddb498969aa4e7c4e0d3b88c` |
| `pseudo-hires-switch.sfc` | `36d749a243bbd24be199d6cb9272f3665b4ffae757b8ecb8ee7f6c43f1bd1a10` |

## Baseline counterexamples

Created an isolated detached worktree at the exact Issue baseline
`06e295b62340b3052002de517a77de588177ed35`. Copied only the new test file and
its shared guest builder. Baseline production files and expected pixels were
unchanged. The final comparison returned exit 101: **10 failed, 2 passed**.
The isolated low-resolution control and ordinary-hires character-phase control
passed. Representative baseline failures:

| Assertion | Observed baseline | Required |
| --- | --- | --- |
| Pseudo-hires geometry | 256x224, 229376 RGBA bytes | 512x224, 458752 bytes |
| Window routing control, sample 0 | red `[255,0,0,255]` | sub blue `[0,0,255,255]` |
| Both screens disabled, sample 0 | main cyan `[0,255,255,255]` | sub magenta `[255,0,255,255]` |
| Reduced brightness, sample 0 | `[119,0,0,255]` | `[0,0,119,255]` |
| Original enabled CPU guest | 256x224, 229376 bytes | 512x224, 458752 bytes |
| Runner enabled fixture | P6 `256 224` | P6 `512 224` |

Reproduce (the final command is expected to fail on baseline):

```sh
git worktree add --detach .cache/issue-53/baseline 06e295b62340b3052002de517a77de588177ed35
cp crates/snes-core/tests/ppu_pseudo_hires.rs .cache/issue-53/baseline/crates/snes-core/tests/
cp crates/snes-core/tests/common/pseudo_hires.rs .cache/issue-53/baseline/crates/snes-core/tests/common/
cargo test --manifest-path .cache/issue-53/baseline/Cargo.toml --locked -p snes-core --test ppu_pseudo_hires
```

The first new-test run passed all eleven tests then present. Adding the explicit
ordinary-hires SETINI phase control produced twelve passing focused tests and
the two passing baseline controls above. No expectations were adjusted to fit
either implementation, and no existing test file was modified.

## Mandatory validation (2026-10-24)

Executed in this macOS worktree with Homebrew Rust 1.94.0. Wasm uses the rustup
compiler owning its installed target. The auxiliary SPC assembler uses
installed Rust 1.99.0 with the script's existing bootstrap flag. An additional
warning-denying Clippy run with CI's Rust 1.99.0 also passed.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
rustup run 1.99.0 cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo test --locked -p snes-core --test ppu_pseudo_hires --test ppu_hires --test ppu_offset --test ppu_mode6_offset --test direct_color
python3 tools/make-fixture.py .cache/smoke.bin
cmp tests/roms/smoke.bin .cache/smoke.bin
RUSTUP_TOOLCHAIN=1.99.0 RUSTC="$(rustup which --toolchain 1.99.0 rustc)" python3 tools/build-test-tools.py --spc
python3 tools/hardware-tests.py \
  --ca65 "$PWD/.cache/tools/cc65/bin/ca65" \
  --ld65 "$PWD/.cache/tools/cc65/bin/ld65" \
  --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"
python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"
python3 tools/build-cleared-games.py
python3 tools/build-cleared-games.py
npm --prefix web ci
RUSTC="$(rustup which rustc)" npm --prefix web run build
(cd web && npx playwright install chromium)
SNES_TEST_PORT=18085 npm --prefix web test -- --grep 'SETINI:'
SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'
```

- Formatting and warning-denying Clippy passed. Workspace: **276 tests passed**.
  Focused targets: **12 pseudo-hires, 11 ordinary hires, 12 Mode 2/4 offset,
  10 Mode 6 offset and 7 direct-color tests passed**.
- Smoke regeneration matched byte-for-byte, SHA-256
  `73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb`.
- Pinned tool builds passed. The auxiliary SPC assembler retained its existing
  `rust-objcopy`/`libLLVM.dylib` warning; build and execution succeeded. Workspace
  Clippy remained warning-free. Hardware suites passed: full CPU last test
  $0649/PC $0081A2, basic CPU $0452/PC $0081A2, SPC $0557/PC $008060.
  HDMA passed eleven 16-row grayscale bands and the final shade through row 223.
- Both cleared-game builds passed with unchanged Castle SHA-256
  `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`
  and Memory `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
- npm installation/audit (zero vulnerabilities), release Wasm build and
  Chromium installation passed. The focused actual Wasm/canvas test passed.
  The full required browser filter passed **21 tests in 2.7 minutes**, including
  the new SETINI guest, both ordinary-hires canvas checks, Castle/Memory
  five-minute emulated replays, Castle route/credits, music/effects, Web Audio
  drift/queue checks, the freeze negative control, persistence failure/recovery
  and Pages subpath loading. No mandatory check failed or was skipped.

Raw logs remain under `.cache/issue-53/`; hardware evidence is in
`.cache/hardware-tests/results.json` and `.cache/ppu-test/results.json`, and
browser artifacts are under `web/test-results`. The expected baseline failure
is recorded separately above. No unexpected implementation-test failures occurred.

## Remaining uncertainty and inventory

All 24 categories keep their complete prior scopes and statuses from the
[Issue #51 inventory](issue-51-validation.md#remaining-scope-and-inventory),
[Issue #49 record](issue-49-validation.md#remaining-uncertainty-and-inventory),
[earlier inventory](issue-27-validation.md#remaining-uncertainty-and-inventory)
and [README](../README.md#implemented-features-and-limits). This change adds
bounded pseudo-hires evidence within `ppu-hires`; it does not narrow another
category to the subset exercised here. In particular:

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

No category becomes verified or complete. The Issue #3 handoff remains intact;
these checks do not complete Issue #3 or establish commercial compatibility.
No game hash, pin, replay, duration, audio/gameplay assertion, negative control,
license notice or fixture alteration was changed. No commercial ROM or firmware
was acquired. Optional unaudited games are excluded by the required browser
filter. Physical hardware, optional additional scenes and post-merge checks
were neither required nor performed.
