# Issue 147 CRT default validation

[Issue #147](https://github.com/takahirox/snes-rust/issues/147) changes the browser
startup default to CRT. Only the exact saved `pixel` preference opts out; explicit
`crt`, missing, invalid and inaccessible preferences select CRT when WebGL works.
The Display selector lists CRT first and continues to reflect the active mode.
Startup GPU fallback retains the existing Pixel status/retry path without
writing a display preference.

The renderer, shader, resource cleanup, performance guard and source Canvas 2D
path are unchanged. The [Issue #142 record](issue-142-validation.md), screenshots
and measurements remain historical evidence of the earlier optional CRT release.
No core, ROM file/hash, legal notice, dependency, published asset allowlist,
application layout or unrelated product behavior changes.

## Browser coverage

The production-build `display.spec.js` now verifies a fresh browser context has no
stored display preference, selects CRT without a settings interaction, and shows
a real linked WebGL program with lit GPU readback and changing canvas frames.
Switching to Pixel exposes a lit source canvas with continuing Wasm frames and
working controller input.

Preference checks cover exact Pixel/CRT values, an invalid string, an empty
string, a JSON-quoted Pixel string, denied preference reads/writes and an
inaccessible `localStorage` property. The last case uses an in-memory variant of
the original MIT diagnostic with no SRAM, preserving the separate protection
against playing a battery-backed cartridge before save restoration succeeds.
Both explicit preferences are selected, reloaded and checked with a playing ROM.

Unavailable WebGL, compile/link/draw/error failures and sustained renderer/full
workload overload now exercise automatic startup CRT, without an explicit CRT
selection. Each must expose lit, playable Pixel, report the fallback, leave the
preference absent and allow an explicit CRT retry after recovery. Existing context
loss/restoration, GPU resource balance, pixel orientation/opacity/capture,
256/512 transitions, fullscreen, resize and retina touch checks remain.

The live switching check retains native selector keyboard/focus behavior,
frame progression, input, audio starts, per-change audio-stop deltas, SRAM and
exact source framebuffer bytes; it also compares source geometry across switches.
Existing browser tests target `#video` explicitly where they require the source
canvas, since default CRT adds a second canvas. The source-driven Castle replay
screenshots refresh the overlay after writing their paused evidence frames.
The Pages subpath smoke test also asserts default CRT and a visible overlay.

## Local validation on 2026-11-17

Environment: macOS 27.2.1 arm64, Node 26.12.1, Python 3.13.13, Playwright 1.61.1
with headless Chromium 149.0.7827.55.
Workspace checks used Homebrew Rust 1.94; the production Wasm build used rustup
Rust 1.99 via the README's compiler override.

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo test --workspace --locked`: 519 passed.
- `python3 tools/make-fixture.py .cache/smoke.bin` and comparison with
  `tests/roms/smoke.bin`: byte-identical.
- `python3 tools/build-test-tools.py`: pinned ca65/ld65 and bass built; the existing
  C string-size and duplicate C++ linker-library warnings did not prevent success.
- Two consecutive `python3 tools/build-cleared-games.py` builds: passed with
  identical outputs and the pinned Castle/Memory hashes.
- `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65"
  --ld65 "$PWD/.cache/tools/cc65/bin/ld65"`: both CPU suites passed.
- `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"`:
  all eleven grayscale bands and final-row persistence passed.
- `npm --prefix web ci` and
  `RUSTC="$(rustup which rustc)" npm --prefix web run build`: passed.
- `node --check` on the app and all changed JavaScript tests: passed (11 files).
  Production `app.js` and `index.html` are byte-identical to their final sources.
- `npm --prefix web test -- display.spec.js --grep-invert 'screenshots|measure Pixel'
  --reporter=list`: focused display run, 24 passed in 24.1 seconds.
- `npm --prefix web test -- --grep-invert '301 seconds' --reporter=list`: **149 passed
  in 6.3 minutes**, with no failures, retries or skipped tests in the executed
  suite. Includes all 27 display tests, both demo comparisons, workload
  measurements, complete cleared-game routes/replays, Pages subpath/build asset
  allowlist/hash/MIME checks, SRAM recovery, audio, reset, keyboard/gamepad and
  responsive full SNES touch controls.
- Staged `git diff --check`, documentation links and final diff scope: passed.

The separate source-built SPC700 suite is not run locally for this browser-only
change; its existing CI gate remains. The documented full browser command excludes
optional user-supplied `301 seconds` game tests; no such ROMs are fetched. Physical
mobile/GPU testing and remote CI are not claimed.

## Required post-merge verification — pending

After merge, verify the new `main` Pages workflow and the published app at
<https://takahirox.github.io/snes-rust/>. Record the workflow/revision, fresh-context
CRT selection and live frames, explicit Pixel persistence after reload, and
playable Pixel with WebGL unavailable. Local production/subpath checks do not
establish deployment success. If a defect is found, track it in a separate linked
task. This node does not push, create a PR, merge, comment or arrange monitoring.
