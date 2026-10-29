# Issue #110: bundled Shionari browser demo

## Change and distribution boundary

[Issue #110](https://github.com/takahirox/snes-rust/issues/110) adds one bundled
original game to the existing browser frontend. Both local files and the demo
use one cartridge-loading function: pause/save, read, normalize local copier
headers, hash, save again, core validation/replacement, restore that cartridge's
SRAM, draw, then wait for Play. Reloading the demo resets volatile state and
restores its own saved SRAM. Neither loading path activates audio. Loading blocks
concurrent cartridge/Play/Reset actions; failures preserve the previous machine
or retain the failed-restore write lock and recovery control.

The demo fetch is relative, same-origin and rejects redirects. The frontend
checks its exact size and SHA-256 before loading it through the unchanged Wasm
core. There are no emulation or game-specific hardware changes.

The build copies only `demos/shionari.sfc`, explicitly, into `web/dist`.
The output allowlist is `index.html`, `app.js`, `style.css`, `snes_wasm.wasm`,
`shionari.sfc`. The game is 524,288 bytes, SHA-256
`3cb02d07e3bf2bc3bf470311b4046ffb061ee7f8aa7b844e551b4144f0826b42`.
An additional rebuild check seeded `castle.sfc`, `memory.sfc`, `smoke.bin` and a
hidden `.stale` file in dist; rebuilding removed all four and left exactly the
five allowed files with the pinned ROM bytes.

The previous app shipped no games and offered only a local file picker. There
was no legacy public demo payload or selector to delete. Castle/Memory source
builds, their audits, diagnostics, hardware fixtures and existing regression
tests are retained unchanged. The [separate ROM notice](../demos/NOTICE.md)
records the maintainer's exact project/public-demo permission, AI-assisted
authorship, embedded-content provenance and reserved rights; the emulator's MIT
license does not cover the game. User ROMs and SRAM are never uploaded.

## Browser assertions and limits

`web/tests/pages.spec.js` serves the actual production `web/dist` strictly under
`/snes-rust/`; it does not generate a substitute frontend or Wasm. It checks the
complete allowlist, regular files, exact ROM size/hash, same-origin GET requests,
successful responses and advertised HTML/JS/CSS/Wasm/binary MIME types. Both
`application/octet-stream` and `application/vnd.nintendo.snes` ROM responses
load through `arrayBuffer`. Unpublished/root/test/cache paths return 404. A
separate deliberate remote-fetch probe confirms the existing CSP blocks it;
its expected CSP error is separate from the error-free gameplay checks.

The Shionari gameplay check clicks the accessible demo control, observes the
real Wasm exports (battery flag 1, SRAM length 8192), and verifies no AudioContext
exists until clicking Play. It runs the original game's title for one second,
checks the background is unchanged after another half second, then holds keyboard
X (the existing SNES A mapping) until the scene changes. After another second,
the warehouse story background differs from the title. The SHA-256 comparison
uses framebuffer rows 16–143, excluding dialogue/typewriter text and the blinking
cursor, so idle dialogue animation cannot satisfy progression. Screenshots of
both canvases and the two background hashes are attached to the Playwright test
result. Direct inspection confirmed the title's harbor and the story's warehouse
office. Pause freezes the image; Reset succeeds with SRAM preserved. Normal play
covers about three seconds, with Playwright's 30-second test timeout bounding
failure. No JavaScript, console, CSP, failed-network or HTTP errors are permitted
in this gameplay check. This demonstrates controller-driven entry into the
adventure, not a full game clear or a claim of complete emulator compatibility.

Battery tests seed byte 3 through the existing real Wasm SRAM buffer, rather than
claiming an in-game save-menu playthrough. They verify reset and page-reload
retention, same-demo reload, identical local/copier-header save identity, separate
keys/bytes for the local diagnostic, and switching in both directions. Failure
checks cover quota errors before loading and at the asynchronous replacement
boundary, blocked demo/local/same-demo replacement, unavailable storage reads,
invalid base64, wrong-size saves, blocked page-exit/reset writes, and retry
recovery. HTTP/download-integrity failures and invalid local headers leave the
previous cartridge playable with its save intact. Existing keyboard/gamepad,
Web Audio, video/256–512 scaling and cleared-game checks remain in the full suite.

## Executed pre-merge checks

Executed on 2026-10-30, macOS arm64, Node 26.12.1, Python 3.13.13, Chromium via
Playwright 1.61.1, with Rust 1.99.0 from rustup. On this mixed Homebrew/rustup
installation the Rust build/test commands selected
`RUSTC="$(rustup which rustc)"` and the workspace test also selected
`RUSTDOC="$(rustup which rustdoc)"`.

- `cargo fmt --all -- --check`: passed (also with `rustup run stable cargo`).
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo test --workspace --locked`: 518 tests passed, no failures or ignored
  tests; doctests passed. The first run's doctest failed because PATH selected
  Homebrew rustdoc 1.94 with rustup rustc 1.99; selecting matching RUSTDOC resolved
  it. No source workaround was made.
- `python3 tools/make-fixture.py .cache/smoke.bin` and
  `cmp tests/roms/smoke.bin .cache/smoke.bin`: passed.
- `python3 tools/build-test-tools.py --spc`: passed; hash-pinned cc65,
  bass-untech and SPC assembler built locally.
- `python3 tools/build-cleared-games.py`, twice: passed with unchanged hashes:
  Castle `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`;
  Memory `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
- `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65"
  --ld65 "$PWD/.cache/tools/cc65/bin/ld65"
  --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"`: passed all three
  suites (full CPU terminal test `0649`, basic CPU `0452`, SPC `0557`).
- `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"`:
  passed, 11 grayscale bands and final shade through row 223.
- `npm --prefix web ci`, `npm --prefix web run build` and Chromium installation:
  passed. The clean/rebuild output matched the exact five-file allowlist and
  pinned 524,288-byte ROM hash above.
- Focused Chromium: `SNES_TEST_PORT=18085 npm --prefix web test --
  tests/pages.spec.js tests/emulator.spec.js`: 20 passed. An initial new-test
  failure used an SRAM offset beyond the diagnostic's smaller SRAM; using the
  existing diagnostic marker offset corrected the test. Product behavior and
  fixtures were not changed to satisfy it.

- Full relevant Chromium: `SNES_TEST_PORT=18085 npm --prefix web test --
  --grep-invert '301 seconds'`: **47 passed in 3.8 minutes**, no failures or
  skipped relevant tests. This includes Castle's five-minute replay, its
  complete five-map/credits route, Memory's five-minute controller playthrough,
  looping music/finite effects, real gesture-activated Web Audio and bounded
  audio queues, original keyboard/gamepad/save/error tests, 256/512 transitions,
  OBJ/Mode 7 fixtures, Issue #107 raster checks and all ten Pages/demo tests.
  The documented grep excludes four optional Gothicvania/original-Castle tests
  that require separately supplied, incompletely audited ROMs. Those external
  binaries were not acquired or run.
- `node --check` for the modified app, build tool and Pages test; documentation
  local-file link checks; staged `git diff --check`: passed.

The full browser run tested the clean production artifact after the stale-file
rebuild experiment. No ROM hashes, mandatory gates or existing fixture behavior
were changed. Temporary logs and browser evidence remain ignored under `.cache`
and `web/test-results`; they are not production assets.

## Required post-merge verification: pending

This worktree validation does not establish that the new demo is deployed.
After merge, an invoked/authorized author or merger must inspect the resulting
`main` Pages deployment, record its run URL and actual production URL/result,
confirm the visible demo control, fetch `/snes-rust/shionari.sfc` successfully
and check the exact size/hash, initialize real Wasm, load/play with input and a
local battery save, and check for 404/CSP/MIME/console errors. No push,
publication, PR, merge or live-deployment verification was performed in this
implementation node. A production failure requires a separate `task` Issue
linking the failed run and evidence. This post-merge-only check is not a
pre-merge blocker or implicit background monitoring.

Optional longer Shionari playthroughs, alternate browsers and physical SNES
checks were not performed. No external Shionari ROM acquisition was needed.
