# Issue #164 gameplay fullscreen

[Issue #164](https://github.com/takahirox/snes-rust/issues/164) moves Scale and
Display to the player toolbar. Desktop fullscreen fits the game image to
the viewport at its 256:224 display aspect ratio, including when the framebuffer
switches to 512 samples per row. On devices with a coarse primary pointer, the
fullscreen presentation also includes the twelve SNES buttons and an Exit
fullscreen button. The controller sits below the game in portrait and beside
it in landscape.

## Production screenshots

These captures show the title screen of the bundled Shionari no Misshitsu ROM
in Chromium 149.0.7827.55 with the production Wasm build. The two phone
viewports use touch emulation at DPR 2. Landscape also uses emulated insets of
44 px on either side, 8 px at the top and 21 px at the bottom.

| Viewport | CRT player | CRT fullscreen | Pixel player | Pixel fullscreen |
| --- | --- | --- | --- | --- |
| Desktop 1280×900 | [CRT](issue-164/desktop-crt-player.png) | [CRT fullscreen](issue-164/desktop-crt-fullscreen.png) | [Pixel](issue-164/desktop-pixel-player.png) | [Pixel fullscreen](issue-164/desktop-pixel-fullscreen.png) |
| Portrait 320×740 | [CRT](issue-164/portrait-crt-player.png) | [CRT fullscreen](issue-164/portrait-crt-fullscreen.png) | [Pixel](issue-164/portrait-pixel-player.png) | [Pixel fullscreen](issue-164/portrait-pixel-fullscreen.png) |
| Landscape 667×320 | [CRT](issue-164/landscape-crt-player.png) | [CRT fullscreen](issue-164/landscape-crt-fullscreen.png) | [Pixel](issue-164/landscape-pixel-player.png) | [Pixel fullscreen](issue-164/landscape-pixel-fullscreen.png) |

## Local validation

- Production Wasm and site build passed with Rust 1.99.0. Both bundled ROM
  assets matched their pinned SHA-256 hashes. The build contains the six
  allowlisted Pages assets. The [ROM notice](../demos/NOTICE.md) records the
  maintainer's distribution authorization.
- Workspace formatting, Clippy and 519 Rust tests passed. The original smoke
  ROM reproduced byte for byte, all three CPU/SPC700 suites and the scanline
  HDMA fixture passed, and both cleared game artifacts reproduced their pinned
  hashes on two builds.
- The Playwright suite passed all 167 applicable tests in 6.5 minutes on
  macOS arm64, including 17 Pages subpath checks. The documented
  `--grep-invert '301 seconds'` filter excluded four optional tests that
  require user-supplied ROMs.
- The browser tests observe the actual fullscreen element, check focus on
  entry and exit, and exercise missing and rejected requests. They read
  rendered CRT pixels and verify the visible Pixel surface and game input
  after WebGL context loss during fullscreen. The 256/512 guest changes
  resolution in fullscreen, and the paused framebuffer is checked through
  fullscreen entry and exit.
- Chrome Device Emulation rotates the fullscreen touch layout from 375×667
  to 424×320 and back. The touch tests use real multi-touch injections to
  check simultaneous buttons, cancellation and releases against a Wasm guest.
  Keyboard and emulated standard gamepad input also reach the guest.

Physical phones and tablets, including iOS Safari, were outside the local
test run.

## Required post-merge verification

Once merged, inspect the `main` Pages deployment and exercise the
[published emulator](https://takahirox.github.io/snes-rust/) at desktop and
touch viewport sizes. Record the deployed revision, workflow run, browser
evidence and any limitations. Track a confirmed published regression in a
separate linked task when invoked.
