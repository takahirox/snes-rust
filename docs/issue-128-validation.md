# Issue #128: cartridge prompts and Reset playback

The persistent selector now says **Insert cartridge**, and the empty screen says
**Insert a cartridge** beside the existing **Inserted cartridge / None** field.
The selector, popover, file picker, notices and layout retain their existing
structure. README usage instructions match the new action label.

Reset uses the existing `pause()` → `core.reset()` → `draw()` → `start()` path.
Pausing releases held keyboard/touch input, stops queued audio and attempts to
persist SRAM. Reset preserves the core's SRAM, redraws the reset frame and
attempts audio activation during the click without awaiting it. Successful
visible resets show **Playing** and the common **Pause** button, even when
previously paused. The canvas receives focus for immediate game input.
Absent cartridges, loading and unresolved save restoration still guard Reset;
hidden documents remain paused. Reset failures report an error and remain
paused. Persistence warnings remain separate from operation errors.

No Rust/Wasm hardware source, ROM binary, ROM hash, SRAM identity, dependency or
published artifact allowlist changed. Both bundled games retain the distribution
permission and asset notices in [the ROM notice](../demos/NOTICE.md); diagnostics
and cleared games remain test-only under the existing provenance decisions.

## Validation on 2026-10-31

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo test --workspace --locked`: 519 passed.
- `python3 tools/make-fixture.py .cache/smoke.bin` and comparison with the tracked
  fixture: identical SHA-256, passed.
- `python3 tools/build-cleared-games.py`, run twice after building the pinned
  assemblers: passed both builds with the manifest's Castle/Memory ROM hashes.
- `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65"
  --ld65 "$PWD/.cache/tools/cc65/bin/ld65"`: both CPU suites passed.
- `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"`:
  scanline HDMA fixture passed.
- Production static/Wasm build passed with
  `RUSTC="$(rustup which --toolchain 1.99.1 rustc)" npm --prefix web run build`.
  The default Homebrew Rust 1.94 compiler lacked the Wasm target; the installed
  Rust 1.99.0 toolchain's linker then failed to locate `libLLVM.dylib`. Selecting
  the existing 1.99.1 compiler resolved the local build without repository changes.
  An early browser attempt stopped at server startup while `dist` was absent;
  the successful runs used the completed production build.
- Focused Chromium run of reset, emulator, demo launch, high-resolution,
  pseudo-hires and touch suites: 48 passed (28.3 seconds).
- `python3 tools/build-test-tools.py --spc`: blocked at the SPC assembler
  download with `Hash mismatch for spcasm`. The expected archive hash remains
  `979b0fbe19830ffb49d2f187a5512044b7514914f1d289686c340ffd1d6a6e63`.
  No mismatching assembler was extracted or executed, and the pin was not changed.
  The required SPC700 source gate therefore could not run; this is a remaining
  validation blocker. The independent CPU and HDMA gates above passed.
- `SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'`:
  all 105 Chromium tests passed (6.7 minutes), including both bundled games,
  cleared-game playthroughs, audio drift/stalls, Pages artifact/hash/subpath/MIME
  checks, picker cancellation, keyboard/gamepad, all twelve touch controls,
  fullscreen/safe areas and 256/512 canvas rendering. Browser diagnostics found
  no unexpected console, CSP, request or runtime errors in the checked flows.

`web/tests/reset.spec.js` executes genuine Wasm with observation wrappers and
targeted reset/audio failure injection. Desktop (1280×900), narrow portrait
(320×740) and short landscape (667×320) checks cover both playback states,
reset-origin markers, subsequent frame/input progress, SRAM, stable geometry,
audio flushing and bounded scheduling. Other cases cover blocked, pending and
unavailable Web Audio, false-return/throwing resets, loading/restore guards and
hidden-tab behavior. The touch suite also verifies captured-input release and
all twelve buttons plus gamepad input after Reset without Play.

Existing hardware/replay checks now explicitly pause in the same browser task
as Reset via `web/tests/playback.js`. Their exact fresh-frame, pixel, 256/512
canvas, native/Wasm and replay assertions are retained. Pages tests also verify
reset autoplay and SRAM for both bundled games under `/snes-rust/`.

## Follow-up after merge

Required by Issue #128 and not performed here: after the new `main` GitHub Pages
deployment, record its workflow run and live URL, then check desktop and mobile
labels, selector geometry, Reset from playing/paused, video/input/audio and
404/CSP/MIME/runtime errors. This is post-merge work and does not block pre-merge
approval. No push, PR, merge or GitHub comment was performed by this node.

Optional physical iOS Safari/Android Chrome and qualitative first-time visitor
checks were not performed. Optional user-supplied external-game replays remain
excluded by the documented `--grep-invert '301 seconds'` browser command.
