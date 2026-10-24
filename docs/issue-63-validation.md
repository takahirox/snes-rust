# Issue #63: static Mode 7 fixed-point sampling

Historical record: [Issue #65](issue-65-validation.md) supersedes the retained
zero-based Mode 7 screen-Y convention and individually reconciles affected
pixel expectations. The clipping/truncation correction and row-independent
CPU guest controls below remain in force.

## Specification contract recorded before implementation and expectations

Retrieved 2026-10-24 before production changes or final oracle authoring:

- [Anomie's Register Doc](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt),
  revision 1157, 2007-07-12, SHA-256
  `19dfdf3f1c206655b8ede11bcd6dc053e36634f876819e40bb39cfb88a237dfd`.
- [Fullsnes](https://problemkaputt.de/fullsnes.htm#snesppurotationscaling),
  no$sns v1.5 extraction, SHA-256
  `3853b65a7331bf12eea431ff494a2948a1c78ef22f7e819db4b139fd5a79f6c6`.

These exactly match the Issue #59 source versions. Local retrieved copies stay
ignored under `.cache/issue-63/`; no external emulator code is used.

Passage-level reconciliation (line numbers refer to those retrieved bytes):

- Anomie $210D/$210E (249–267), $211B–$2120 (376–434); Fullsnes
  Rotation/Scaling (1142–1161): matrices are signed 16-bit 8.8, offsets and
  centers signed 13-bit integers. Ignore the upper three offset/center bits;
  retain the existing shared M7 write latch and independent planar latch.
- Anomie $211F/$2120 (423–436) explicitly defines
  `CLIP(a) = (a & 0x2000) ? (a | ~0x3ff) : (a & 0x3ff)` and supplies the
  three separately masked products for each origin, then the recurrence
  adding A/C per horizontal pixel. Fullsnes Formula (1181–1189) independently
  masks ORG's bits 10–12, restores them for negative ORG, and masks the
  displacement products and B/D times SCREEN.Y with `AND NOT 3Fh`.
  Its A/C times SCREEN.X products are unmasked. The Port Notes (1220–1223)
  clarify that six low bits are discarded before summation, despite the
  multiply-port observation suggesting three. The explicit Formula governs
  the selected products; the note does not require masking horizontal steps.
- With signed-13-bit inputs, differences range from -8191 to 8191. Thus the
  mathematical sign in Fullsnes is exactly difference bit 13 in Anomie.
  Their clipping rules agree throughout that domain. This is bit selection,
  not saturation to +/-1024. In particular +1024 becomes 0, -1025 becomes -1,
  +8191 becomes 1023 and -8191 becomes -1023.
- Clearing six low two's-complement bits rounds a negative product downward
  to a multiple of 64: -32 becomes -64, not 0. Final arithmetic shift by eight
  likewise floors, including negative coordinates. No intermediate overflow
  is needed: the signed products and sums in this bounded domain fit i32.
- Anomie $211A (357–371), $211F/$2120 (416–421) and BACKGROUNDS / Mode 7
  (1880–1935), and Fullsnes M7SEL (1127–1140), agree: encodings 0/1 wrap;
  2 is transparent outside; 3 uses character zero. Anomie explicitly selects
  its pixel using the transformed coordinate's low three bits. Outside status
  must be evaluated before wrapping; character zero is not a map entry lookup.
- Fullsnes EXTBG (1241–1247) and Anomie BACKGROUNDS / Mode 7 agree that BG2
  receives BG1's same eight-bit source, decoding bit 7 as priority. Existing
  independent layer decoding, direct color, routing and composition remain.

Reconciled equations, with `xf=x XOR 255` on H flip and `yf=y XOR 255` on V flip:

```text
clip(d) = (d & 1023) - (1024 if d & 8192 else 0)
T(v) = v & ~63
dh = clip(s13(HOFS) - s13(CX)); dv = clip(s13(VOFS) - s13(CY))
X0 = T(A*dh) + T(B*dv) + T(B*yf) + 256*s13(CX)
Y0 = T(C*dh) + T(D*dv) + T(D*yf) + 256*s13(CY)
PX = (X0 + A*xf) >> 8; PY = (Y0 + C*xf) >> 8
```

These confirm the Issue's three candidate probes. For A=32, H=1, x=7:
T(32)+224=224, hence X=0 (old sum=256, X=1). For A=128,H=1024,x=0:
clip=0, X=0 (old X=512). For H=-1025: clip=-1, T(-128)=-128,
X=-1 (old X=-513). Replacing A with C gives independent Y discriminators.
These derivations are source-based expectations, not measurements on hardware.

There is a retained scheduler discrepancy: Fullsnes SCREEN.Y starts at 1,
whereas the existing framebuffer row convention passes 0 for its first row.
This Issue uses that existing input convention explicitly and does not resolve
physical scanline origin. Anomie qualifies its bit-accurate formula as tentative;
agreement between documents and bounded software tests is not hardware proof.
No arithmetic candidate was disproved. Earlier identity-only evidence remains
valid within its stated scope and cannot justify the old general transform.

## Implementation, independent vectors and addressing coverage

Production changes are confined to coordinate generation in `Ppu::mode7`.
Existing signed-register consumption, screen-over addressing, raw-byte decode,
BG1 direct color and composition are retained. There is no new public API.

`ppu_mode7_transform.rs` uses the baseline public PPU API. It never calls a
production coordinate helper or computes a second transform at runtime. Its
independently authored literal coordinate pairs position one raw $81 marker
in an otherwise green map. All 16384 map cells select green character 1 except
the expected source cell, which selects character 2; only its selected pixel
is red. Thus a wrong source anywhere else, including another pixel in the same
tile, selects raw $82. Coordinates outside the map are explicitly wrapped only
when drawing the test artwork, not when calculating expected coordinates.
CGRAM indices 1/129 are red and 2/130 green, initialized separately. Both
BG1-only and BG2-only output must reach the same marker. This sparse scene
cannot conceal a disagreement between distant locations with a repeating strip.
Expected RGBA values are literal arrays, including independently authored
$00/$80/$81 transparency and BG1 direct-color controls.

The 15 factored tests cover both equations, both displacement inputs, all six
masked origin products, negative rounding, fractional horizontal steps,
cross terms, signed centers, ignored upper bits and all four flips. Selected
literal derivations (matrix values here are integer register words):

| Inputs | Derived source (X,Y) |
| --- | --- |
| A=32,H=1,x=7; all others zero | (0,0), since 0+224 floors to 0 |
| A=-32,H=1,x=7 | (-2,0), since -64-224=-288 floors to -2 |
| C=32 or -32,H=1,x=7 | (0,0) or (0,-2) independently on Y |
| B=32,V=1,y=7 | (0,0): T(32)+T(224)=192, not 256 |
| B=-32,V=1,y=7 | (-2,0): -64-256=-320 |
| D=32 or -32,V=1,y=7 | (0,0) or (0,-2) independently on Y |
| A=C=16,B=D=32,x=3,y=7 | (0,0): 48+192=240, not 272 |
| A=C=32,x=0/7/8/15/16 | X=Y=0/0/1/1/2 |
| A=128,B=64,C=-64,D=128,H=5,V=-3,CX=2,CY=-2,x=7,y=9 | (9,-1) |
| Same, H flip / V flip / both | (129,-61) / (68,118) / (188,57) |
| A=-96,B=160,C=192,D=-48,H=-4,V=7,CX=-3,CY=4,x=5,y=3 | (-2,5) |

For the final mixed-sign vector, dh=-1,dv=3; X's terms are
64+448+448-768-480=-288 and Y's are -192-192-192+1024+960=1408.
For the flip vector, dh=3,dv=-1: X0=384-64+64*yf+512 and
Y0=-192-128+128*yf-512. Substituting xf=7/248 and yf=9/246 gives
the four literal pairs above. Artwork is asymmetric in both coordinates.
Horizontal A/C products are deliberately unmasked in production as specified.
Pixel tests establish fractional stepping; they cannot distinguish masking just
those products' lowest six bits after all other terms are multiples of 64,
because the final pixel division discards eight bits. No stronger evidence is
claimed for that unobservable intermediate distinction.

At x=y=0 and coefficient 128, the clipped boundary vectors are:

| Offset | Center | Difference | clip | Source (including center) |
| --- | --- | --- | --- | --- |
| 1023 | 0 | 1023 | 1023 | 511 |
| 1024 | 0 | 1024 | 0 | 0 |
| 1025 | 0 | 1025 | 1 | 0 |
| -1023 | 0 | -1023 | -1023 | -512 |
| -1024 | 0 | -1024 | -1024 | -512 |
| -1025 | 0 | -1025 | -1 | -1 |
| 4095 | -4096 | 8191 | 1023 | -3585 |
| -4096 | 4095 | -8191 | -1023 | 3583 |
| 0 | -1023 | 1023 | 1023 | -512 |
| 0 | -1024 | 1024 | 0 | -1024 |
| 0 | -1025 | 1025 | 1 | -1025 |
| 0 | 1023 | -1023 | -1023 | 511 |
| 0 | 1024 | -1024 | -1024 | 512 |
| 0 | 1025 | -1025 | -1 | 1024 |

Each boundary is exercised via H into A/C and via V into B/D; both output
centers take the table's center. The unused matrix terms are zero. Four
upper-bit encodings (0/$2000/$6000/$E000) repeat the signed-center/cross-term
case after preserving each register's low 13 bits.

All four screen-over encodings exercise source -1/0/1023/1024 independently
on each axis, including a negative corner. Zero matrices select literal
centers so those addressing controls do not depend on row origin. Additional
corrected clipped-displacement cases reach (-1,1) and (1,-1) through A and C:
wrapping reaches the red map marker, transparency yields black, and
character-zero fill yields blue at the retained intra-character coordinates
(7,1)/(1,7). Character zero also has different literal colors at (0,0),
(7,0), (0,7) and (7,7), distinguishing fill from both wrapped map and backdrop.
Every case runs with BG1-only and BG2-only routing. Existing #59/#61 suites
continue to cover latch, independent decoding/priority, windows and composition.

The four original CPU-built guest variants use A=32,H=1 or A=128,H=1024,
with BG1-only or BG2-only routing. The CPU writes all map entries, artwork,
palette and registers under forced blank and finally publishes $30=$63.
Native startup is limited to 300000 instructions/eight frames, rejects STP
and APU errors, discards the partial setup frame, then checks three complete
stable 256x224/229376-byte frames per variant. `run_frame` also enforces its
400000-master-clock deadline and propagates CPU/APU errors. Hosts only load,
advance and observe. Browser checks use the identical builder bytes via the
ordinary file picker, a 30-second test deadline and five-second observation
polls. Three observations check actual Wasm bytes and canvas ImageData,
completion, geometry, console/page errors and the UI's core-error field.
The existing mandatory `--grep-invert '301 seconds'` filter includes all four
new tests automatically; no existing filter or assertion was weakened.

See [provenance](../tests/roms/PROVENANCE.md#original-mode-7-fixed-point-transform-guests-approved)
for source paths, original MIT authorship, build commands and all four hashes.
Both independent fixture builds compared byte for byte. Only source is committed.

## Exact-baseline counterexamples

Created a detached worktree at the exact evaluation baseline and copied only
new tests and their original guest builder, leaving production unchanged:

```sh
git worktree add --detach .cache/issue-63/baseline efe3a06dfbcd7aeaaf9b2586369741fdf741b778
cp crates/snes-core/tests/ppu_mode7_transform.rs .cache/issue-63/baseline/crates/snes-core/tests/
cp crates/snes-core/tests/common/mode7_transform.rs .cache/issue-63/baseline/crates/snes-core/tests/common/
cargo test --manifest-path .cache/issue-63/baseline/Cargo.toml --locked -p snes-core --test ppu_mode7_transform
```

Compilation succeeds against the baseline public API. **Twelve tests fail
behaviorally; three controls pass.** Independent first failing pixel assertions:

| Test | Baseline source / observed RGBA | Expected source / RGBA |
| --- | --- | --- |
| fractional X origin | (1,0), green [0,255,0,255] at (7,0) | (0,0), red [255,0,0,255] |
| fractional Y origin | (0,1), green at (7,0) | (0,0), red |
| clipped positive displacement X | (512,0), green at (0,0) | (0,0), red |
| clipped positive displacement Y | (0,512), green at (0,0) | (0,0), red |
| clipped negative displacement X | (-513,0), wraps to (511,0), green | (-1,0), wraps to (1023,0), red |
| clipped negative displacement Y | (0,-513), wraps to (0,511), green | (0,-1), wraps to (0,1023), red |
| B/D origin and row products | (1,0), green at (0,7) | (0,0), red |
| CPU guest | green at display (7,0), fractional BG1 | red |

Other failing tests cover mixed-sign cross terms/centers, boundary vectors,
screen-over interaction and raw decode/direct-color controls at a corrected
source. All failures are pixel mismatches, not compilation, unsupported APIs,
or setup timeouts. Passing controls are identity sampling, ignored upper bits,
and the nonidentity four-flip control (its origin products already align to
64). Negative clipping tests are separate so both candidate counterexamples
are actually executed against the baseline despite the positive failures.
The old source coordinates in the table are arithmetic diagnoses; the recorded
observations are real public-framebuffer pixel failures. No hardware measurement
is implied.

## Mandatory validation (2026-10-24)

Used Node 24.12.0, Python 3.13.13 and the installed Rust 1.99.0 toolchain
(rustc b940084d7, Cargo 5f94df478), matching CI. Selected its bin directory
in PATH and RUSTC to avoid Homebrew compiler precedence. As documented for
Issues #55–#61, an ignored `.cache/issue-63/wasm-ld` wrapper sets
DYLD_LIBRARY_PATH to that toolchain's lib directory before invoking rust-lld;
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER points to it. No production code
or test expectations change for this local tool setup.

Executed gates (logs under ignored `.cache/issue-63/`):

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo test --locked -p snes-core --test ppu_mode7_transform --test ppu_mode7_scroll --test ppu_mode7_extbg --test direct_color --test ppu_pseudo_hires --test ppu_obj_limits
cargo run --locked -p snes-core --example mode7-transform-fixture -- .cache/mode7-transform
cargo run --locked -p snes-core --example mode7-transform-fixture -- .cache/issue-63/rebuilt
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
SNES_TEST_PORT=18085 npm --prefix web test -- --grep 'Mode 7 transform:'
SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'
```

Formatting and warning-denying Clippy pass. **335 workspace tests pass**;
the requested focused command passes **69 tests across six suites**, including
all 15 transform tests. All existing Mode 7, direct-color, pseudo-hires and OBJ
regressions remain. The smoke rebuild compares byte for byte with SHA-256
`73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb`.
All pinned assembler builds pass. Auxiliary SPC/native release builds emit the
previously documented macOS rust-objcopy missing-libLLVM strip warning; builds
and executions still succeed. Hardware gates pass full CPU $0649/PC $0081A2,
basic CPU $0452/PC $0081A2, SPC $0557/PC $008060, and all eleven sixteen-row
HDMA grayscale bands with the final shade through row 223.

Both source-built cleared-game passes retain the expected hashes:
Castle `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`
and Memory `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
Their source pins, licenses, replay inputs/durations, gameplay/audio assertions,
route checks and negative controls are unchanged. npm ci/audit passes with
zero vulnerabilities; the production release Wasm build and Chromium setup pass.
The four new focused browser checks pass in 4.5 seconds including setup.
The native and browser fixtures are independently rebuilt twice and compared
byte for byte, with all four hashes recorded in provenance.

The initial development guest check failed because its builder wrote both
VRAM address bytes to the low address port. Correcting the original builder
to write distinct $2116/$2117 ports resolved that setup bug. It required no
production workaround or weakened expectation. The twelve intentional exact-
baseline failures above remain separate from final validation gates.

The full mandatory Chromium filter passes **33 tests in 2.8 minutes**, with no
skips or failures. It includes both cleared games' five-minute replays,
Castle's complete maps/credits route, Memory's music/effects and gesture-activated
Web Audio, queue/drift/stall checks, freeze negative control, persistence
failure/recovery, hires/SETINI, OBJ sizes/limits, Pages subpath loading, existing
Mode 7 scroll/EXTBG tests and all four new transform checks. No mandatory gate
remains failed or unperformed. The twelve expected baseline failures are
separate from the final passing gates; optional external games were not run.

Staged whitespace, JavaScript syntax, changed Markdown local destinations,
new provenance/specification anchors and balanced fences pass. The inventory's
24 rows/scopes/statuses compare verbatim with Issue #61. Command logs remain
ignored under `.cache/issue-63/`; source-built hardware evidence is in
`.cache/hardware-tests/results.json` and `.cache/ppu-test/results.json`, and
browser evidence is under `web/test-results`. Only source and documentation
are committed.

## Remaining uncertainty and inventory

All 24 inherited categories retain the complete scopes below verbatim from
Issue #61: 21 partial, `dsp-voices` implemented_unverified, and `ppu-raster`/`pal`
unimplemented. No category becomes verified or is promoted. Bounded static
transform coverage refines the outstanding Mode 7 scope without implying full
coverage of every transform interaction.

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

Mode 7 remains partial: mosaic, interlace/overscan, EXTBG with pseudo-hires,
comprehensive window/color-math combinations, multiplication latency/rendering
interference, physical reset retention, raster register changes and pixel-fetch/
arbitration timing remain. The physical scanline-origin discrepancy is retained
explicitly. Static software tests do not establish physical arithmetic hardware
behavior; no measurements were made on a console. Complete transform interactions
with these remaining modes are outside this bounded implementation. All other
inventory limitations and complete feature scopes above remain in force.

No commercial ROM, firmware or third-party emulator implementation was acquired.
Passing these bounded checks establishes neither complete hardware accuracy nor
commercial-game compatibility and does not complete Issue #3. Optional external
game cases are excluded by the required filter; they were not acquired or run.
No post-merge verification is required.
