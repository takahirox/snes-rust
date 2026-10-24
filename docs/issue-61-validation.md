# Issue #61: static Mode 7 EXTBG

## Specification contract (recorded before implementation and expectations)

Retrieved 2026-10-24 before coding:

- [Anomie's Register Doc](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt),
  revision 1157, 2007-07-12; SHA-256
  `19dfdf3f1c206655b8ede11bcd6dc053e36634f876819e40bb39cfb88a237dfd`.
  Sections $2105, $2133 and BACKGROUNDS / Mode 7 define SETINI.6 enabling
  BG2, sharing BG1's map, character byte and Mode 7 offsets rather than the
  planar BG2 offsets. BG1 uses eight color bits; BG2 uses seven with bit 7
  as priority. The front-to-back order is OBJ3, OBJ2, BG2-high, OBJ1, BG1,
  OBJ0, BG2-low, backdrop. BACKGROUNDS' transparency rule applies to the
  color index: BG2's masked zero is transparent even when priority is high.
  $2123–$212F and the Mode 7 paragraph retain independent layer screen/window
  controls. $2130–$2132 and Mode 7 retain normal layer-selected math and allow
  BG2 on sub with BG1 on main.
- Crucially, Anomie's Mode 7 section, immediately after describing EXTBG,
  says: “BG1, being a 256-color BG, can do Direct Color mode”. It then excludes
  seven-bit BG2. There is no EXTBG exception for BG1. Thus $2130.0 continues
  to select BG1 direct color when $2133.6 is set, independently of display enables.
  The old `direct_color_preserves_other_color_sources_and_priority` assertion
  requiring paletted BG1 was unsupported and must be corrected.
- [Fullsnes](https://problemkaputt.de/fullsnes.htm), no$sns v1.5 extraction;
  SHA-256 `3853b65a7331bf12eea431ff494a2948a1c78ef22f7e819db4b139fd5a79f6c6`.
  SNES PPU Rotation/Scaling / EXTBG corroborates the shared eight-bit source
  and seven-bit BG2/priority interpretation. SNES Picture Processing Unit (PPU) /
  Background Priority Chart corroborates the ordering. Color Palette Memory
  (CGRAM) and Direct Colors / Direct Color Mode explicitly restricts direct
  color to 256-color BGs (BG1 in Modes 3/4/7),
  with zero transparent and no map palette bits in Mode 7. Window specifies
  inclusive endpoints, per-layer selectors and TMW/TSW; Color-Math specifies
  independent TM/TS, CGADSUB.1 for BG2 and CGADSUB.0 for BG1.
  Fullsnes' SNES PPU BG Control / $2105 overview has an uncertain
  BG1/BG2-question-mark note about Mode 7 direct color; its detailed direct-color
  section and Anomie's explicit BG2 exclusion resolve that ambiguity in favor
  of BG1-only direct color.
  Fullsnes also discusses external-input garbage in other modes; this change
  makes no claim about that hardware behavior and preserves their renderer.

Derived expectations, distinct from source facts: raw $00 is transparent on
both layers; $01/$7F are BG2-low indices 1/127; $80 is BG1 index 128 and
transparent BG2; $81/$FF are BG1 indices 129/255 and BG2-high indices 1/127.
BG1 direct $80 gives BGR555 $4000 (RGBA 0,0,131,255); $81 gives $4004
(32,0,131,255); $FF gives $639C (230,230,197,255). CGRAM mutations must not
change those direct pixels, but must change BG2. Literal expectations below
are authored independently of production sampling, ranks or conversion helpers.

The bounded contract is static, non-mosaic, non-interlaced 256x224 composition.
It reuses the existing transform; identity interior displacement is the only
positioning evidence added here. No general transform or timing claim follows.

## Implementation and independent evidence

The production change is confined to `ppu.rs`. Mode 7 dispatch admits BG2 only
with SETINI.6; both layer requests use the same existing coordinate/map/byte
sampler. BG2 masks its index before transparency, uses palette colors and emits
layer 1 with separate low/high priority. BG1 retains the complete source byte
and direct-color selection independently of EXTBG. Existing composition then
applies each layer's TM/TS, windows and CGADSUB bit, including BG2 on sub.
No transform arithmetic, offset latch, OBJ evaluation or math algorithm changes.
The old direct-color assertion is corrected; all other direct-color coverage
is preserved. The historical Issue #11 record now identifies the correction.

Ten original public-API integration tests in `ppu_mode7_extbg` cover:

| Test | Bounded evidence |
| --- | --- |
| `every_raw_byte_interpretation_on_each_and_both_layers` | All six required bytes, BG1/BG2/both, rows 0/24/223, two distinct backdrops |
| `extbg_disabled_and_disabled_layers_cannot_leak_pixels_or_math` | SETINI-off and TM/TS-off controls; BG1 paletted behavior retained |
| `both_bg2_priorities_against_all_obj_priorities_with_each_screen_enable` | Low/high BG2 against all four OBJ priorities, BG1/BG2/both; direct BG1 distinguishes otherwise identical low pixels |
| `independent_main_windows_include_both_boundaries` | Independent selectors/masks, inclusive edges and adjacent outside pixels, empty window, high BG2 revealing BG1, transparent BG2 revealing masked BG1 |
| `main_layer_identity_selects_fixed_color_math` | CGADSUB.1 versus .0 on high BG2, low BG1 and $80 BG1; independently calculated fixed addition (3,2,1) |
| `independent_subscreen_routing_windows_and_bg2_math_operand` | TM/TS separation, BG1/BG2 sub operands, TMW versus TSW, both boundaries, disabled sub and transparent $80 |
| `bg1_direct_color_with_extbg_is_independent_of_cgram_and_bg2_display` | Raw zero and $01/$80/$81/$FF, BG2 enabled/disabled, CGRAM mutations, paletted BG1 controls and paletted BG2 |
| `shared_source_with_small_offset_ignores_bg2_planar_settings` | Interior identity H=3/V=2 samples columns 27..30 on row 26, independently changed BG2 H=90/V=74 and ignored size/map/character-base settings |
| `extbg_bit_preserves_other_bgmodes` | Unchanged framebuffer with SETINI.6 toggled in each Mode 0–6 (no external-input hardware claim) |
| `original_cpu_guest_completes_and_renders_stable_extbg_scene` | Source-built original guest, completion and literal pixels over three complete stable frames with instruction/frame bounds and STP/error rejection |

The positioning artwork exists only on tile row 2, so an independent BG2
vertical source cannot hide behind uniform rows. Its H/V alternatives also
differ modulo tile dimensions. This establishes a shared source in a small
identity interior scene, without extending the transform accuracy claim.

The original MIT 65C816 guest and its eight-byte artwork are built by
`common/mode7_extbg.rs` and `mode7-extbg-fixture`. Executed instructions initialize
all scene state under forced blank. The guest's completion marker records
initialization reaching its end; hosts independently check the output, never
inject success or patch PPU state. Native startup is bounded to eight frames /
300,000 instructions and rejects STP/APU errors. The partial setup frame is
discarded before three complete stable frames, each bounded by 400,000 clocks
and 100,000 instructions. The browser uses the ordinary picker, real release
Wasm and canvas ImageData, with five-second polling and a 30-second test deadline.
Both check eight literal columns on rows 24/80/160/223 and 256x224 geometry /
229376 RGBA bytes. Browser observations repeat with animation callbacks between
them and reject page/console errors. See the
[provenance record](../tests/roms/PROVENANCE.md#original-mode-7-extbg-composition-guest-approved)
for source licensing and the independently reproduced generated hash.

The guest scene's columns are independently specified:

| X | Source | Routing / priority | Literal RGBA |
| --- | --- | --- | --- |
| 0 | $00 | Both transparent; gray backdrop | 131,131,131,255 |
| 1 | $01 | BG1 windowed; BG2-low red palette | 255,0,0,255 |
| 2 | $7F | BG1 windowed; BG2-low green palette | 0,255,0,255 |
| 3 | $80 | BG2 transparent; BG1 direct blue | 0,0,131,255 |
| 4 | $81 | BG2-high covers BG1 direct | 255,0,0,255 |
| 5 | $FF | BG2-high green index 127 covers BG1 | 0,255,0,255 |
| 6 | $01 | BG2 windowed; BG1 direct red | 32,0,0,255 |
| 7 | $FF | BG2 windowed; BG1 direct full source | 230,230,197,255 |

## Exact predecessor counterexamples

An isolated detached checkout contains the exact predecessor production code,
with only the new integration test and common guest source copied in:

```sh
git worktree add --detach .cache/issue-61/baseline 3ef31ed5a7ae5e4bd04591a1b1b7cb7822c952f0
cp crates/snes-core/tests/ppu_mode7_extbg.rs .cache/issue-61/baseline/crates/snes-core/tests/
cp crates/snes-core/tests/common/mode7_extbg.rs .cache/issue-61/baseline/crates/snes-core/tests/common/
cargo test --manifest-path .cache/issue-61/baseline/Cargo.toml --locked -p snes-core --test ppu_mode7_extbg
```

Compilation succeeds against baseline public APIs. Eight tests fail behaviorally;
the two passing controls are EXTBG/layer disabled and other BGMODE preservation.
Representative literal counterexamples:

| Baseline assertion | Actual | Expected |
| --- | --- | --- |
| Raw $01, BG2-only, (24,0) | Gray 131,131,131,255 | Red 255,0,0,255 |
| BG1 direct $01, EXTBG on, (24,24) | Paletted red 255,0,0,255 | Direct red 32,0,0,255 |
| BG2 subscreen added to backdrop, (20,24) | 16,16,0,255 | 49,16,0,255 |
| BG2-selected fixed math, raw $81 | BG1 yellow 255,255,0,255 | 57,16,8,255 |
| Shared offset BG2, (25,24) | Gray 131,131,131,255 | Red 255,0,0,255 |
| CPU guest (1,24), after completion | Gray 131,131,131,255 | Red 255,0,0,255 |

The direct-color failure is the source-confirmed behavioral correction, not a
compilation failure or an emulator-to-emulator oracle. All ten tests pass on the
implementation. Intentional predecessor failures are separate from final gates.

## Mandatory validation (2026-10-24)

Used CI's Rust 1.99.0 toolchain explicitly (Cargo 1.99.0, rustc 1.99.0,
Clippy 0.1.99), Node 24.12.0 and Python 3.13.13. As in Issue #59, selected
the installed toolchain's bin directory in PATH and RUSTC, avoiding Homebrew
compiler precedence. The ignored `.cache/issue-61/wasm-ld` wrapper supplies
that toolchain's library directory through DYLD_LIBRARY_PATH to rust-lld;
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER selects it. This is the same local
macOS linker setup as earlier validations, with no production workaround.

Executed commands (sanitized logs under ignored `.cache/issue-61/`):

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo test --locked -p snes-core --test ppu_mode7_extbg --test ppu_mode7_scroll --test direct_color --test ppu_obj_limits --test ppu_pseudo_hires
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
SNES_TEST_PORT=18085 npm --prefix web test -- --grep 'Mode 7 EXTBG:'
SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'
cargo run --locked -p snes-core --example mode7-extbg-fixture -- .cache/issue-61/rebuilt
cmp .cache/issue-61/rebuilt/mode7-extbg.sfc .cache/mode7-extbg/mode7-extbg.sfc
shasum -a 256 .cache/issue-61/rebuilt/mode7-extbg.sfc
```

Formatting and warning-denying Clippy pass. **320 workspace tests pass**,
including the ten new tests. The five requested focused suites pass **54 tests**:
EXTBG 10, independent Mode 7 offsets/shared latch 10, direct color 7,
OBJ limits 15 and pseudo-hires 12. The exact-baseline suite was rerun after
strengthening the positioning/direct-zero vectors: compilation still succeeds,
with the same eight behavioral failures and two passing controls.

Smoke regeneration is byte-identical, SHA-256
`73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb`.
All hash-pinned assembler builds pass. The existing macOS rust-objcopy missing
libLLVM strip warning appears in auxiliary SPC/native release builds; those
builds and executions succeed. The CPU/SPC gates pass full CPU $0649 / PC
$0081A2, basic CPU $0452 / PC $0081A2 and SPC $0557 / PC $008060.
The HDMA PPU gate passes all eleven 16-row grayscale bands and final shade
through row 223. Evidence remains at `.cache/hardware-tests/results.json` and
`.cache/ppu-test/results.json`.

Both fresh cleared-game builds reproduce unchanged hashes:

- Castle `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`.
- Memory `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.

npm installation/audit passes with zero vulnerabilities; the production release
Wasm build and Chromium installation pass. The new focused browser test passes
in 3.9 seconds including setup, observing distinguishing actual Wasm/canvas
pixels with no errors. Independently regenerated guest bytes compare exactly
with the browser's image, SHA-256
`12e6ff36091c8cc72521d669d5cb69f66a1ac4dbea20d89e4f3663467c57e705`.


The full mandatory Chromium filter passes **29 tests in 2.8 minutes**, with no
skipped tests.
It includes both games' five-minute replays, Castle's complete map/credits route,
Memory's music/effects and gesture-activated Web Audio, queue/drift/stall checks,
the freeze negative control, persistence failure/recovery, hires/SETINI,
OBJ size/limits, Pages subpath loading, independent Mode 7 scroll/shared-latch
and the new EXTBG Wasm/canvas check. Optional external-game cases are excluded
by the required filter and were neither acquired nor run. No mandatory check
remains failed or unperformed. Browser artifacts are under `web/test-results`.

Staged whitespace checks, JavaScript syntax, changed Markdown local destinations,
new specification/provenance anchors and balanced fences pass. All 24 inherited
inventory rows/scopes/statuses compare verbatim with Issue #59. Only source and
documentation are committed; generated ROMs and local tool/log files stay ignored.

## Remaining uncertainty and inventory

All 24 categories retain their complete supplied scopes below (copied verbatim
from Issue #59): 21 partial, `dsp-voices` implemented_unverified, and
`ppu-raster`/`pal` unimplemented. No category is promoted or verified. The
inventory's broad outstanding EXTBG scope now has the bounded static subset
implemented above; complete EXTBG interactions remain unverified.

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

Mode 7 remains partial despite the static EXTBG evidence. Remaining specific
limitations are mosaic, nonidentity arithmetic/truncation, signed-coordinate /
screen-over edges, interlace/overscan, pseudo-hires composition, multiplication
timing/interference, physical reset retention and pixel-fetch/raster timing.
The existing row-origin convention is retained; these tests do not independently
establish hardware row origin. BG1 direct color with EXTBG is resolved here
by the cited specifications and executable discriminating vectors. General
color-window logic, clipping/prevention, saturation and halving accuracy remain
outside this bounded math evidence. All other complete inventory scopes above
remain in force.

Existing Castle/Memory source pins, licenses, generated hashes, replay inputs,
durations, gameplay/audio assertions and negative controls are preserved.
No commercial ROM or firmware was acquired. Limited passing checks establish
neither full hardware accuracy nor commercial compatibility and do not complete
Issue #3. No post-merge verification is required.
