# Issue #116: browser interface and touch controls

## Interaction and implementation

The light interface puts the actual Canvas display first, groups Shionari's
identity with a prominent **Play demo**, and keeps **Open ROM** secondary. Before
loading, the display uses a neutral placeholder; no promotional artwork or new
runtime asset is included. Help / Controls contains the keyboard map, scale,
local-file privacy and save recovery guidance. About / Legal preserves the full
game notice and provenance link.

The demo action invokes the existing size/hash-validated cartridge loader and
SRAM restoration, then starts frames. AudioContext creation/resume happens in the
initial activation handler, before the fetch. A rejected or pending audio unlock
leaves gameplay running and offers a Sound retry. Loading disables conflicting
actions; duplicate loading and the second click of a mouse double click are
ignored. Once activated, the primary action pauses/resumes rather than fetching
or resetting again. Reset remains a distinct SRAM-preserving action. A failed
restore labels the primary action **Restore save first** and directs focus to
recovery; recovery does not silently start gameplay.

Touch controls use independent pointer masks, ORed with keyboard and standard
Gamepad API input. Pointer capture handles outside releases; cancellation, lost
capture, blur, hidden tabs, resizing, orientation and fullscreen transitions clear
virtual input. The D-pad supports sliding into diagonals and multiple fingers.
All twelve buttons have accessible names, focus feedback, disabled/pressed states
and targets of at least 44 CSS pixels in tested phone layouts. Enter/Space can
operate a focused virtual button without also activating the game Start mapping.
Pointer-initiated playback focuses Canvas so Enter continues to reach game Start;
keyboard activation retains button focus for normal keyboard navigation.

Portrait places the controls below the screen, with shoulders above each cluster
and Select/Start toward the lower center. Short landscape places the clusters
beside the centered display. Neither arrangement overlays game pixels. Safe-area
padding and viewport-relative screen sizing leave room for browser chrome and
home indicators. Resizing changes presentation and input state, without replacing
the core or its SRAM.

The ROM, its pinned SHA-256, notice, CSP, production builder and five-file Pages
allowlist are unchanged. No Rust emulation behavior changed. Local ROMs and saves
remain local, and the existing failed-save/restore protections remain exercised.

## Actual browser screenshots

Captured locally from the production build in Playwright Chromium 149.0.7827.55,
with the original game running at its title screen. Before images use upstream
`f8d79231d32037d092db09e16838b8aaba82997b`; after images use this implementation.
Phone contexts enable touch and mobile viewport behavior. These are full-page
screenshots, so the former long page is visible beyond the specified viewport.
They show genuine emulated frames from the unchanged committed Shionari ROM,
under the repository's [game-specific notice](../demos/NOTICE.md), not concepts or
AI-generated artwork. Images are validation evidence, outside the published
artifact.

| Viewport (CSS px) | Before | After |
| --- | --- | --- |
| Desktop 1280 × 900 | [Screenshot](issue-116/before-desktop.png) | [Screenshot](issue-116/after-desktop.png) |
| Narrow portrait 320 × 740 | [Screenshot](issue-116/before-portrait.png) | [Screenshot](issue-116/after-portrait.png) |
| Short landscape 667 × 320 | [Screenshot](issue-116/before-landscape.png) | [Screenshot](issue-116/after-landscape.png) |

## Executed validation (2026-10-30)

Environment: macOS arm64, Homebrew Rust 1.94.0 for native checks, rustup Rust
1.99.0 for Wasm and the separately built SPC assembler, Node 26.12.1,
Python 3.13.13, Playwright 1.61.1 / Chromium 149.0.7827.55.

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --workspace --locked` | 518 passed; no failures |
| `python3 tools/make-fixture.py .cache/smoke.bin` and `cmp tests/roms/smoke.bin .cache/smoke.bin` | Identical |
| `python3 tools/build-test-tools.py`, then with `--spc` using rustup Rust | Passed |
| `python3 tools/build-cleared-games.py`, twice | Both builds match the pinned Castle and Memory hashes |
| Source-built `tools/hardware-tests.py` with ca65, ld65 and spcasm | All three CPU/SPC suites passed |
| Source-built `tools/ppu-test.py` with bass-untech | All eleven HDMA grayscale bands verified |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Production Wasm/site build passed |
| Full mandatory `SNES_TEST_PORT=18087 npm --prefix web test -- --grep-invert '301 seconds'` | 64 passed in 3.8 minutes; no failures or skips |
| `git diff --check` (including staged new files) | Passed |

The first default Wasm build encountered the README's known mixed Homebrew/rustup
compiler issue (Homebrew could not find the installed Wasm standard library).
Selecting rustup's compiler as documented fixed the build. The first overlap test
also exposed a test-driver error: CDP touch-end lists points to release, rather
than remaining points. Correcting that driver made the real multi-touch overlap
assertions pass; no product workaround was introduced. The full suite also
caught Enter being consumed by the focused Play button after a mouse click.
Focusing Canvas after pointer-initiated playback fixed this regression while
retaining native keyboard activation of focused buttons.

The new browser coverage checks each button through real guest auto-read bytes,
not CSS alone. A test-only subroutine added to a copy of the original MIT smoke
fixture records both controller bytes in WRAM; the committed fixture and bundled
game stay unchanged. Coverage includes multi-pointer diagonals/face/shoulder
combinations, two pointers holding one bit, keyboard/gamepad overlap, cancellation,
outside release, lost capture, focus/blur/visibility, fullscreen and live resize.

Mobile layout checks exercise 320 × 740, 390 × 844, 667 × 320, 844 × 390 and
568 × 320. Every control and the display must be within the viewport, hit-testable
and free of horizontal overflow; Canvas shape must remain 256:224. Additional CDP
safe-area emulation checks 44px side insets and a 21px home-indicator inset.
A Shionari touch-A test verifies an actual title-to-story video change. Launch
checks cover delayed fetch, double activation, fast double click, initial-gesture
audio resume, rejected/pending audio recovery, hidden-page completion, repeated
pause/resume/reset and SRAM-preserving fullscreen. Existing Pages tests retain
local switching, hash/MIME/error paths, save recovery/isolation, CSP and subpath
checks; original 256/512 geometry, diagnostic, Castle/Memory replays, progression
and audio scheduling tests remain included.

## Limitations and required post-merge work

Physical iOS Safari/Android Chrome, Bluetooth controllers, assistive technology
and human usability sessions were not tested. Chromium mobile emulation and
emulated safe areas do not establish behavior on physical devices. Very small
viewports and visible error/recovery messages can require vertical scrolling;
errors remain accessible instead of being clipped. The emulator retains its
existing compatibility limits. Optional unaudited user-supplied game checks
(`301 seconds`) are excluded by the documented mandatory command; none was
fetched or bundled.

After merge, the author/merger must check the actual new `main` GitHub Pages
deployment on desktop and portrait/landscape mobile viewports: one-tap launch,
visible controls, relative ROM/Wasm requests and absence of 404/CSP/MIME/runtime
errors. Record its deployment URL/run and results; open a follow-up task Issue
with reproduction/logs on failure. This check remains pending until after merge.
No background monitoring is implied.
