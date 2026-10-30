# Issue #118: emulator-first browser hierarchy

## Design and scope

Before, the sidebar's “BUNDLED ADVENTURE” and 26px Japanese game heading
outweighed the 21px app name. The separate romanized subtitle also consumed a
full row on idle portrait phones. That hierarchy could look like a game website
and remained awkward when selecting another cartridge.

Now, **snes-rust** is the primary heading (28px desktop / 20px mobile), with a
small **SNES emulator** descriptor. The unchanged generic player dominates the
desktop. **Try demo** and **Open ROM** share one accessible Game selection group;
the demo name is a 12px caption associated with its button. A small desktop
current-cartridge label follows successful loads, including local filenames;
failed replacements retain the previous label. Mobile omits that extra row once
playing, giving the space to the screen. Short landscape keeps the center video,
left/right controller clusters and compact actions.

This follows the README principles: remove promotional prose and redundant
headings, communicate alternatives through grouping and spacing, and retain
familiar actions. The neutral idle placeholder, real emulated pixels, playback
lifecycle and all twelve touch buttons remain. Only the initial action copy and
current-cartridge label bindings change in JavaScript. The pinned ROM, provenance,
full About / Legal notice, privacy/recovery guidance, storage pipeline, CSP and
five-file Pages artifact remain unchanged. No artwork, framework, ROM or upload
path was added. The game remains all rights reserved under its
[existing notice](../demos/NOTICE.md), separately from the emulator's MIT license.

## Actual production-build screenshots

Captured on 2026-10-30 in Playwright 1.61.1 / Chromium 149.0.7827.55. Before uses
upstream `f1d2e9e6b447489abf813f7d70aafae64f2eca60`; after uses this implementation.
Each view uses a clean context; mobile enables touch and mobile viewport behavior.
Playing images follow one activation of the demo and show the unchanged game's
actual Wasm-rendered title screen, under its game-specific notice. Ready images
show the neutral placeholder. These full-page images are evidence outside the
published artifact, with no generated art or promotional overlays.

| Viewport (CSS px) | Before, ready | After, ready | Before, playing | After, playing |
| --- | --- | --- | --- | --- |
| Desktop 1280 × 900 | [Image](issue-118/before-desktop-ready.png) | [Image](issue-118/after-desktop-ready.png) | [Image](issue-118/before-desktop-playing.png) | [Image](issue-118/after-desktop-playing.png) |
| Narrow portrait 320 × 740 | [Image](issue-118/before-portrait-ready.png) | [Image](issue-118/after-portrait-ready.png) | [Image](issue-118/before-portrait-playing.png) | [Image](issue-118/after-portrait-playing.png) |
| Short portrait 360 × 640 | [Image](issue-118/before-short-portrait-ready.png) | [Image](issue-118/after-short-portrait-ready.png) | [Image](issue-118/before-short-portrait-playing.png) | [Image](issue-118/after-short-portrait-playing.png) |
| Short landscape 667 × 320 | [Image](issue-118/before-landscape-ready.png) | [Image](issue-118/after-landscape-ready.png) | [Image](issue-118/before-landscape-playing.png) | [Image](issue-118/after-landscape-playing.png) |
| Small landscape 568 × 320; safe areas 44px left/right, 21px bottom | [Image](issue-118/before-short-landscape-safe-area-ready.png) | [Image](issue-118/after-short-landscape-safe-area-ready.png) | [Image](issue-118/before-short-landscape-safe-area-playing.png) | [Image](issue-118/after-short-landscape-safe-area-playing.png) |

## Executed validation

Environment: macOS arm64, Homebrew Rust 1.94.0 for native checks; rustup Rust
1.99.0 for Wasm and the SPC assembler; Node 26.12.1, Python 3.13.13.

