# Issue #51: non-mosaic Mode 6 offsets

## Specification contract (recorded before implementation)

Retrieved 2026-10-24, matching the source bytes recorded for Issue #49:

- [Anomie's Register Doc](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt),
  revision 1157, 2007-07-12, SHA-256
  `19dfdf3f1c206655b8ede11bcd6dc053e36634f876819e40bb39cfb88a237dfd`.
  Sections $2105, $210D–14, Tile Maps and Character Maps, BG Scrolling,
  Modes 2, 5 and 6. Mode 6 explicitly fixes BG3 as well as BG1 to eight
  full-dot columns (16 half-dot samples), and explicitly excludes extending
  offsets to sixteen full dots. Mode 2 describes the initial partial column,
  retained fine scroll, separate H/V lookups, and scanline-independent BG3 row.
- [Fullsnes](https://problemkaputt.de/fullsnes.htm), no$sns v1.5 extraction,
  SHA-256 `3853b65a7331bf12eea431ff494a2948a1c78ef22f7e819db4b139fd5a79f6c6`.
  BGMODE, BG3SC, BG3HOFS/BG3VOFS and BG Map establish register units, page
  layout and raw descriptor masks. Its offset algorithm is under construction;
  it supplies no competing coordinate algorithm. The tentative fixed-height
  Mode 6 annotation is resolved as in Issue #49: height remains 8 or 16.

### Reconciliation and equations

These equations are a derivation from the prose and register units, not a claim
that Anomie's explicitly tentative Mode 2 pseudocode is hardware measurement.
Number the visible eight-full-dot columns after applying BG1 fine scroll. The
first partial column is unaffected; column k>=1 uses BG3 column k-1. This gives
the following in full dots, before conversion to output samples:

```text
j = output sample 0..511; d=floor(j/2); phase=j mod 2
H=BG1HOFS & 1023; V=BG1VOFS & 1023; f=H mod 8
q=d+f; k=floor(q/8)
eligible iff k>=1; first eligible j=2*(8-f)
table_x = (q-8 + (BG3HOFS & 1016)) mod 1024
table_y_H = BG3VOFS mod 1024
table_y_V = (BG3VOFS+8) mod 1024
BG3 column = floor(table_x/8) mod W3
BG3 row = floor(table_y/T3) mod H3; T3=8 or 16 from BGMODE bit 6
```

BG3 horizontal units are full dots, its column width is always eight dots,
and its fine H bits are ignored. Its size bit changes only row height here:
at T3=16 and V3 mod 16<8 the H/V lookups select the same word; otherwise
V selects the next row. It never doubles descriptor-column width. This
Mode-6-specific rule follows the explicit final sentences of Anomie's Mode 6
section, rather than importing Mode 2's large-BG3 horizontal repetition.

BG3SC sets W3/H3=32 or 64. For column c and row r after table wrapping:

```text
page=floor(c/32)+floor(r/32)*(W3/32)
byte=((BG3SC & 252)*512 + page*2048 + (32*(r mod 32)+c mod 32)*2) mod 65536
```

Each word is raw. Only bit 13 enables BG1; bit 14 alone does not. H uses
`word & 0x3f8`, V uses `word & 0x3ff`. Bits 10..12 and 15 are ignored;
H low three bits are ignored too. Mode 4's axis selector is absent.

```text
sx ordinary = j+2*H
sx enabled H = j+2*f+2*(Hword & 0x3f8)
sy ordinary = y+1+V
sy enabled V = y+1+(Vword & 0x3ff)
sx %= 16*W1; sy %= T1*H1
```

The factor two preserves BG1 fine scroll in full dots and preserves phase.
BG1 remains 16 samples wide and 8/16 rows high, with whole-cell flips and
ten-bit character wrapping. Even samples use TS and odd samples use TM.
BG3 is never a visible layer. Disabled axes use ordinary hires independently.
The Issue #19 equation is thus recovered in full-dot units from column numbering,
not assumed in half-dot units. No contradictory evidence was found for Modes
2/4, so their implementation and expectations remain intact.

### Distinguishing literal vectors

H1=35, V1=19, H3=31, V3=17, T3=8, base=$E000: first eligible j=10.
BG3 column 3/row 2 is H at $E086; V row 3 is $E0C6. With words $BC2F
and $BC35, H=40 and V=53. At y=6:

| j | ordinary source | both enabled source | H/V bytes |
| --- | --- | --- | --- |
| 8,9 | (78,26),(79,26) | (78,26),(79,26) | no lookup |
| 10,11 | (80,26),(81,26) | (96,60),(97,60) | $E086/$E0C6 |
| 24,25 | (94,26),(95,26) | (110,60),(111,60) | $E086/$E0C6 |
| 26,27 | (96,26),(97,26) | (288,80),(289,80) | $E088/$E0C8 (H=128,V=73) |

This distinguishes an eight-half-dot boundary, a sixteen-full-dot boundary,
undoubled fine scroll, and accidental use of BG3 fine H. At T3=16,V3=0,
both lookups read $E006; word $2045 supplies H=64,V=69. At V3=8 they
read $E006/$E046. Horizontal entries still advance at j=26, regardless of T3.

For H1=0,H3=248,V3=248,T3=8, the boundary j=16 selects c31/r31;
j=32 selects c32/r31. Literal H/V addresses by BG3SC layout:

| layout | j=16 H/V | j=32 H/V |
| --- | --- | --- |
| 32x32 | $E7FE/$E03E | $E7C0/$E000 |
| 64x32 | $E7FE/$E03E | $EFC0/$E800 |
| 32x64 | $E7FE/$E83E | $E7C0/$E800 |
| 64x64 | $E7FE/$F03E | $EFC0/$F800 |

Base $F800 adds $1800, wrapping $E7FE to $FFFE and $F800 to $1000.
At T3=16,V3=504 the same addresses apply; H3 remains 248, not 504.
At H3=504,V3=1016, a 64x64 table wraps c63/r63 to c0/r0.

Residual uncertainty: these are source-derived static addressing rules. Tests
can detect implementation regressions, not independently establish disputed
hardware behavior. Physical fine-boundary measurements, exact fetch/raster
timing and mosaic interactions remain outstanding and are not claimed here.

## Implementation and independent numerical evidence

Only `crates/snes-core/src/ppu.rs` changes production behavior. Raw map lookup
now takes an explicit column width: character lookup keeps its existing hires
sample units, while Mode 6 descriptor lookup uses eight-full-dot columns.
Mode 6 converts output samples to dots for descriptor selection and doubles
the replacement H offset/fine scroll for character sampling. The shared
Mode 2 path is algebraically unchanged; Mode 4 retains its own axis selector.
No scheduler, frontend, output geometry or public API changes are needed.

`crates/snes-core/tests/ppu_mode6_offset.rs` supplies ten tests. Its original
coordinate image is constructed independently of VRAM addressing; tests select
literal source-coordinate vectors or fixed crop displacements. Neither the
oracle nor fixture construction calls production address helpers.

| test | distinguishing evidence |
| --- | --- |
| `disabled_offsets_retain_ordinary_hires` | Both BG1 heights, all BG1 layouts, H=35/V=19, both image edges/phases; BG2-only enables and all unused bits retain ordinary addressing |
| `independent_axes_enable_masks_and_ignored_metadata` | All 16 H/V enable-mask pairs, bit 15/10..12 ignored, H low bits ignored, first boundary and right edge, independently disabled axes |
| `all_fine_residues_and_successive_full_dot_columns` | Every BG1/BG3 fine-H residue, both BG1/BG3 sizes, initial/successive boundaries in both phases, disabled following column and scanline-independent table selection |
| `bg3_layouts_page_edges_heights_and_64k_wrap` | All BG3SC layouts, both BG3 row heights and BG1 heights, literal H/V byte addresses at page edges, base $F800 wrapping across 64 KiB |
| `large_bg3_shared_words_and_vertical_fine_crossing` | Every BG3 V residue 0..15, shared vs separate H/V words, no horizontal repetition for large BG3 |
| `source_and_descriptor_table_wrap_both_phases` | H=1023/V=999, source wrapping, BG3 c63/r63 to c0/r0, H3=504/1016 equivalence, both BG3 sizes including V3=1020 ten-bit wrap |
| `offsets_feed_whole_cell_flips_and_character_number_wrap` | All flips, both BG1 heights, subcharacter boundaries and N=1023 wrapping to N+1=0/N+16=15/N+17=16 |
| `bg3_hidden_and_main_sub_phase_controls` | BG3 descriptors operate while hidden; enabling BG2/BG3 adds no visible layers; independent TM/TS phase controls |
| `source_built_enabled_guests` / `source_built_disabled_controls` | Eight original CPU-only cartridges, H/V/both/neither at fine H=0/3, literal pixels on both sides of the initial boundary in both phases |

Each CPU guest initializes VRAM, CGRAM and registers under forced blank using
executed instructions. The host observes `$30=$51`, exactly three frame wraps
within 150,000 steps, 512x224 RGBA geometry and literal red/green/white/blue
pixels; it injects no machine state. No cartridge binary is committed. Source,
graphics, data and code are original MIT work; see the
[provenance decision](../tests/roms/PROVENANCE.md#original-mode-6-offset-guests-and-graphics-approved).

An initial test-development failure used a wrapped-character map word only on
row zero while testing row one for eight-row BG1 cells. Initializing both rows
with the same intended word corrected the fixture. No production behavior or
pre-existing expectation was changed to accommodate it.

## Baseline comparison

Created an isolated worktree at exact evaluation baseline
`5230d50c7163c08918b328382e303d879bfa95e6`, copied only the final new test
file, and ran it without production edits. Result: **8 failed, 2 passed**
(expected exit 101). Both disabled controls passed; all enabled test functions
failed. Representative observed vs required RGBA:

| assertion | baseline | expected |
| --- | --- | --- |
| CPU H-only, fine=0, j=16 | (255,0,0,255) | (0,255,0,255) |
| BG3 page edge j=16, source (144,54) | (148,82,16,255) | (205,41,139,255) |
| BG3 large row, j=32, source (288,136) | (197,197,197,255) | (172,139,106,255) |
| ten-bit source wrap j=2,y=10, source (0,5) | (16,213,148,255) | (24,57,90,255) |

Reproduce (the last command is expected to fail on baseline):

```sh
git worktree add --detach .cache/issue-51/baseline 5230d50c7163c08918b328382e303d879bfa95e6
cp crates/snes-core/tests/ppu_mode6_offset.rs .cache/issue-51/baseline/crates/snes-core/tests/
cargo test --manifest-path .cache/issue-51/baseline/Cargo.toml --locked -p snes-core --test ppu_mode6_offset
```

## Mandatory validation (2026-10-24)

Executed in the assigned macOS worktree with Homebrew Rust 1.94.0, Python
3.13.13 and Node 24.12.0. Wasm uses the rustup compiler owning its installed
target. Only the auxiliary SPC assembler uses installed Rust 1.99.0 and the
build script's existing bootstrap setting.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo test --locked -p snes-core --test ppu_mode6_offset --test ppu_offset --test ppu_hires
cargo test --locked -p snes-core --test ppu_mode6_offset
python3 tools/make-fixture.py .cache/issue-51/smoke.bin
cmp tests/roms/smoke.bin .cache/issue-51/smoke.bin
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
SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'
```

- Formatting and warning-denying Clippy passed. Workspace: **264 tests passed**.
  Focused targets: **10 Mode 6 offset, 12 Mode 2/4 offset, 11 ordinary hires**
  passed. Existing test files and expectations are unchanged.
- Pinned tool builds passed. The separate SPC assembler retained its existing
  warning about `rust-objcopy`/`libLLVM.dylib`; its build and hardware execution
  succeeded. Workspace Clippy remained warning-free.
- Hardware suites passed: full CPU last test $0649/PC $0081A2; basic CPU
  $0452/PC $0081A2; SPC $0557/PC $008060. HDMA passed all eleven grayscale
  bands of sixteen scanlines and the final shade through line 223. Evidence:
  `.cache/hardware-tests/results.json`, `.cache/ppu-test/results.json`.
- Smoke regeneration matched byte-for-byte, SHA-256
  `73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb`.
- Both fresh cleared-game builds retained Castle SHA-256
  `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`
  and Memory `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
- npm installation/audit (zero vulnerabilities), release Wasm build and
  Chromium installation passed. Browser suite: **20 passed in 2.8 minutes**
  using the documented alternate port. This includes both actual Wasm/canvas
  hires tests, Castle/Memory five-minute emulated replays, Castle's full route
  and credits, Memory progression/music/effects, gesture-activated Web Audio,
  audio queue/drift checks, freeze negative control, persistence failure/recovery
  and Pages subpath loading. Browser evidence remains in `web/test-results`.

All fixture pins, substitutions, licenses, notices, hashes, gameplay and audio
assertions, actual Wasm/canvas checks, replay inputs and negative controls are
preserved. No commercial ROM or firmware was acquired. Optional unaudited games
are excluded by the prescribed browser filter; physical hardware, optional extra
scenes and post-merge checks were neither required nor performed.

## Remaining scope and inventory

This adds bounded non-mosaic, non-interlaced Mode 6 BG1 addressing evidence.
All 24 inventory categories retain their prior scope: `cpu`, `bus`, `scheduler`,
`dma`, `hdma`, `controllers`, `ppu-bg`, `ppu-offset`, `ppu-hires`, `ppu-mode7`,
`ppu-objects`, `ppu-color`, `ppu-ports`, `ppu-display`, `spc`, `ipl`, `dsp-noise`,
`dsp-pitch`, `dsp-envelope`, `dsp-echo` and `dsp-timing` remain **partial**;
`dsp-voices` remains **implemented_unverified**; `ppu-raster` and `pal` remain
**unimplemented**. In particular, mosaic/offset interactions and exact
fetch/raster behavior still prevent completion of `ppu-offset`. Hires math,
windows, OBJ accuracy, pseudo-hires, interlace, overscan and PAL are outside
this work. Passing these checks establishes neither full hardware accuracy nor
commercial compatibility and does not complete Issue #3.
