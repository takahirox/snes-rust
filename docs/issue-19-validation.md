# Issue #19 Mode 2/4 offset-per-tile validation

Implemented on 2026-10-23 for [Issue #19](https://github.com/takahirox/snes-rust/issues/19),
against evaluation baseline `c7ff61b9c927fe283ef1a4f753c1075d5b8f237c`.
Only `crates/snes-core/src/ppu.rs` changes production behavior. The existing
scanline renderer now resolves BG1/BG2 source coordinates through raw BG3 map
words in Modes 2/4. Character decoding and composition consume those coordinates
through their existing paths. There are no game-specific exceptions, public API,
output geometry, scheduler or frontend changes.

## Specification attribution

- [Fullsnes](https://problemkaputt.de/fullsnes.htm), **SNES PPU BG Control**:
  **2105h BGMODE**, **2109h BG3SC**, **2111h BG3HOFS**, **2112h BG3VOFS**;
  **SNES PPU Video Memory (VRAM), BG Map** gives offset-descriptor fields.
  These establish tile sizes, map pages/base, scroll register latching, the two
  layer-enable bits, unused bits, the Mode 4 axis selector and coarse H offset.
  **SNES PPU Offset-Per-Tile Mode** is marked under construction and refers
  readers to Anomie's documentation; it does not supply a complete algorithm.
- [Anomie's Register Doc, revision 1157, 2007-07-12](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt),
  **BACKGROUNDS / BG Scrolling**, **Mode 2**, **Mode 4**: replacement axes,
  initial unaffected region, retained H fine scroll, scanline-independent BG3 row
  selection and single-word Mode 4 selection. Its Mode 2 discussion explicitly
  distinguishes eight-pixel offset columns from affected 16×16 characters and
  explains repeated descriptors/shared H/V words when BG3 uses 16×16 tiles.
- The Issue's explicit `q=x+(BGnHOFS&7)` equations govern the first boundary and
  table-column calculation. This avoids depending on the older document's
  tentative pseudocode when fine scroll carries across an eight-pixel boundary.
  Only specification facts were consulted; no emulator implementation was copied.

## Independently derived addressing vectors

All coordinates below are decimal unless prefixed with `$`. Display line `y`
uses background line `s=y+1`. The tests supply source-coordinate expectations,
then calculate RGBA from original coordinate-labelled graphics; the oracle does
not call the production coordinate, map or planar helpers. Separate composition
and CPU-guest assertions use literal RGBA.

For ordinary H scroll 35 (coarse 32, fine 3), ordinary V scroll 19 and display
line 6, the first eligible x is 5. Mode 2 H word `$9C2F` has coarse offset 40
and V word `$9C35` has offset 53; enable bits are added separately. The high
unused bits and H low three bits must not change these values.

| screen (x,y) | neither enabled | H only | V only | both |
| --- | --- | --- | --- | --- |
| (4,6) | (39,26) | (39,26) | (39,26) | (39,26) |
| (5,6) | (40,26) | (48,26) | (40,60) | (48,60) |
| (13,6) | (48,26) | (56,26) | (48,60) | (56,60) |

Mode 4 descriptor `$1C35` plus enable bits replaces H with coarse 48; setting
bit 15 instead replaces V with 53. At `(5,6)` the respective sources are
`(56,26)` and `(40,60)`. The following BG3 row contains enabled `$63F1`, which
must have no effect in either case. Mode 2 independently tests all 16 H/V enable
mask pairs; both modes test BG1-only, BG2-only, both and neither masks for each
background and both character sizes.

For each affected fine scroll f=0..7 and BG3 H scroll 24+t (t=0..7), the boundary
is `e=8-f`. BG3 V scroll 17 selects map row 2. Its columns 3,4,5 hold H offsets
64,128,192. The independent source-X vectors at screen x
`[e-1,e,e+7,e+8,e+15,e+16,e+24]` are
`[39,72,79,144,151,216,64]`: the final descriptor is disabled, restoring ordinary
scroll. Display lines 0 and 19 use the same table words with source Y 20 and 39.
This matrix crosses offset columns inside affected 16×16 characters too.

With H scroll 1023, ordinary V 999, display line 10, BG3 V 1020,
H descriptor `$63FF` and V descriptor `$63FA`, screen x=0 retains `(1023,1010)`;
x=1 uses `(0,5)`, x=8 uses `(7,5)`. On line 0, x=2 uses `(1,1019)`.
The BG3 H row is its wrapped row 31; BG3VOFS+8 wraps to row 0 for V.

For BG3 8×8, base byte `$E000`, H scroll 248 and V scroll 248, x=8 reads
column 31, row 31 for H; its V comes from row 32. At x=16, column 32 is used.
Each page holds 32×32 words, or 2048 bytes. The independently enumerated byte
addresses are:

| BG3SC layout | x=8 H | x=8 V | x=16 H | x=16 V |
| --- | --- | --- | --- | --- |
| 32×32 | $E7FE | $E03E | $E7C0 | $E000 |
| 64×32 | $E7FE | $E03E | $EFC0 | $E800 |
| 32×64 | $E7FE | $E83E | $E7C0 | $E800 |
| 64×64 | $E7FE | $F03E | $EFC0 | $F800 |

H values 64/128 and V values 53/73 give `(72,54)` at x=8 and `(144,74)` at
x=16 on display line 0. Repeating with BG3SC base `$FC` adds `$11800` to these
addresses and wraps at 64 KiB: e.g. `$E7FE` becomes `$FFFE`, while `$F800`
becomes `$11000`, read at `$1000`. No character/palette/flip interpretation is
applied to any descriptor.

For BG3 16×16 at V=0, y=0 and y=8 select one word. Descriptor `$6045` therefore
supplies both H=64 and V=69 in Mode 2: x=8/16 use `(72,70)` / `(80,70)`.
The next descriptor `$6087` gives x=24 source `(152,136)`. At BG3 V=8, V+8
selects the next row instead, whose `$6033` gives source Y=52. With H/V scroll
504 the four-screen table crosses its page boundaries at 512 rather than 256,
using the last row of the table above. These cases run for both modes, both
backgrounds and both affected character sizes.

All four flip combinations are checked after offset addressing using nonuniform
characters. The composition fixture enables both backgrounds: high BG2 covers
low BG1, high BG1 then wins, raw BG1 zero exposes BG2, and subscreen selection
independently routes either layer into math. Mode 4 BG1 raw `$51`, palette 7,
gives five-bit RGB `(6,10,12)` or RGBA `(49,82,98,255)`, bypassing CGRAM. BG3
remains disabled in TM and TS throughout, including while BG1/BG2 supply subscreen
pixels.

## Original CPU-executed guests and baseline experiment

`tests/ppu_offset.rs` constructs eight original MIT 32 KiB LoROMs in memory:
Modes 2/4 × H/V replacement × enabled/disabled. Every register, VRAM and CGRAM
initialization uses emulated CPU writes under forced blank; no host-side PPU
patching occurs. Each execution is bounded to three frames and 150,000
instructions. With ordinary H=3, x=4 is source x=7 (red), then x=5 is source
x=40 (blue) for enabled H, or source y=33 (white) for enabled V. Disabled
controls remain red. x=12 checks the far end of that offset column. BG3 H fine
bits are 7 and BG3 is absent from TM/TS. See
[provenance](../tests/roms/PROVENANCE.md#original-offset-per-tile-65c816-guests-and-graphics-approved).

The final test file was run with only `ppu.rs` temporarily replaced by the exact
baseline version obtained via `git show c7ff61b9c927fe283ef1a4f753c1075d5b8f237c:crates/snes-core/src/ppu.rs`.
The implementation was restored in a `finally` block before rerunning tests.
`cargo test -p snes-core --test ppu_offset --locked` exited 101 on the baseline:
**10 failed, 2 passed**. Every offset-enabled test failed, including both CPU
mode guests. Both `disabled_offsets_retain_ordinary_scroll` and
`source_built_guests_disabled_controls` passed. Representative failures:

| assertion | baseline RGBA | expected RGBA |
| --- | --- | --- |
| Mode 2 guest enabled H, x=5 | (255,0,0,255) | (0,0,255,255) |
| Mode 4 guest enabled H, x=5 | (255,0,0,255) | (0,0,255,255) |
| BG3 page boundary, source (72,54) | (148,82,16,255) | (205,41,139,255) |
| ten-bit wrap, source (0,5) | (230,98,230,255) | (24,57,90,255) |

Restoring the implementation gives **12 passed, 0 failed**. Existing direct-color
and game-derived regression expectations were not changed.

## Executed validation

Commands ran in the assigned worktree on macOS, with Python 3.13.13, Node
24.12.0, the default Rust 1.94 compiler for workspace checks, and installed
rustup Rust 1.99 for the auxiliary SPC assembler and Wasm build. The initial
default-compiler SPC build failed on unsupported language features, as anticipated
by the README; selecting the installed compiler below passed without source edits.

| command | actual result |
| --- | --- |
| `cargo test -p snes-core --test ppu_offset --locked` | 12 passed, including eight generated guest variants |
| `cargo fmt --all -- --check` | passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | passed |
| `cargo test --workspace --locked` | 110 tests passed: 44 core unit, 7 direct-color, 1 envelope, 4 noise, 3 pitch, 12 offset, 39 regression; no failures/ignored tests |
| `python3 tools/make-fixture.py .cache/smoke.bin` then `cmp tests/roms/smoke.bin .cache/smoke.bin` | passed, original hash unchanged |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | passed, all three pinned tools built |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | passed all 3 suites |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | passed all 11 grayscale bands, 16 lines each, last shade through line 223 |
| `python3 tools/build-cleared-games.py` (twice after tool setup) | both passed pinned ROM hashes from fresh build directories |
| `npm --prefix web ci` | passed, zero audit vulnerabilities |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | passed |
| from `web`: `npx playwright install chromium` | passed |
| from `web`: `SNES_TEST_PORT=18085 npm test -- --grep-invert '301 seconds'` | 18 passed in 2.5 minutes; zero failed or skipped |

The initial browser launch at the default port failed before tests because 8080
was occupied. The README's alternate-port override was then used without changing
any assertions or test selection. Only the existing unaudited optional games are
excluded by the prescribed filter. All seven cleared-game checks passed: Castle
five-minute replay, complete five-map/credits route, Memory five-minute replay,
looping music/finite effects, gesture-activated Web Audio, queue/drift checks,
and the Castle freeze negative control. All ten diagnostic/control/save tests
and the Pages subpath test also passed. Browser evidence remains under
`web/test-results` (JSON, screenshots and audio artifacts as emitted by the
unchanged tests).

The source hardware gates retain their pins, licenses, font replacements and
numerical assertions. Their local JSON evidence remains in
`.cache/hardware-tests/results.json` and `.cache/ppu-test/results.json`. Verified
terminal tests/PCs: full CPU `$0649` / `$0081A2`, basic CPU `$0452` / `$0081A2`,
SPC `$0557` / `$008060`. Their ROM hashes remain:

- Full CPU: `1727e7c46ebb5df55711598264ba19005986bd34e915621383966adf674b3cfa`
- Basic CPU: `514d58950ae1ca940d6673c2a028b258b2ab17293bbc7cbd7cb54dded32abc5b`
- SPC: `279d6a92d051ff8a98e190ce73e40482ff2cc981812509c7bca35a047484507e`
- HDMA: `aef339e5d4a7319db31de577b253d977b45f5c45f09adfaa99a302ca300a957a`
- Smoke: `73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb`
- Cleared Castle: `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`
- Cleared Memory: `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`

## Remaining scope

This establishes bounded non-mosaic Mode 2/4 BG1/BG2 addressing within the current
256×224 scanline snapshot model. Mode 6 remains unimplemented and requires hires
geometry plus its specific addressing. Mosaic/offset interactions, timed mosaic
state, mid-scanline register changes and fetch timing remain explicitly unverified.
`ppu-offset` and `ppu-bg` remain partial; this does not complete `ppu-hires` or
`ppu-raster`. Other inventory categories retain all their previous scope and
evidence, including the existing color, DSP, object and Mode 7 limitations.
These results do not establish full hardware accuracy or commercial compatibility.
No commercial game or firmware was acquired. No post-merge check is required.
