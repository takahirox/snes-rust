# Issue 134 mute and volume validation

The playback toolbar now has one always-visible audio group: an accessible mute
button beside a native 0–100% slider. The old Sound checkbox is removed. Muting
sets the slider to zero; unmute restores the last nonzero level. Raising the
slider above zero unmutes. The default is 100%, with a validated browser-wide
`snes-rust:volume` preference shared across cartridges. Invalid or inaccessible
preference storage falls back safely without changing SRAM safeguards.

All 32 kHz stereo sources connect through one master `GainNode`, including
already queued buffers. Gain stays within 0–1 and approaches changes with a
5 ms time constant. Volume changes preserve frame generation and the bounded
audio queue. Pause, Reset, cartridge changes and fullscreen retain the preference;
playback gestures and volume/unmute gestures retain normal Web Audio activation
and error recovery. Help and README describe the replacement controls.

## Validation on 2026-11-02

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo test --workspace --locked`: 519 passed.
- `python3 tools/make-fixture.py .cache/smoke.bin` and comparison with the tracked
  fixture: identical.
- `python3 tools/build-test-tools.py` and two consecutive
  `python3 tools/build-cleared-games.py` builds: passed with the pinned
  Castle/Memory hashes.
- `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65"
  --ld65 "$PWD/.cache/tools/cc65/bin/ld65"`: both CPU suites passed.
- `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"`:
  scanline HDMA fixture passed.
- `RUSTC="$(rustup which rustc)" npm --prefix web run build`: production Wasm/site
  build passed. `npm --prefix web ci` and Chromium installation passed.
- `npm --prefix web test -- --grep-invert '301 seconds' --reporter=list,json`:
  122 passed in 301.5 seconds, zero failures, flakes or skips. This includes
  cleared-game playthroughs, queue/drift/stall checks, both bundled demos,
  keyboard/gamepad/all twelve touch buttons, safe areas/fullscreen, rendering,
  and production asset hashes, allowlist, CSP, MIME and `/snes-rust/` behavior.
- Focused existing emulator, cartridge launch, Reset, hierarchy and touch tests:
  61 passed. Focused new volume tests: 17 passed. After tightening the silence
  measurement and adding slider-gesture recovery to the launch regression,
  `npm --prefix web test -- volume.spec.js demo-launch.spec.js --reporter=list,json`:
  25 passed in 15.3 seconds. Browser diagnostics detected no unexpected console
  or runtime errors in these flows.

`web/tests/volume.spec.js` uses genuine Web Audio with matched analyser taps
before and after the master gain. It measures the output/input RMS ratio at
100%, 50%, 25% and 0% for both pinned demos and the source-built local smoke ROM,
while checking frame progression and every source's connection to the same
master gain. At zero, measured output RMS must be below 1e-8 and attenuation
ratio below 1e-5 while input remains nonzero. A separate check changes gain while blocking the main thread for
40 ms: existing scheduled audio attenuates without any new frames or sources.
The recorded measurements are in [audio measurements](issue-134/audio-measurements.json).

The new checks also cover native Home/End/arrow keys, mute focus and accessible
names/state, zero-volume semantics, restore after mute, adjustment while muted or
paused, cartridge/Reset/fullscreen continuity, reload of muted and audible
settings, malformed storage, and denied storage access/reads/writes. A non-battery
variant of the original smoke fixture isolates audio storage denial from the
existing battery-save protection. Desktop mouse and actual Chromium touch events
drag the native slider. Geometry checks cover desktop, 320px portrait, 667×320
landscape and 568×320 landscape with 44px side/21px bottom safe-area insets,
including stable positions before/after cartridge insertion, 44px audio hit areas,
no horizontal scroll and no screen/controller overlap.

## Implementation screenshots

These are Chromium screenshots of the production app, before inserting a
cartridge and while the original local diagnostic is running.

| Viewport | Empty slot | Playing |
| --- | --- | --- |
| Desktop 1280×900 | [Before](issue-134/desktop-before.png) | [After](issue-134/desktop-playing.png) |
| Portrait 320×740 | [Before](issue-134/portrait-before.png) | [After](issue-134/portrait-playing.png) |
| Landscape 667×320 | [Before](issue-134/landscape-before.png) | [After](issue-134/landscape-playing.png) |
| Landscape 568×320 with safe areas | [Before](issue-134/safe-area-before.png) | [After](issue-134/safe-area-playing.png) |

## CI and follow-up

The required GitHub jobs remain `Emulator checks / core` and
`Emulator checks / browser`; no remote CI run is claimed by this local node.
The SPC700 assembler/source suite was not run locally (no core changes); CI retains its existing
pinned source gate. No Rust hardware source, ROM binary, ROM hash, SRAM identity,
dependency or published asset allowlist changed. The two demos keep their
[existing distribution permission and asset notices](../demos/NOTICE.md);
cleared games and diagnostic ROMs remain test-only.

Optional physical iOS Safari/Android Chrome and headphone listening were not
performed. User-supplied external-game replays remain excluded by the documented
`--grep-invert '301 seconds'` command.

After merge, Issue 134 requires checking the new main Pages deployment at
desktop, portrait and landscape sizes for visible controls, mute/live volume,
and page errors, recording the deployment run and live evidence. That check is
pending and belongs after merge. This node does not push, open a PR, merge or
comment on GitHub.
