# Issue #65: Mode 7 visible screen-row origin

## Source reconciliation before the production change

On 2026-10-24, reopened both primary documents and saved their retrieved bytes
under ignored `.cache/issue-65/`. Their hashes match the copies recorded for
#59/#63; this conclusion is based on these passages, not previous test success:

- [Fullsnes, SNES PPU Rotation/Scaling](https://problemkaputt.de/fullsnes.htm#snesppurotationscaling),
  no$sns v1.5 extraction, SHA-256
  `3853b65a7331bf12eea431ff494a2948a1c78ef22f7e819db4b139fd5a79f6c6`.
  Retrieved HTML line 1176 states exactly:
  `SCREEN.Y = Display (Target) Y-Coordinate: (1..224 or 1..239) XOR (yflip*FFh)`.
  Line 1182 repeats that range in the conditional flip calculation. These
  expressly apply the flip to the one-based display coordinate. Lines 1183–1189
  separately truncate displacement and B/D row products; horizontal A/C terms
  retain their fractions. Lines 1220–1223 clarify the six-bit truncation.
- [Anomie's Register Doc](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt),
  revision 1157, dated 2007-07-12, SHA-256
  `19dfdf3f1c206655b8ede11bcd6dc053e36634f876819e40bb39cfb88a237dfd`.
  $211A, lines 369–371, defines flips on a `256x256 pixel 'screen'`.
  $211B–$2120, lines 376–436, specifies signed 8.8 coefficients, signed 13-bit
  centers and clipped displacements with separately masked products. Its
  qualification at line 423 is retained exactly: “The bit-accurate formula
  seems to be something along the lines of:”. Its SX/SY range is implicit;
  it neither supplies nor contradicts Fullsnes's explicit one-based Y range.
  BACKGROUNDS / BG Scrolling, lines 1643–1650, explains that scanline zero is
  internally rendered but not output, and why games often use VOFS=-1.
  This explains the scanline/display terminology; it does not authorize a
  global planar-BG coordinate change in this bounded Issue.
  BACKGROUNDS / Mode 7, lines 1914–1935, describes BG2's use of the same map,
  character data and Mode 7 scroll with independent seven-bit decode/priority.
- Fullsnes's Port Notes (1201 onward) also describe internal multiplication
  activity on scanline zero. Internal preparation on zero is distinct from
  its explicitly defined visible SCREEN.Y range. Neither this note nor
  Anomie's discussion measures physical fetch latency or arbitration.

The scheduler already calls `render_line(v-1)` for physical scanlines 1–224.
Thus its framebuffer coordinate r is intentionally zero-based, while Mode 7's
static transform coordinate is s=r+1. In the supported non-mosaic,
non-interlaced 256-pixel path, first convert to s, then apply VFLIP. For example,
row zero flipped is 1 XOR 255 = 254; flipping row zero then adding one would
produce 256 and is incorrect. HFLIP remains x XOR 255.

The reconciled contract is:

```text
r = framebuffer row (0..223); s = r + 1
xf = HFLIP ? (x XOR 255) : x
yf = VFLIP ? (s XOR 255) : s
clip(v) = (v & 1023) - (1024 if v & 8192 else 0)
T(v) = v & ~63
dh = clip(s13(HOFS) - s13(CX)); dv = clip(s13(VOFS) - s13(CY))
PX = (T(A*dh) + T(B*dv) + T(B*yf) + 256*s13(CX) + A*xf) >> 8
PY = (T(C*dh) + T(D*dv) + T(D*yf) + 256*s13(CY) + C*xf) >> 8
```

A–D are signed 16-bit 8.8, all shifts arithmetic. Negative T rounds downward
in multiples of 64; the final shift floors. #63's displacement clipping,
separate product truncation, screen-over addressing and layer decode remain.
The only production edit adds one in `Ppu::mode7`, before its V flip; both BG1
and EXTBG BG2 traverse that same code. Framebuffer storage, planar BGs, OBJ,
windows, HDMA and scheduler events retain their coordinates.

## Independent literal pixel vectors

`ppu_mode7_transform.rs` uses only the baseline's public PPU interface. Each
case places one red raw $81 pixel at a literal source coordinate; the rest of
the 128x128 map is green, with only one unique map cell selecting the marked
character. Its other pixels are green. Thus adjacent rows and remote cells
cannot alias through repeating stripes. CGRAM 1/129 is literal red
[255,0,0,255], 2/130 green [0,255,0,255]. Every vector runs BG1-only and BG2-only.
Expected coordinates/colors never use a production helper or runtime transform.

Independent derivations of every new coordinate family (all omitted registers
zero; x=7, except the mixed-sign control):

| Framebuffer r | s / V-flipped s | D=256 source Y | B=384 or D=384 source component | B=-384 or D=-384 source component |
| --- | --- | --- | --- | --- |
| 0 | 1 / 254 | 1 / 254 | 1 / 381 | -2 / -381 |
| 9 | 10 / 245 | 10 / 245 | 15 / 367 | -15 / -368 |
| 223 | 224 / 31 | 224 / 31 | 336 / 46 | -336 / -47 |

D-only identity vectors set A=256, so source X=7 and Y is the third column.
B-only vectors set A=C=D=0, so X is column four and Y=0. Fractional D-only
vectors have X=0 and Y from column four. Negative tests substitute -384 into
one row coefficient at a time; the other component is zero. The products
384*s and -384*s already are multiples of 64, so flooring +/-1.5*s gives the
last two columns. Last-row tests are separate functions so baseline panics on
row zero cannot hide last-row failures. The identity controls also derive
(0,1), (7,9), (255,224) directly from (x,r+1).

For A=128,B=64,C=-64,D=128,H=5,V=-3,CX=2,CY=-2,
dh=3,dv=-1. PX's numerator is 832+64*yf+128*xf;
PY's numerator is -832+128*yf-64*xf. At (x,r)=(7,9),
xf=7/248 and yf=10/245, giving these literal results:

| M7SEL | X numerator | Y numerator | Source (X,Y) |
| --- | --- | --- | --- |
| 0 | 2368 | 0 | (9,0) |
| H | 33216 | -15424 | (129,-61) |
| V | 17408 | 30080 | (68,117) |
| H+V | 48256 | 14656 | (188,57) |

For A=-96,B=160,C=192,D=-48,H=-4,V=7,CX=-3,CY=4,
(x,r)=(5,3): dh=-1,dv=3,yf=4. X's separately truncated terms are
64+448+640-768-480=-96; Y's are -192-192-192+1024+960=1408.
Arithmetic shifts give (-1,5), independently exercising negative fractional D
and nonzero signed offsets/centers.

## Reconciliation of earlier expectations

These supersede the historical row convention, not #63's clipping/truncation:

- #59 `hv_interleaving_changes_vertical_capture_without_planar_interference`:
  VOFS=21 now selects source rows 22/23/24/25 at framebuffer 0/1/2/3,
  hence blue/white/yellow/black. Rows 48/49/50/51 select source 70/71/72/73,
  hence yellow/cyan/magenta/black; row 223 selects 245, black. This still
  discriminates independently latched V=21 from the planar-latch value 66.
- #59 `positive_negative_offsets_fine_tile_boundaries_and_ignored_upper_bits`:
  horizontal vectors remain; vertical vectors use literal samples of r+1+V.
  For V=1, (r=0/4/5/6/12/13/14) maps to source
  2/6/7/8/14/15/16 (blue/white/yellow/cyan/magenta/red/green).
  V=7 maps 0/6/7/8/16 to 8/14/15/16/24;
  V=8 maps 0/5/6/7/15 to 9/14/15/16/24.
  V=-1 maps 0/1/2/7/8/15/16 to 0/1/2/7/8/15/16.
  V=-8 maps 0/6/7/8/14/15/22/23 to -7/-1/0/1/7/8/15/16.
  Original source colors and upper-bit encodings remain; negative -8 explicitly
  retains out-of-map black coverage. V=-1 at framebuffer zero is now source
  zero/red, so only that obsolete black assertion is removed.
- #61 `shared_source_with_small_offset_ignores_bg2_planar_settings`: at
  framebuffer (24,24), H=3,V=2 now selects (27,27), instead of (27,26).
  Its four source bytes move to character row 3, matching the new literal Y.
  Columns 27–30, raw $80/$81/$FF/$7F, all three routings and unrelated planar
  settings remain. No offset is changed.
- #61 original EXTBG guest map initializes tile rows 0–27 only, through source
  223. Framebuffer row 223 now samples source 224/tile row 28, character zero,
  raw zero and gray backdrop [131,131,131,255]. Native/browser tests assert it
  explicitly and add framebuffer row 222/source 223 for all previous literal
  priority/transparency/direct-color colors. Its guest source/bytes/hash stay
  unchanged. All other #61 scenes are constant across character rows.
- #63 identity changes to (x,r+1); mixed-sign and ignored-upper-bit vectors
  now use (9,0) and (-1,5). Normal/V flip sources (9,-1)/(68,118) become
  (9,0)/(68,117); H and H+V retain (129,-61)/(188,57) after flooring.
- #63 separate B/D truncation probes move from framebuffer row 7 to row 6,
  preserving screen Y=7 and the original discriminator T(32)+T(224)=192
  versus combined truncation to 256; the -32 case remains -64-256=-320.
  The A=C=16/B=D=32 case retains 48+192=240 versus 272. This preserves
  sensitivity to separate truncation instead of silently losing that check.
- #63 displacement boundaries: the A/C cases remain unchanged; at row zero,
  the B/D cases add T(128*1)=128 before the final shift. Both source components
  use the following literal table (offset and center in signed register units):

| Offset | Center | A/C source | B/D source |
| --- | --- | --- | --- |
| 1023 | 0 | 511 | 512 |
| 1024 | 0 | 0 | 0 |
| 1025 | 0 | 0 | 1 |
| -1023 | 0 | -512 | -511 |
| -1024 | 0 | -512 | -512 |
| -1025 | 0 | -1 | 0 |
| 4095 | -4096 | -3585 | -3584 |
| -4096 | 4095 | 3583 | 3584 |
| 0 | -1023 | -512 | -511 |
| 0 | -1024 | -1024 | -1024 |
| 0 | -1025 | -1025 | -1024 |
| 0 | 1023 | 511 | 512 |
| 0 | 1024 | 512 | 512 |
| 0 | 1025 | 1024 | 1025 |

Each entry follows center + floor(clip(offset-center)/2) for A/C or
center + floor((clip(offset-center)+1)/2) for B/D. The same signed inputs,
clipping transitions and two independent output components remain tested.
All row-independent A/C, screen-over, character-zero, latch, raw decoding,
priority, transparency and direct-color controls remain. In particular #63's
four B=D=0 CPU guests retain their exact register writes/bytes/hashes; they
are useful arithmetic controls but cannot establish row origin.

## Exact-baseline counterexamples

Created a detached ignored worktree at the exact requested baseline, copied
only public-API tests and their original builder, and ran:

```sh
git worktree add --detach .cache/issue-65/baseline a321b4b757cae117a8241be614a6a954b00f53b0
cp crates/snes-core/tests/ppu_mode7_transform.rs .cache/issue-65/baseline/crates/snes-core/tests/
cp crates/snes-core/tests/ppu_mode7_row.rs .cache/issue-65/baseline/crates/snes-core/tests/
cp crates/snes-core/tests/common/mode7_row.rs .cache/issue-65/baseline/crates/snes-core/tests/common/
cargo test --manifest-path .cache/issue-65/baseline/Cargo.toml --locked -p snes-core --test ppu_mode7_transform row_origin
cargo test --manifest-path .cache/issue-65/baseline/Cargo.toml --locked -p snes-core --test ppu_mode7_row
```

Compilation succeeds with the baseline public API. All eleven selected direct
tests and all four natural-scheduler guest tests fail on pixel assertions,
with observed green [0,255,0,255] instead of red [255,0,0,255]. No setup,
unsupported-API or timeout failure is counted. Separate last-row tests execute
both X/Y and V-flipped discriminators despite first-row failures elsewhere:

| Test vector | Baseline source | Required source |
| --- | --- | --- |
| D=256, row 0, normal / V | (7,0) / (7,255) | (7,1) / (7,254) |
| D=256, row 223, normal / V | (7,223) / (7,32) | (7,224) / (7,31) |
| B=384, row 0, normal / V | (0,0) / (382,0) | (1,0) / (381,0) |
| B=384, row 223, normal / V | (334,0) / (48,0) | (336,0) / (46,0) |
| B=-384, row 0, normal | (0,0) | (-2,0) |
| D=384, row 0, normal | (0,0) | (0,1) |
| Mixed signed centers, (7,9), normal | (9,-1) | (9,0) |
| A=B=D=256 guest, first row, normal / V | (7,0) / (262,255) | (8,1) / (261,254) |

The source columns above are independent arithmetic diagnoses; actual recorded
observations are public-framebuffer RGBA failures in `.cache/issue-65/baseline-*.log`.
The corrected implementation passes those same eleven tests and all four guests.

## Original CPU guest, native runner and browser

The MIT builder in `tests/common/mode7_row.rs` creates four variants with
A=B=D=256,C=0 and zero offsets/centers. Thus the equations reduce exactly to
PX=x+yf, PY=yf. At x=7 and rows 0/9/223, normal source coordinates are
(8,1)/(17,10)/(231,224); V-flipped are (261,254)/(252,245)/(38,31).
Each source occupies a distinct map cell and a distinct marked character.
All six markers are present in every variant, with red/blue/yellow by row;
the rest of the full map and other character pixels are green. Rendering
x=6/8 or adjacent rows must remain green, so repeated/shifted artwork cannot
hide a wrong row. Literal colors also distinguish endpoints from the interior.

The CPU initializes forced blank, complete map, artwork, CGRAM, transform and
routing, releases blank, writes $30=$65 and loops. Native tests load only the
ROM, execute the natural scheduler, bound startup by 300000 instructions/eight
frames, reject STP/APU errors and discard setup. They check three complete
stable frames per variant, with fewer than 100000 instructions per frame and
`run_frame`'s 400000-clock bound. Geometry is 256x224 and 229376 RGBA bytes.
Native runner checks execute each generated cartridge for six frames, require
$30=$65 and parse P6 output: 256x224, 172032 RGB bytes, red at (7,0), blue at
(7,9), yellow at (7,223), green at (6/8,0/9/223) and (7,1/8/10/222).
The runner check is reproducible after building the four fixtures:

```sh
cargo build --locked -p snes-core --bin snes-run
python3 - <<'PYPPM'
from pathlib import Path
import subprocess
for rom in sorted(Path('.cache/mode7-row').glob('*.sfc')):
    ppm = Path('.cache/issue-65', rom.stem + '.ppm')
    subprocess.run(['target/debug/snes-run', str(rom), '--frames', '6',
                    '--timeout', '10', '--expect', '0x30=0x65',
                    '--ppm', str(ppm)], check=True)
    magic, geometry, depth, pixels = ppm.read_bytes().split(b'\n', 3)
    assert (magic, geometry, depth, len(pixels)) == (b'P6', b'256 224', b'255', 172032)
    for y, color in [(0, [255,0,0]), (9, [0,0,255]), (223, [255,255,0])]:
        for x, expected in [(6, [0,255,0]), (7, color), (8, [0,255,0])]:
            a = (y * 256 + x) * 3
            assert list(pixels[a:a+3]) == expected
    for y in [1,8,10,222]:
        a = (y * 256 + 7) * 3
        assert list(pixels[a:a+3]) == [0,255,0]
PYPPM
```

The browser builds the identical cartridge bytes and uses the ordinary ROM
picker and Play button. Three observations separated by animation callbacks
compare actual Wasm framebuffer bytes and canvas ImageData at the same 13
points, completion $65, 256x224/229376 geometry and core error length zero.
It rejects console/page errors and any UI core error. Each observation is
bounded by five seconds, with a 30-second test deadline. The unchanged mandatory
`--grep-invert '301 seconds'` selection automatically includes all four tests.
No host register or raster injection, production oracle, commercial image or
firmware is used. [Provenance](../tests/roms/PROVENANCE.md#original-mode-7-screen-row-guests-approved)
records original authorship, MIT, source/build paths and all hashes. Both new
fixture builds compared byte for byte. Old #59/#61/#63 guest controls are intact.

## Mandatory validation on 2026-10-24

Toolchain: Rust/Cargo 1.99.0 (rustc b940084d7, Cargo 5f94df478), Node 24.12.0,
Python 3.13.13. Like #55–#63, selected that installed Rust toolchain's bin/RUSTC
and used an ignored wasm-ld wrapper setting its lib directory in
DYLD_LIBRARY_PATH before invoking rust-lld. This addresses the mixed Homebrew/
rustup installation; it changes no repository compiler flags or expectations.
Logs are ignored under `.cache/issue-65/`.

Executed commands:

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo test --locked -p snes-core --test ppu_mode7_transform --test ppu_mode7_scroll --test ppu_mode7_extbg --test direct_color --test ppu_pseudo_hires --test ppu_obj_limits --test ppu_mode7_row
cargo run --locked -p snes-core --example mode7-row-fixture -- .cache/mode7-row
cargo run --locked -p snes-core --example mode7-row-fixture -- .cache/issue-65/rebuilt
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
SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'
```

Formatting and warning-denying Clippy pass. All **350 workspace tests** pass;
the focused command passes **84 tests across seven targets** (26 transform,
four row guests). Smoke rebuild matches SHA-256
`73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb`.
All pinned assembler builds pass; CPU full/basic end at $0081A2 with last tests
$0649/$0452, SPC ends at $008060/$0557, and all eleven sixteen-row grayscale
bands pass with the final shade through row 223. Existing auxiliary tool builds
emit the previously documented macOS rust-objcopy/libLLVM strip warning; builds
and executions succeed. Pinned sources, licensing and all assertions remain.

Both cleared-game builds pass and retain their pinned sources/hashes:
Castle `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`
and Memory `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
The game source pins, licenses, replay inputs, durations, gameplay/audio checks
and browser selection remain unchanged. npm ci reports zero vulnerabilities;
production Wasm build and Chromium installation pass.

The full mandatory Chromium selection passes **37 tests in 2.9 minutes**, including all four
new screen-row checks, existing Mode 7 scroll/EXTBG/transform controls, both
cleared-game five-minute replays, Castle's full route, Memory music/effects and
Web Audio, queue/drift/stall and freeze-negative controls, save recovery,
hires/SETINI, OBJ and Pages smoke checks. There are no skipped or failed tests.
The native PPM checks pass for all four variants. No mandatory gate remains
failed or unperformed. Staged whitespace, JavaScript syntax, Markdown local destinations/anchors
and the verbatim 24-row inventory comparison pass. Only source and documentation
are committed; generated ROMs, source-built tools, PPM images and logs remain
ignored.

Intentional exact-baseline failures are separate from final gate results.
Initial correction runs exposed the old #59/#61 row expectations described
above; their source-based reconciliation preserved latch and composition
coverage. No hardware measurement was performed. Optional external games were
not acquired or run; no required post-merge verification exists.

## Remaining uncertainty and inherited inventory

The following 24 category rows are retained verbatim from #63, including their
complete scopes and statuses: 21 partial, dsp-voices implemented_unverified,
ppu-raster/pal unimplemented. No category becomes verified.

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

Mode 7 remains partial. This resolves the documentary visible-row discrepancy
only for static, non-mosaic, non-interlaced 256-pixel sampling. Mosaic,
interlace/overscan, EXTBG with pseudo-hires, comprehensive transform/window/
color-math interactions, multiplication latency/rendering interference,
physical reset retention, mid-scanline register changes and physical fetch/
arbitration timing remain outside this Issue. Anomie's arithmetic remains
qualified as tentative. Documentary agreement and software regressions do not
verify the physical rendering pipeline or imply physical-console measurement.
All inherited inventory limitations above remain in force. No full hardware
accuracy or commercial compatibility is claimed, and partial Issue #3 work
is not completion. No commercial ROM, firmware or third-party emulator
implementation was acquired.
