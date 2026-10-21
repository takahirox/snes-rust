# snes-rust

An experimental SNES interpreter written from scratch in Rust, with a dependency-free
WebAssembly adapter and a local browser frontend. No existing emulator core,
Nintendo firmware, commercial ROM, or third-party game ROM is included.

**Issue [#3](https://github.com/takahirox/snes-rust/issues/3) remains in progress.**
Audited, source-built modified versions of Castle Platformer and Memory Game run
through the same core in five-minute browser regressions. Their exact sources,
assets, notices, alterations and ROM hashes are recorded in the
[cleared-game audit](tests/roms/CLEARED-GAMES.md). The modified Memory Game adds
original looping gameplay music and card/match/win/loss sound effects through
an uploaded SPC program; Castle remains silent. Hardware and audio fidelity verification remain release
requirements. Gothicvania and the original Castle release remain
user-supplied investigations with unresolved binary/build licensing. This is not
a completed release or a claim of broad commercial compatibility. See the
[delivery checklist and evidence](docs/issue-3-progress.md).

## Run the browser app

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
`web/dist`; no deployment is required.

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
  The full/basic licensed semantic suites pass. Internal instruction cycles,
  interrupt latency, DRAM refresh, multiplication/division delays, IRQ edges,
  DMA/HDMA conflicts and bus access ordering are **not hardware accurate**.
- A master-clock scheduler, NTSC scanlines, VBlank/HBlank, NMI, timer IRQs,
  serial/automatic joypad reads, general DMA and direct/indirect HDMA.
- Planar BG modes, tilemaps, scroll/mosaic, sprites, priorities, windows and
  color math, plus a basic Mode 7 transform. Rendering snapshots registers per
  scanline. Mid-scanline raster effects, offset-per-tile modes 2/4/6, hires/
  interlace/overscan, direct color, exact object overflow/rotation and Mode 7
  edge behavior are incomplete or unverified. VRAM/OAM access restrictions,
  register read latches and PPU open bus are simplified.
- SPC700 execution/timers and CPU communication using a firmware-free,
  high-level IPL upload protocol. All licensed SPC semantic tests pass; IPL
  instruction timing and general firmware execution are not emulated.
  DSP provides BRR voices, simplified ADSR/gain, stereo mixing and FIR echo.
  It uses linear interpolation; Gaussian interpolation, noise, pitch modulation,
  exact envelope/key-on delays, echo saturation and sample-clock fidelity remain
  incomplete. Non-silent audio does not prove reference audio accuracy.
- Enhancement chips, ExLoROM/ExHiROM, PAL, save states and broad commercial
  compatibility are unsupported. Unsupported cartridge hardware/region errors
  are explicit. High-resolution ROM behavior may render incorrectly rather
  than being rejected.

## Contributing

Follow the [development flow](docs/development-flow.md),
[review guidelines](docs/review-guidelines.md), and [agent guidance](AGENTS.md).
