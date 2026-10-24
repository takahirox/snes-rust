# snes-rust

An experimental SNES interpreter written from scratch in Rust, with a dependency-free
WebAssembly adapter and a browser frontend. No existing emulator core,
Nintendo firmware, commercial ROM, or third-party game ROM is included.

The first-playable baseline runs audited, source-built modified versions of
**Castle Platformer** and **Memory Game** through the same generic core. Both
have five-minute controller replays; Castle also has a complete five-map/credits
route. Memory plays original game-generated music and finite sound effects.
Their exact sources, assets, notices, alterations and ROM hashes are in the
[cleared-game audit](tests/roms/CLEARED-GAMES.md).

Issue [#3](https://github.com/takahirox/snes-rust/issues/3) was closed as a tracking
handoff to [#5](https://github.com/takahirox/snes-rust/issues/5). The
[delivery checklist](docs/issue-3-progress.md) maps its requirements to current
code and executable evidence; [final validation](docs/issue-5-validation.md)
distinguishes the verified target behavior from optional hardware accuracy.
This is a first-playable baseline for these artifacts, not broad commercial
compatibility. Gothicvania and the original Castle release are optional,
user-supplied investigations with unresolved binary/build licensing.

## Run the browser app

Public site: <https://takahirox.github.io/snes-rust/> (deployment and live-site
checks passed on 2026-10-22; see the [Pages verification record](docs/issue-9-validation.md)).
Visitors select their own legally obtained ROM locally. ROMs are never uploaded
or downloaded by the app, and browser SRAM saves remain in local storage.

Requirements: Rust 1.94+ with Cargo, the `wasm32-unknown-unknown` standard library,
Node.js 24+, npm, and Python 3.12+. Only browser tests use an npm dependency
(Playwright); the Rust workspace has no external dependencies.

```sh
rustup target add wasm32-unknown-unknown
npm --prefix web ci
npm --prefix web run build
npm --prefix web run serve
```

Open <http://127.0.0.1:8080>. Select a lawfully obtained ordinary NTSC LoROM or
HiROM `.sfc`/`.smc` cartridge, then click **Play**. Files stay client-side. No server
upload or remote ROM download is implemented. The static production output is
`web/dist`; the local server serves the same assets as GitHub Pages.

Keyboard: arrows move, Z = B, X = A, A = Y, S = X, Q/W = L/R, Enter = Start,
Shift = Select. Standard gamepads use the corresponding four face buttons,
shoulders, Start/Select, D-pad and left stick. Audio activates on Play's user
gesture; Sound toggles playback. Pause stops queued audio. A hidden tab pauses.
The canvas supports integer scaling and preserves pixel edges.

SRAM for battery-backed cartridges is stored locally under the SHA-256 of the
ROM after removing any copier header. Reset clears volatile machine state and
controllers, preserving SRAM. Opening a ROM resets it and restores its own saved
SRAM. Saves are persisted every five seconds, on pause/reset/ROM changes and on
page exit; browser storage failure is reported. A failed save blocks opening
another cartridge, preserving the current machine's recoverable SRAM. Play and
Reset retain the warning until a later save succeeds. Keep the tab open while
storage is unavailable. A failed save restoration blocks saving, Play, Reset and
cartridge replacement so existing persisted bytes cannot be overwritten by empty
SRAM. Use **Retry restoring save** after restoring storage access (or repairing
corrupt stored data); recovery must succeed before playing. Clearing browser
storage erases persisted saves. There are no save states or save-file import/export
controls yet.

If Homebrew Cargo picks a different `rustc` from rustup on macOS, select the
compiler that owns the installed wasm standard library:

```sh
RUSTC="$(rustup which rustc)" npm --prefix web run build
```

## Validate

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo test --locked -p snes-core --lib machine::dma_tests
cargo test --locked -p snes-core --test dma_access
cargo test --locked -p snes-core --test hdma_init
cargo test --locked -p snes-core --test hdma_reload
cargo test --locked -p snes-core --test hdma_phase
cargo test --locked -p snes-core --test hdma_ntrl
cargo test --locked -p snes-core --test ppu_counter_latch
cargo test --locked -p snes-core --test ppu_vram_access
cargo test --locked -p snes-core --test ppu_oam_access
cargo test --locked -p snes-core --test ppu_oam_reload
cargo test --locked -p snes-core --test ppu_cgram_access
cargo test -p snes-core --test dsp_noise --locked
cargo test -p snes-core --test dsp_pitch --locked
cargo test -p snes-core --test ppu_offset --locked
cargo test --locked -p snes-core --test ppu_hires
cargo test --locked -p snes-core --test ppu_mode6_offset
cargo test --locked -p snes-core --test ppu_pseudo_hires
cargo test --locked -p snes-core --test ppu_obj_sizes
cargo test --locked -p snes-core --test ppu_obj_limits
cargo test --locked -p snes-core --test ppu_mode7_scroll
cargo test --locked -p snes-core --test ppu_mode7_extbg
# Original CPU-built browser/runner fixtures (also built by browser tests).
cargo run --locked -p snes-core --example hires-fixture -- .cache/hires
cargo run --locked -p snes-core --example pseudo-hires-fixture -- .cache/pseudo-hires
cargo run --locked -p snes-core --example obj-sizes-fixture -- .cache/obj-sizes
cargo run --locked -p snes-core --example obj-limits-fixture -- .cache/obj-limits
cargo run --locked -p snes-core --example mode7-scroll-fixture -- .cache/mode7-scroll
cargo run --locked -p snes-core --example mode7-extbg-fixture -- .cache/mode7-extbg
python3 tools/make-fixture.py .cache/smoke.bin
cmp tests/roms/smoke.bin .cache/smoke.bin
# Requires make and C/C++ compilers; builds local hash-pinned assemblers.
python3 tools/build-test-tools.py
python3 tools/build-cleared-games.py
python3 tools/build-cleared-games.py
npm --prefix web run build
cd web
npx playwright install chromium
npm test -- --grep-invert '301 seconds'
# If port 8080 is occupied, prefix the test command with SNES_TEST_PORT=18085.
```

For Issue #25, all 29 focused DMA/HDMA unit tests, four CPU guests, 143 workspace
tests and 18 browser checks passed, together with the source-built hardware gates.
See the [validation evidence](docs/issue-25-validation.md) for exact commands,
baseline failures, compiler selection, game replay results and remaining limits.
Issue #27 extends suppression to the WRAM address registers; its
[validation record](docs/issue-27-validation.md) records the new baseline failures,
38 focused unit tests, five CPU guests and the final mandatory gate results.
Issue #29 adds direction-specific reverse WMDATA conflict handling; its
[validation record](docs/issue-29-validation.md) separates supported write/pointer
effects from unverified electrical byte values, with 48 focused tests and six
CPU guests. Issue #31 adds separate scanline-0 initial descriptor/indirect-pointer
prefetch, natural scheduler vectors and two CPU guests; its
[validation record](docs/issue-31-validation.md) distinguishes supported ordering
and register effects from approximate clock alignment. Issue #33 adds subsequent
row-end metadata reload, ordered payload/reload phases and terminating indirect
reads; its [validation record](docs/issue-33-validation.md) records natural
scheduler regressions, two original CPU guests and the corrected older vectors.
Issue #35 separates HBlank/rendering from scanline HDMA service eligibility; its
[phase validation](docs/issue-35-validation.md) records reference terminology,
numeric boundaries, baseline failures, natural scheduler vectors and two CPU guests.
Issue #37 adds live CPU writes to the HDMA line counter, preserving the prepared
transfer decision; its [validation record](docs/issue-37-validation.md) derives
live zero/repeat behavior and records boundary vectors and two original CPU guests.
Issue #39 connects WRIO bit 7 to PPU counter capture and STAT78 acknowledgement;
its [counter-latch validation](docs/issue-39-validation.md) records source qualifications,
64 edge/mirror vectors, scanline-crossing accesses, B-bus controls and an original CPU guest.

The checked-in diagnostic ROM is original MIT code, pixels and sample data,
reproducible without an assembler. It tests actual CPU/PPU/DMA/joypad/SPC/DSP
execution and browser controls, audible buffers, pause/reset and save restoration.
It is **not a game** and does not substitute for the two game targets.

The cleared Castle and Memory builds are mandatory browser checks. Their sources
are downloaded only from the allowlist, then converted and assembled locally;
their ROMs are never downloaded. Repeating the build verifies the expected hashes
from fresh build directories. Chromium runs 18,100 frames (301.17 emulated seconds)
per game, asserting gameplay throughout the run. Memory also wins and restarts
using controller input, requiring independently non-silent music and game effects
in every window.
An idle-gameplay test recovers the looping score's notes, rests and durations
from rendered audio, checks a finite card effect while music continues, and
emits JSON plus a listenable stereo WAV in `web/test-results`. A separate real-time
keyboard test verifies music and card effects reach Web Audio after clicking
Play, the effect ends while music continues, and reset returns to a silent title.
A separate fixed-input Castle replay traverses all five maps from reset and
finishes the credits scroll. It verifies switch-modified map tiles, ordered map
transitions, controller-driven traversal, changing video and framebuffer hashes
at recorded checkpoints, rejecting any player death. Castle boots directly into
gameplay; it has no title screen. The input recording is
[`web/tests/castle-route.json`](web/tests/castle-route.json).
CI performs these builds and runs. Generated images and
their accompanying notices stay under `.cache/cleared-games`; select them with
the ordinary ROM picker to play locally. See the audit for coverage and limits.

The licensed CPU/SPC and HDMA suites build from hash-pinned source. Their fonts
are replaced with zero bitmaps; native assertions inspect RAM, terminal PCs and
rendered grayscale bands. Do not judge their blank font screens as a failure.
Build-only assemblers are isolated in `.cache/tools`. C/C++ compilers and make
are needed. The optional SPC assembler needs Rust **1.99+** and experimental
compiler features; the script enables those only for that separate build tool.
No emulation component from that tool's repository is built or used.

```sh
python3 tools/build-test-tools.py --spc
python3 tools/hardware-tests.py \
  --ca65 "$PWD/.cache/tools/cc65/bin/ca65" \
  --ld65 "$PWD/.cache/tools/cc65/bin/ld65" \
  --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"
python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"
```

On the mixed macOS installation described above, prefix the tool-build command
with `RUSTC="$(rustup which rustc)"` to select Rust 1.99. Omit `--spc` and
`--spcasm ...` to run only the 65C816 suites. Results and hashes are written to
`.cache/hardware-tests/results.json` and `.cache/ppu-test/results.json`.
[Provenance and notices](tests/roms/PROVENANCE.md) describe the precise source
artifacts, compiler pins and deliberate fixture alterations.

The Pages smoke test runs as part of the browser suite, or by itself after a
production build and Chromium installation:

```sh
npm --prefix web test -- tests/pages.spec.js
```

It inspects the actual `web/dist`, serves it strictly under `/snes-rust/`, checks
HTML/CSS/JavaScript/Wasm responses and real Wasm initialization, and plays the
checked-in MIT diagnostic through local file selection. It rejects unexpected
files, symlinks, root-path or remote requests, HTTP failures and browser/CSP
errors. The diagnostic and test evidence are outside the public artifact.

## GitHub Pages deployment

The repository's Pages source is **GitHub Actions** (`build_type: workflow`),
enabled and verified on 2026-10-22 for [Issue #9](https://github.com/takahirox/snes-rust/issues/9).
For a new repository, a maintainer must first open **Settings → Pages → Build
and deployment → Source** and select **GitHub Actions**. No PAT or additional
credentials are needed by the workflow. If the `github-pages` environment has
deployment restrictions, allow `main`.

Every push to `main` runs [Deploy GitHub Pages](.github/workflows/pages.yml).
It calls the existing [Emulator checks](.github/workflows/ci.yml) with pinned Rust
1.99.0, the `wasm32-unknown-unknown` target, Node 24 and Python 3.13. The browser
job builds with `npm --prefix web run build` and uploads only `web/dist` after
its browser checks (including the subpath smoke test) pass. Deployment waits for
both core and browser jobs to pass. Pull requests and other branch pushes run
checks separately, without production deployment permissions. Production runs
share a concurrency group to prevent overlapping deployments.

The Pages artifact contains exactly `index.html`, `app.js`, `style.css` and
`snes_wasm.wasm`. Rebuilding clears stale output, and the smoke test enforces this
allowlist before upload. Relative URLs, including `fetch('snes_wasm.wasm')`, work
under the project subpath. Generated `web/dist` stays ignored by Git. No ROMs,
firmware, `.srm` saves, source/download caches, test evidence or game artifacts
are published. Users supply lawful ROMs locally; saves on the public origin are
separate from saves on localhost and remain in that browser's local storage.

Inspect the repository's [Actions tab](https://github.com/takahirox/snes-rust/actions)
for the deployment run, failed job/step logs and the `github-pages` artifact;
the `github-pages` environment and deployment job show the deployed URL. Fix
failed checks before expecting publication; setup errors may require the Pages
source selection above. Later `main` pushes rebuild and republish automatically,
without committing generated files.

**Post-merge verification completed:** the first `main` deployment initially
failed because Pages was disabled. Issue #9 enabled the Actions source and reran
the deployment successfully, then verified the public app, asset hashes and MIME
types, Wasm initialization and local diagnostic playback. See the
[run](https://github.com/takahirox/snes-rust/actions/runs/39764299327/attempts/2)
and [verification evidence](docs/issue-9-validation.md). A later `main` push has
not been exercised separately.

For future deployment changes, the PR author/merger checks the resulting `main`
deployment run and opens its reported HTTPS URL. Record the run URL,
live URL and actual result: assets load without 404/CSP/MIME errors, Wasm
initializes, and the local ROM chooser is enabled. Where possible select the
cleared diagnostic to check input/video without uploading a ROM. Confirm later
`main` pushes can republish. A passing local build does not establish publication,
and this check cannot block pre-merge approval. If setup or
the live site fails after merge, create a new Issue with the `task` label linking
[#7](https://github.com/takahirox/snes-rust/issues/7), the run, failing URL,
reproduction, actual/expected behavior and logs; fix it through the normal
Issue → PR flow without reopening #7. An agent can perform that follow-up only
when invoked/authorized after push; no background monitoring is implied.

## Optional game validation

For optional long game replays, supply the exact local images identified there:

```sh
GOTHICVANIA_ROM=/absolute/path/gothicvania.sfc \
CASTLE_ROM='/absolute/path/Castle Platformer (v1.04).sfc' \
npm --prefix web test -- --grep '301 seconds'
```

These tests verify ROM hashes and run 18,100 frames per game (301.17 emulated
seconds) from reset. Every 900-frame window requires controller-driven player
movement, a grounded B jump, active gameplay, changing video and (for Gothicvania)
non-silent audio. Source-derived game-state markers verify title/play/game-over
and death/restart transitions. Two negative controls freeze the observed player
position after the first valid window and require the next window to fail.
JSON evidence includes every window and active-frame counts in `web/test-results`.

After 900 startup frames, the tests bound each audio buffer and cumulative sample
count against the core's NTSC raster to 128 interleaved samples (2 ms). This
checks internal cadence; it does not establish hardware timing, audible fidelity,
interactive playback synchronization or a complete level traversal. Samples
include death/title pauses and restarts. Tests are skipped unless their ROM path
is set; CI does not acquire these incompletely audited games.

## Headless debugging

```sh
cargo run --release -p snes-core --bin snes-run -- tests/roms/smoke.bin \
  --frames 120 --timeout 15 --trace 16 --dump 0x10,16 \
  --expect 0x11=0 --ppm .cache/frame.ppm
```

`--replay FILE` accepts strictly increasing `frame,buttons` CSV rows, with decimal
or `0x` hexadecimal numbers and `#` comments. The last mask remains held until
another row. SNES button bits are B/Y/Select/Start/Up/Down/Left/Right/A/X/L/R at
bits 15..4. `--expect ADDRESS=BYTE` may be repeated; `--expect-pc ADDRESS` checks a
terminal 24-bit PC; `--sram FILE` loads/saves raw cartridge RAM. All expectations
fail the process on mismatch. `--dump ADDRESS,LENGTH` prints at most 256 bytes
without I/O side effects. `--trace N` retains at most 1,000 CPU and SPC states.
The runner emits JSON lines with FNV-1a frame digests and audio energy/counts;
these hashes are regression checks, not cryptographic integrity checks.
Frames and wall-clock limits are explicit. STP and core errors fail with CPU
context; a program legitimately waiting forever is bounded by the frame limit.

## Implemented features and limits

- Ordinary LoROM/HiROM, copier-header removal, header validation, reset vectors,
  WRAM/SRAM mirrors, open bus, multiply/divide I/O and region-specific bus waits.
- All 256 65C816 opcode decodings; native/emulation and register-width modes,
  decimal arithmetic, stack edge cases, software interrupts, NMI/IRQ, WAI/STP.
  Licensed full/basic semantic suites pass. Independent bus-order/count tests
  cover addressing, stacks, interrupts and RMW writes. CPU-cycle multiply/divide
  latency and a 40-master-clock scanline refresh hold are implemented. This is
  not a claim of every instruction/interrupt edge on every CPU revision.
- A master-clock scheduler, NTSC scanlines, VBlank/HBlank, NMI, timer IRQs,
  serial/automatic joypad reads with busy/completion behavior, general DMA and
  direct/indirect HDMA. HDMA preempts DMA between bytes; all devices advance
  during transfers. Tests check DMA writes across the VBlank access boundary,
  deferred NMI, IRQ comparisons and guest-generated HDMA raster bands.
  DMA/HDMA A-bus reads of `$4210–$421F` in system-I/O banks reach the existing
  status/result handlers, including read-clear NMI/IRQ acknowledgement for
  payloads, HDMA descriptors and indirect pointers. Writes remain excluded.
  Other established A-bus exclusions retain zero-read/discard-write
  approximations; zero and floating-bit values are not electrical hardware claims.
  Legal B-bus and CPU accesses retain their behavior. The
  [corrected A-bus validation](docs/issue-23-validation.md) supersedes Issue #21's
  blanket CPU-register read exclusion and records reference sections, register
  vectors, baseline failures and an original CPU-executed guest. Restrictions
  elsewhere in `$4200–$420F` remain unresolved where unsupported by references.
  Forward DMA/direct HDMA/indirect HDMA payloads from WRAM to B-bus `$2180–$2183`
  suppress the write, preserving all 17 pointer bits (including no `$2180`
  increment), while retaining source reads, timing and byte/row progression.
  Full WRAM banks and system-bank mirrors are checked per payload byte using the
  effective B register; ROM/SRAM transfers, CPU port accesses and table fetches
  remain legal. The [WMDATA validation](docs/issue-25-validation.md) and
  [address-register validation](docs/issue-27-validation.md) record specification
  sections, the HDMA derivation, baseline failures, original vectors and CPU guests.
  Reverse DMA/direct HDMA/indirect HDMA from `$2180` into a current WRAM
  destination retains the A-bus write and all 17 WMDATA pointer bits. The written
  byte uses the previous emulator bus byte as a deterministic approximation;
  exact electrical values remain unverified. The
  [reverse-conflict validation](docs/issue-29-validation.md) records the
  direction-specific evidence and separates approximation tests from hardware
  side-effect assertions.
  Enabled HDMA channels prefetch their initial descriptor and indirect address
  at the modeled scanline-0 initialization event, in both the initial frame and
  subsequent frames. The first scanline HDMA event uses that metadata and reads
  live payload bytes. Zero initial headers terminate internally while preserving
  `$420C`, including the indirect-pointer read effects described in the
  [prefetch validation](docs/issue-31-validation.md). Writing `$420C` does not
  trigger initialization. Subsequent rows reload descriptors and indirect
  pointers in the HBlank ending the preceding row, after all eligible channel
  payloads. Terminating indirect reads publish their pointer/table effects,
  counting later enabled, unterminated channels; `$420C` stays programmed.
  Prepared metadata survives later table edits while payload bytes stay live.
  The [row-end validation](docs/issue-33-validation.md) documents the ordered
  algorithm and the last-channel zero-header derivation. CPU writes to `$43xA`
  update all eight live counter bits while preserving the prepared transfer/hold
  decision for the next event. The subsequent decrement uses the written value,
  then evaluates repeat and exhaustion. A live `$00` wraps to `$FF`; only a
  fetched zero descriptor terminates. Frame initialization still replaces
  pre-initialization writes with table metadata. The
  [live-counter validation](docs/issue-37-validation.md) distinguishes these
  derived effects from descriptor-fetch behavior. `dma` and `hdma` remain partial:
  startup/alignment and full overhead timing, arbitrary mid-frame enabling, same-channel contention, exact electrical open-bus behavior and the
  complete mode-interaction matrix remain separate work. HBlank status and
  whole-line rendering stay at 1096 master clocks; independent scanline HDMA
  eligibility is at 1112 (H=278), serviced at the next CPU/DMA bus boundary.
  This maps the documented activity phase to service eligibility without
  asserting an earlier hardware request edge or exact first-byte alignment.
  Initialization and byte timing retain their approximate scheduler conventions;
  the [phase record](docs/issue-35-validation.md) separates these from source facts.
- Planar BG modes, tilemaps, scroll/mosaic, sprites, priorities, windows and
  color math, plus a basic Mode 7 transform. BG1 direct color is supported in
  Modes 3/4 and Mode 7 (including EXTBG), with tile palette contributions
  in Modes 3/4, transparency and the existing composition pipeline. See the
  [direct-color validation](docs/issue-11-validation.md) for numerical, rendering
  and original CPU-executed guest coverage. Non-mosaic Modes 2/4 support BG3
  offset-per-tile addressing for BG1/BG2, including independent enable bits,
  fine-scroll preservation, BG3 screen layouts and 8×8/16×16 tile addressing.
  The [offset validation](docs/issue-19-validation.md) records coordinate vectors,
  original CPU-executed guests and baseline failures. `ppu-offset` and `ppu-bg`
  remain partial. Non-mosaic Mode 5 BG1/BG2 and Mode 6 BG1 now use rectangular
  16×8/16×16 hires cells, full-dot horizontal scroll and independent even-sub /
  odd-main output samples. Ordinary non-interlaced hires frames expose native
  512×224 packed RGBA; ordinary low-resolution frames retain 256×224. Core/Wasm
  geometry and byte length drive browser drawing and runner PPM output. Display
  shape and scale controls continue to count 256 full dots horizontally.
  A frame containing hires rows is promoted to 512 samples, duplicating earlier
  low-resolution dots and subsequent low-resolution rows; it shrinks only at
  the next low-resolution row zero. This safe storage policy does not verify
  physical switching/fetch timing. The [hires validation](docs/issue-49-validation.md)
  records specifications, equations, original CPU guests and transition checks.
  `ppu-hires` is partial; `ppu-raster` remains unimplemented.
  Non-mosaic Mode 6 BG1 also supports independent H/V offsets from raw BG3
  descriptors, with full-dot columns and retained fine scroll. Its
  [offset validation](docs/issue-51-validation.md) records source-derived units,
  numerical boundaries, original CPU guests, baseline failures and gate results.
  SETINI bit 3 also enables 512-sample pseudo-hires in low-resolution modes:
  each full dot supplies even-sub / odd-main phases without changing BG tile,
  scroll, offset or Mode 7 coordinates. Tested support is non-interlaced,
  non-mosaic, EXTBG disabled, CGADSUB=$00 and CGWSEL=$00 (or $02), with
  independent TM/TS, TMW/TSW layer windows, CGRAM[0] for both displayed backdrops,
  brightness and forced blank. The same packed-frame promotion policy applies.
  The [pseudo-hires validation](docs/issue-53-validation.md) records literal
  pixels, CPU-executed guests, baseline counterexamples and Wasm/canvas checks.
  Static non-interlaced OBJs use independent width/height for all eight OBSEL
  selectors, including 16x32 and 32x64 rectangles. Vertical flips reverse rows
  within each stacked square half; character increments retain independent
  nibble wrapping and name-table selection. The
  [OBJ size validation](docs/issue-55-validation.md) records source sections,
  coordinate derivations, independent pixel crops, exact-baseline failures,
  original CPU-built cartridges and actual Wasm/canvas checks. Non-interlaced
  OBJ evaluation selects 32 objects and admits 34 eight-pixel tiles in reverse
  object order, with clipped-tile accounting, X=-256's counting exception and
  stable programmed priority rotation. STAT77 overflow flags accumulate until
  the frame boundary, where forced blank inhibits clearing. The
  [OBJ limit validation](docs/issue-57-validation.md) records the specification
  contract, independent survivor vectors, CPU status/lifetime guests and actual
  Wasm/canvas checks. `ppu-objects` remains partial: physical evaluation/fetch
  timing, rotation oddities, interlace and hires interactions remain outstanding.
  Mode 7 captures independent signed 13-bit H/V offsets through the same
  previous-byte latch as its matrix/center registers, while the BG1 ports also
  update ordinary planar scroll through its separate latch. The
  [Mode 7 scroll validation](docs/issue-59-validation.md) records interleaved
  writes, multiplication, identity pixel vectors and an original CPU guest
  observed natively and through actual Wasm/canvas output. Static, non-mosaic,
  non-interlaced 256-pixel Mode 7 EXTBG also supports BG2's seven-bit palette
  index, raw-index-zero transparency, per-pixel priority against all OBJ priorities,
  independent screen/window routing and bounded layer-selected color math.
  BG1 retains eight-bit direct color with EXTBG; BG2 remains paletted. The
  [EXTBG validation](docs/issue-61-validation.md) reconciles the specifications,
  corrects the old BG1 assertion and records original CPU-built native/browser
  evidence and exact-baseline failures. `ppu-mode7` remains partial:
  nonidentity arithmetic/truncation, signed-coordinate and screen-over edges,
  mosaic, interlace/overscan, pseudo-hires composition, multiplication timing/
  interference, physical reset retention and fetch/raster interactions remain.
  Rendering snapshots registers per scanline. Math-enabled pseudo-hires,
  color-window clipping/prevention interactions, ordinary-hires composition,
  hires mosaic/color-math/window/OBJ accuracy,
  mosaic/offset interactions, timed mosaic state, mid-scanline raster effects,
  interlace/overscan/PAL, physical object overflow timing/rotation oddities and
  Mode 7 transform edge behavior and EXTBG interactions beyond the static scope
  above are outside the tested targets.
  `ppu-color` remains partial:
  comprehensive window logic, clipping/prevention interactions, subtraction/
  saturation, halving and brightness coverage still need work. VRAM writes obey
  forced/VBlank access and still increment when blocked. VRAM read-buffer fetches
  likewise require forced blank or VBlank, including address-write prefetch;
  HBlank and scanline 0 do not grant access. Reads return the old buffered byte,
  and selected-port reads attempt refill before incrementing. The increment
  still occurs when access is blocked.
  Unavailable fetches retain the buffer as a deterministic approximation; exact
  invalid-fetch bits and physical access/blanking-transition timing remain
  unverified. The [VRAM validation](docs/issue-41-validation.md) records source
  qualifications, the operation/state matrix, CPU/DMA/HDMA vectors and original
  CPU guests. `ppu-ports` remains partial. VRAM prefetch,
  OAM/CGRAM write pairs, latched counters and separate PPU read buses have
  numerical regressions. WRIO starts at `$FF`; bit 7 gates SLHV and falling
  edges capture counters. STAT78 resets both read selectors and acknowledges
  the latch flag only with the gate high. `Machine::reset` uses the same initial
  state; physical warm-reset retention remains unverified.
  OAM ports distinguish software access in forced/VBlank from rendering-owned
  addressing during active display, HBlank and scanline 0. Either address write
  reloads the whole programmed word and even phase. VBlank access begins at
  V=225/H=0; the distinct automatic reload occurs at H=10 dots (40 master
  clocks) when forced blank is clear. Qualified first-VBlank INIDISP writes
  separately restore that address. The [reload validation](docs/issue-45-validation.md)
  supersedes the earlier H=0 reload convention, with numerical CPU/DMA vectors
  and an original executed guest. Latch retention and coincident-access ordering
  remain model policy/assumptions; `scheduler` and `ppu-ports` remain partial.
  Even high-table writes also capture the shared low-byte latch. Without an object-fetch
  pipeline, rendering uses a deterministic two-byte high-table substitute;
  its destinations, returned bytes and preserved latch/phase are approximation
  policy, not hardware-verified corruption. The [OAM validation](docs/issue-43-validation.md)
  records the source disagreement about forced-blank exit and the selected rule.
  CGRAM data ports now distinguish software-addressed forced/VBlank and HBlank
  transfers from rendering-owned accesses. Their shared byte selector, staged
  low-byte latch, paired seven-bit-high commits and PPU2 read-bus bit are retained
  across CPU/DMA/HDMA paths. Without a pixel-fetch pipeline, active accesses use
  color 0 as a deterministic substitute. The entire HBlank access window,
  scanline-0 ownership and retention of phase/latch across ownership changes
  remain model policy. The [CGRAM validation](docs/issue-47-validation.md) records
  the source conflict, operation/state table, independent vectors, original CPU
  guest and baseline counterexamples. `ppu-ports` remains partial: exact
  rendering-owned OAM/CGRAM corruption, unresolved phase/latch retention,
  physical access/blanking latency and dot-level arbitration remain outstanding.
  The selected games update video memory during forced/VBlank.
- SPC700 execution/timers and CPU communication using a firmware-free,
  high-level IPL upload protocol. All licensed SPC semantic tests pass; IPL
  instruction timing and general firmware execution are not emulated.
  DSP provides BRR voices, Gaussian interpolation, ADSR/gain, signed stereo
  mixing, five-sample key-on delay and FIR echo. Numerical tests cover the
  selected game's filter-0 BRR, interpolation/overflow, direct gain, pitch,
  release/retrigger and deterministic 32 kHz output. ADSR and dynamic GAIN use
  one cyclic 30,720-sample rate counter with rate-specific phase offsets. The
  [envelope validation](docs/issue-13-validation.md) covers every rate over two
  periods, selected envelope boundaries, an uploaded original SPC700 guest and
  APU partition invariance. `dsp-envelope` remains partial: exhaustive envelope
  transitions, hidden-envelope/bent-GAIN behavior, live register interactions
  and DSP subcycle sampling remain unverified beyond those tests. One shared
  15-bit noise generator uses that same counter and FLG rate, with per-voice
  NON source selection before envelope scaling; BRR/pitch/ENDX continue while
  noise is selected. The [noise validation](docs/issue-15-validation.md) covers
  all rates, shared routing, signed PCM/OUTX, continuity, an original uploaded
  SPC700 guest and APU partition invariance. Noise remains partial: subcycle
  NON/FLG sampling and register-write races remain unverified. Sample-level PMON
  routes each preceding voice's full-resolution post-envelope output, including
  noise, through signed pitch arithmetic and bounded interpolation advancement.
  The [pitch validation](docs/issue-17-validation.md) covers routing, rounding,
  high increments/saturation, live writes, noise interactions, an original SPC
  guest and APU partition invariance. Pitch remains partial: DSP subcycle register
  sampling, torn pitch writes, SPC bus timing and register/RAM races are outside
  this implementation. Exhaustive BRR filters/interpolation phases, hidden
  envelope corrections and reference echo saturation remain unverified.
  Browser tests bound device-clock drift, queue duration and live sources,
  including main-thread stalls and repeated pause/reset/play.
- Enhancement chips, ExLoROM/ExHiROM, PAL, save states and broad commercial
  compatibility are unsupported. Unsupported cartridge hardware/region errors
  are explicit. High-resolution ROM behavior may render incorrectly rather
  than being rejected.

## Contributing

Follow the [development flow](docs/development-flow.md),
[review guidelines](docs/review-guidelines.md), and [agent guidance](AGENTS.md).
