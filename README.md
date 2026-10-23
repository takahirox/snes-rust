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
cargo test -p snes-core --test dsp_noise --locked
cargo test -p snes-core --test dsp_pitch --locked
cargo test -p snes-core --test ppu_offset --locked
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
  `dma` and `hdma` remain partial:
  WRAM-to-`$2180` conflicts, startup/alignment timing, HDMA initialization/
  overhead and first-frame behavior, same-channel contention, exact electrical
  open-bus behavior and the complete mode-interaction matrix remain separate work.
- Planar BG modes, tilemaps, scroll/mosaic, sprites, priorities, windows and
  color math, plus a basic Mode 7 transform. BG1 direct color is supported in
  Modes 3/4 and Mode 7 with EXTBG disabled, including tile palette contributions
  in Modes 3/4, transparency and the existing composition pipeline. See the
  [direct-color validation](docs/issue-11-validation.md) for numerical, rendering
  and original CPU-executed guest coverage. Non-mosaic Modes 2/4 support BG3
  offset-per-tile addressing for BG1/BG2, including independent enable bits,
  fine-scroll preservation, BG3 screen layouts and 8×8/16×16 tile addressing.
  The [offset validation](docs/issue-19-validation.md) records coordinate vectors,
  original CPU-executed guests and baseline failures. `ppu-offset` and `ppu-bg`
  remain partial; `ppu-hires` and `ppu-raster` are not completed by this change.
  Rendering snapshots registers per scanline. Mode 6 offset/hires addressing,
  mosaic/offset interactions, timed mosaic state, mid-scanline raster effects,
  hires/interlace/overscan, exact object overflow/rotation and Mode 7 EXTBG/transform
  edge behavior are outside the tested targets. `ppu-color` remains partial:
  comprehensive window logic, clipping/prevention interactions, subtraction/
  saturation, halving and brightness coverage still need work. VRAM writes obey
  forced/VBlank access and still increment when blocked. VRAM prefetch,
  OAM/CGRAM write pairs, latched counters and separate PPU read buses have
  numerical regressions.
  Active-display OAM/CGRAM corruption and dot-level pixel fetches are not modeled;
  the selected games update video memory during forced/VBlank.
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
