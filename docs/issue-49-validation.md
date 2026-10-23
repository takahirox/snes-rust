# Issue #49: ordinary Mode 5/6 hires

## Specification contract (recorded before implementation)

Retrieved 2026-10-24:

- [Fullsnes](https://problemkaputt.de/fullsnes.htm), no$sns v1.5 extraction,
  SHA-256 `3853b65a7331bf12eea431ff494a2948a1c78ef22f7e819db4b139fd5a79f6c6`.
  Sections: BGMODE, BGnSC, BGnHOFS, BG Map, 16x16 Tiles, CGRAM, TM/TS, SETINI.
  The mode table supplies depths; BGnSC supplies page layout. SETINI describes
  “SHIFT SUBSCREEN HALF DOT TO THE LEFT”. CGRAM identifies COLDATA as sub backdrop.
  BGMODE tentatively says Mode 6 is fixed 16x8; its question mark is significant.
- [Anomie's Register Doc](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt),
  revision 1157, 2007-07-12, SHA-256
  `19dfdf3f1c206655b8ede11bcd6dc053e36634f876819e40bb39cfb88a237dfd`.
  Sections: $2105, $210D–14, $2133, Tile Maps and Character Maps,
  BG Scrolling, Modes 5/6, Color Math.
  $210F–14 says “Horizontal scrolling scrolls in units of full pixels”. Mode 5
  says “even pixels for subscreen tiles and the odd pixels for mainscreen tiles”. Mode 6 inherits these
  properties. $2133 assigns even output positions to sub and odd to main.
  Tile Maps describes whole-cell flips and ten-bit character wrap.

The explicit Anomie height rule takes precedence over Fullsnes's tentative
Mode 6 height annotation: both modes use cell width 16 and height 8 or 16.
Fullsnes's exclusion of hires addition/subtraction conflicts with Anomie's hires
math description; this work establishes only math-disabled behavior. No hires
math, clipping/window, OBJ or mosaic accuracy is inferred from these fixtures.

### Derived equations

Let `j=0..511` be the zero-based horizontal active image sample, `y=0..223`
its stored row, `H=BGnHOFS & 1023`, `V=BGnVOFS & 1023`, `T=8 or 16`, and
`Wm,Hm=32 or 64` map entries. With offset-per-tile disabled:

```text
screen(j) = sub if j is even, main if j is odd
sx = (j + 2*H) mod (16*Wm)
sy = (y + 1 + V) mod (T*Hm)
column,row = floor(sx/16), floor(sy/T)
page = floor(column/32) + floor(row/32)*(Wm/32)
map_byte = ((BGnSC & 252)*512 + page*2048
            + ((row mod 32)*32 + column mod 32)*2) mod 65536
u = sx mod 16; v = sy mod T
u = 15-u if H-flipped; v = T-1-v if V-flipped
character = (N + floor(u/8) + 16*floor(v/8)) mod 1024
plane_byte = (NBA*8192 + character*bpp*8 + (v mod 8)*2
              + floor(plane/2)*16 + plane mod 2) mod 65536
bit = 7-(u mod 8)
```

The factor two follows from full-dot scrolling and half-dot output; map wrapping
means masking the intermediate horizontal result to 1023 is sufficient even
though `2*H+j` can exceed 1023. No Issue #19 offset-column equation is used.
Horizontal active origin is sample zero, without an extra dot displacement.
Vertical active origin retains hardware scanline 1 (the renderer's row zero).
TM/TS independently select layers for their phases. Color index zero is
transparent; main backdrop is CGRAM[0], sub backdrop is COLDATA. Brightness
applies to both output phases.

Worked examples for a 64-entry map: H=0 gives source 0,1 at output 0,1 and
510,511 at the right edge. H=1 gives 2,3 and 512,513; the last pair enters
page B. H=7 gives 14,15 then 16,17 (the next map cell). H=8 gives 16,17.
H=255 gives 510,511 then 512,513. H=256 gives 512,513; H=511 gives 1022,1023
then 0,1; H=512 repeats H=0; H=1023 repeats H=511. A 32-entry map repeats
at 512 samples. With T=16,V=7,row zero selects local v=8: characters N+16
and N+17. Both flips turn (u,v)=(0,8) into (15,7), selecting N+1, bit zero.
N=1023's right half wraps to character 0.

### Representation policy, separate from hardware facts

The API exposes packed RGBA with an exact width, height and byte length. A frame
starts compact at 256 samples if row zero is low resolution. Encountering a
hires row promotes the frame to 512 samples by duplicating earlier low-resolution
pixels; later low-resolution rows also duplicate each dot. Width never shrinks
within that frame. At the next row zero a wholly low-resolution frame can become
compact again. Resizing at row zero clears unrendered rows. This prevents stride
reinterpretation; it is not evidence for physical mode-switch/fetch timing.
The browser displays both geometries at a 256:224 shape; scale counts full dots.

## Validation results

The results below were executed in the assigned worktree on 2026-10-24.

### Coverage and baseline counterexamples

`crates/snes-core/tests/ppu_hires.rs` uses original, host-initialized planar
characters labelled by character number and local coordinates. Expected colors
come from a separate linear coordinate image or literal page/flip vectors, not
production addressing/composition functions. Coverage includes both Mode 5 depths,
Mode 6 BG1, both heights, four flips, all BGSC layouts, page boundaries in both
axes, ten-bit character wrap, 64 KiB map/character byte wrap, palettes, transparent
zero, BG priorities, hidden Mode 6 backgrounds, independent TM/TS phases,
forced blank, left/cell/page edges, x=510/511 and all requested scroll values.

CPU fixtures share `tests/common/hires.rs` with the `hires-fixture` example:
register/VRAM/CGRAM initialization is executed by the CPU. Five bounded startup
frames verify `$30=$49`, mode markers, geometry and literal pixels; three frames
per controller-driven switch verify complete 256→512→256 output. Reset and
replacement reacquire the framebuffer. No CPU, bus or PPU state is host patched.
The runner test executes the actual binary on all three static cartridges,
checking P6 headers, exact RGB payload sizes (172032 or 344064 bytes) and edge
colors. Browser tests inspect actual Wasm buffers and canvas pixels at both
horizontal edges and the final row, dimensions/length, shape/scale, reset,
cartridge replacement and page errors. An explicit `memory.grow` also detaches
old views and checks subsequent drawing.

For the comparison, created an isolated detached worktree at inspected commit
`d188cd02a8c5c148d8d3972ad7bd86689ee19b91`. Copied only the new test and shared
fixture generator into that worktree. Since the old public API has no dimensions,
a test-local trait reported its actual fixed width=256 and height=224 for Ppu
and Machine. No baseline production code or expected colors were changed.
The final comparison ran 11 tests: **1 passed, 10 failed**, as intended.
The standalone original Mode 1 CPU control passed. Representative failures:

| Test | Baseline observation | Required result |
| --- | --- | --- |
| geometry / CPU guest | 256×224, 229376 RGBA bytes | 512×224, 458752 bytes |
| map page addressing, x=16 | blue `[0,0,255,255]` | green `[0,255,0,255]` |
| whole-cell flip / wrap, x=0 | green `[0,255,0,255]` | white `[255,255,255,255]` |
| main-only phase, x=0 | red `[255,0,0,255]` | fixed magenta `[255,0,255,255]` |
| runner hires output | fixed `256 224` P6 header | `512 224` header |

Reproduce the isolated comparison (an expected nonzero test exit):

```sh
git worktree add --detach .cache/hires-baseline d188cd02a8c5c148d8d3972ad7bd86689ee19b91
mkdir -p .cache/hires-baseline/crates/snes-core/tests/common
cp crates/snes-core/tests/common/hires.rs .cache/hires-baseline/crates/snes-core/tests/common/
cp crates/snes-core/tests/ppu_hires.rs .cache/hires-baseline/crates/snes-core/tests/
cat >> .cache/hires-baseline/crates/snes-core/tests/ppu_hires.rs <<'RS'
trait BaselineGeometry {
    fn framebuffer_width(&self) -> usize { 256 }
    fn framebuffer_height(&self) -> usize { 224 }
}
impl BaselineGeometry for Ppu {}
impl BaselineGeometry for Machine {}
RS
cargo test --manifest-path .cache/hires-baseline/Cargo.toml --locked -p snes-core --test ppu_hires
```

The first workspace run exposed the old direct-color control's unsupported
assumption that hires output zero uses TM. The corrected control still requires
the same blue CGRAM main color for both direct-color settings, at output one;
it additionally verifies output zero uses disabled-TS COLDATA black. No game
expectation, fixture pin, replay input or negative control was weakened.
An initial new browser reset assertion expected alpha=255 in canvas readback;
actual reset RGBA and canvas readback are zero, matching the existing reset
buffer. The test now explicitly checks those zero bytes and fresh geometry.
An early map-wrap vector overlapped its own wrapped character data with map
pages; relocating wrapped characters to byte $4000 removed that fixture alias.

### Final mandatory checks (2026-10-24)

All commands below passed. The workspace uses Homebrew Rust 1.94.0. Wasm was
built with rustup's compiler matching its installed target; only the separate
SPC assembler used Rust 1.99.0 and its script's existing bootstrap flag.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo test --locked -p snes-core --test ppu_hires
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
SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'
# Recheck the added explicit memory-growth test after the full browser run.
SNES_TEST_PORT=18085 npm --prefix web test -- --grep 'real canvas/Wasm'
```

- Formatting and warning-denying Clippy passed; workspace: **254 tests passed**,
  including all **11 hires tests**. The focused hires command also passed.
- Smoke output matched the committed binary byte-for-byte, SHA-256
  `73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb`.
- Pinned cc65/bass/SPC assembler builds passed. The auxiliary SPC assembler
  emitted its existing warning; workspace Clippy remained warning-free.
  Hardware tests passed all three suites (full CPU, basic CPU and SPC).
  The grayscale PPU fixture passed all eleven 16-row bands and final shade.
- Both cleared-game builds passed without changed hashes or notices.
- npm install/audit, release Wasm build and Chromium installation passed.
  The mandatory browser filter passed **20 tests in 2.6 minutes**. This retains
  Castle/Memory five-minute emulated gameplay replays, Castle's complete route
  and credits, music/effects, real Web Audio drift/queue checks, freeze negative
  control, persistence failures/recovery and Pages subpath loading. Both hires
  canvas/Wasm tests passed. The final focused rerun also passed both tests.
- No existing game checkpoint, gameplay assertion, audio assertion, fixture
  provenance/pin/hash/license, replay or negative control was relaxed. The sole
  pre-existing test adjustment is the source-supported hires direct-color phase
  correction explained above.

Optional unaudited games were excluded by the required filter. No commercial
ROM/firmware was acquired; no physical hardware measurements, optional extra
hires scenes, or post-merge/deployed-site checks were performed or required.

### Generated fixture identity

All six original MIT cartridges are reproducible with the README example command:

| Fixture | SHA-256 |
| --- | --- |
| hires-1.sfc | `42c0e37103aec1697121e588a3132fe5e040108df707eca689fd55f7d8e59cc2` |
| hires-5.sfc | `137c8d63c3ee3a22a645421a7be4b2430b0096ccc6f50f8bafad4668d97661b2` |
| hires-6.sfc | `6eb0273b78d3116c30e8dd485a84478539ce206ab0987fb3a643b4db290cb209` |
| hires-1-switch.sfc | `5aef5b9d15768f761fbf65f5cbe8e0e7370dd3f1df961363e71c3deedac25e76` |
| hires-5-switch.sfc | `2e7d8b6931b8b9073694e25850da99ebe36a5903388c948f4c04916a7ec6085f` |
| hires-6-switch.sfc | `f94fa42c3121f07457c317de047255668f0164d3256cbc8fa9ced3524b5932ad` |

## Remaining uncertainty and inventory

All 24 categories and their prior outstanding scopes are retained from the
[baseline inventory](issue-27-validation.md#remaining-uncertainty-and-inventory)
and README. Only `ppu-hires` advances from unimplemented to **partial**, on the
ordinary non-mosaic, non-interlaced Mode 5 BG1/BG2 and Mode 6 BG1 evidence above.
Mode 6 offset-per-tile, SETINI pseudo-hires, interlace, overscan, PAL, mosaic,
hires-specific math/window/OBJ behavior, mid-scanline effects and pixel-fetch
timing all remain outstanding. Mixed-row promotion is storage policy, not
physical switching validation. This change does not complete `ppu-hires`.

`cpu`, `bus`, `scheduler`, `dma`, `hdma`, `controllers`, `ppu-bg`, `ppu-offset`,
`ppu-mode7`, `ppu-objects`, `ppu-color`, `ppu-ports`, `ppu-display`, `spc`, `ipl`,
`dsp-noise`, `dsp-pitch`, `dsp-envelope`, `dsp-echo` and `dsp-timing` remain
**partial** with every previous limitation. `dsp-voices` remains
**implemented_unverified**; `ppu-raster` and `pal` remain **unimplemented**.
No unrelated hardware scope or accuracy claim changes. The original Issue #3
handoff remains unchanged; these limited passes imply neither full hardware
accuracy nor commercial compatibility.