| Check | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --workspace --locked` | 518 passed; no failures |
| `python3 tools/make-fixture.py .cache/smoke.bin` and `cmp tests/roms/smoke.bin .cache/smoke.bin` | Identical |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Pinned assemblers built successfully |
| `python3 tools/build-cleared-games.py`, twice | Both Castle/Memory builds match the pinned hashes |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | All three source-built CPU/SPC suites passed |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | All eleven HDMA grayscale bands verified |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Production Wasm/site build passed |
| `SNES_TEST_PORT=18094 npm --prefix web test -- --grep-invert '301 seconds' --reporter=line --output=../.cache/issue-118-final-results` | All 80 passed in 3.8 minutes; no failures or skips, including Pages smoke |
| Focused hierarchy/touch suite (`--grep 'hierarchy\|touch'`) | All 27 passed |
| `git diff --check`, including staged new files | Passed |

New browser coverage asserts the visible app heading and descriptor, grouped
entry alternatives, accessible demo description, subordinate caption placement
and emphasis, dominant desktop screen, legal disclosure and current-cartridge
label updates across demo/local/failed loads. Mobile idle checks cover all seven
Issue #116 viewport sizes and both emulated landscape safe-area cases. Existing
real guest/controller checks now also bound entry actions and Play/Pause; idle and active layouts assert no horizontal or
unnecessary vertical overflow. All twelve touch targets retain their 44px size,
hit testing and 256:224 video geometry, including resize, fullscreen and emulated
safe areas. Existing demo gesture/audio/download and save recovery tests remain.

During implementation, the new scroll-height checks reproduced excess space in
an intermediate layout: 320 × 740 idle portrait and three short landscapes
scrolled, including wrapped idle status with safe areas. Reserving space for
the taller app header, idle caption and wrapped status fixed those cases; active
portrait returns the metadata row to the player. Recovery errors remain outside the constrained video and can expand the page when needed.

The full suite also passed the unchanged Castle five-minute replay and complete
map/credits route, Memory five-minute playthroughs, music/effects and real Web
Audio scheduling checks. The unchanged source-built games retain SHA-256 values
`197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`
(Castle) and
`6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`
(Memory). No core or hardware accuracy change is claimed.

## Limits and required post-merge verification

Physical mobile browsers, assistive technology and new-visitor usability feedback
were not tested; those are optional. Emulated safe areas do not establish physical
device behavior. The optional unaudited user-supplied game tests named
`301 seconds` are excluded by the documented mandatory command; no such games
were fetched or bundled. Existing emulator compatibility limits remain.

After merge, the author/merger must inspect the resulting `main` Pages workflow
and visually verify the deployed
[public page](https://takahirox.github.io/snes-rust/) on desktop and mobile,
including working demo/local-ROM choices. Record the run, URL and results; open a separate
task Issue with evidence if publication fails. This required check remains
pending at the implementation checkpoint and is not a pre-merge condition. No push, PR, merge, comment, deployment
or background monitoring was performed by this implementation node.

## Post-merge verification

Completed on 2026-10-30 at 07:08 UTC by the merge node. Immediately before
merging, the source Issue was read from GitHub and confirmed open. PR #119
still had the reviewed head `93164656b8d33d45c09da15bcbb4864794e8024a`;
both its push and pull-request core/browser CI runs passed. GitHub merged it
with the expected-head guard and the `merge` method, producing
[`c788796706cd8161748df154362b8987b2f72287`](https://github.com/takahirox/snes-rust/commit/c788796706cd8161748df154362b8987b2f72287).
The merge's second parent is the reviewed PR head, preserving its checkpoint
history. GitHub then closed Issue #118.

The resulting main
[Deploy GitHub Pages run #41017418117](https://github.com/takahirox/snes-rust/actions/runs/41017418117)
passed its core, browser and deployment jobs for that merge commit. Its production
build and full mandatory browser suite passed: **80 tests in 6.0 minutes**.

After deployment, fresh Chromium 149.0.7827.55 contexts visited the actual
[public site](https://takahirox.github.io/snes-rust/). All four viewports below
passed live hierarchy, entry-action, playback and layout assertions. The app name
was 28px on desktop and 20px on mobile, above the 12px demo caption. One demo
activation per context loaded the real public ROM once and ran more than 130
Wasm frames; canvas pixels contained multiple colors. Selecting the repository's
source-built `tests/roms/smoke.bin` as `Local diagnostic.sfc` updated the cartridge
metadata and played successfully in every context. The observed requests were
only GETs for the public page and its four assets; no local ROM or SRAM was
uploaded. No page JavaScript errors occurred.

Visual inspection of the public desktop, portrait and landscape captures
confirmed the emulator-first identity, dominant generic desktop player, compact
demo/local alternatives, and unobscured mobile screen and controls. All twelve
touch buttons and Play/Pause were within the mobile viewport before and after
launch, with no horizontal or vertical overflow. The emulated safe-area view
also kept targets within 44px left/right and 21px bottom insets.

| Public viewport | Ready | Playing |
| --- | --- | --- |
| Desktop 1280 × 900 | [Image](issue-118/public-desktop-ready.png) | [Image](issue-118/public-desktop-playing.png) |
| Portrait 320 × 740 | [Image](issue-118/public-portrait-ready.png) | [Image](issue-118/public-portrait-playing.png) |
| Landscape 667 × 320 | [Image](issue-118/public-landscape-ready.png) | [Image](issue-118/public-landscape-playing.png) |
| Landscape 568 × 320 with emulated safe areas | [Image](issue-118/public-landscape-safe-area-ready.png) | [Image](issue-118/public-landscape-safe-area-playing.png) |

These are actual public-page screenshots, retained in the merge node's GitWeave
checkpoint with this record. They show the bundled game's genuine emulated pixels
under the existing ROM notice. Physical-device review remains unperformed and
optional. Required post-merge verification is complete; publication succeeded,
so no follow-up publication-failure Issue was needed. No background monitoring
was started.
