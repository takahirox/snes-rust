# Issue #162 browser controls

[Issue #162](https://github.com/takahirox/snes-rust/issues/162) makes display
settings visible before a game loads and during play. The previous header hid
them in Help beside an About / Legal disclosure. The new header keeps Help as
a short keyboard, touch and gamepad reference, with a separate group of native
Scale and Display selectors and a direct GitHub link. On narrow phones, the
settings wrap above the screen and the volume slider flexes so the playback
buttons stay on one row. In short landscape, the header fits one row above the
screen and both controller clusters.

Scale requests a 256-pixel multiple up to the space available. On a short
landscape screen, that limit can make all three settings render at the same
size. Display keeps its existing CRT default, stored preferences and Pixel
fallback. The [README](../README.md#run-the-browser-app) covers local ROM privacy,
audio and save recovery. The GitHub repository README links to the
[ROM provenance and distribution notice](../demos/NOTICE.md), which retains
the game permissions and the distinction from the emulator's MIT license.

## Production screenshots

Shionari no Misshitsu running with default 3× Scale and CRT Display in headless
Chromium. The phone captures use touch emulation.

| Viewport | Canvas (CSS px, rounded) | Capture |
| --- | --- | --- |
| Desktop 1280×900 | 768×672 | [Desktop](issue-162/desktop.png) |
| Portrait 320×740 | 265×232 | [Narrow portrait](issue-162/portrait.png) |
| Landscape 667×320 | 158×138 | [Short landscape](issue-162/landscape.png) |

## Local validation

- Workspace formatting, Clippy and Rust tests passed. The original diagnostic
  reproduced byte-for-byte, the CPU and SPC700 ROM suites and HDMA fixture
  passed, and both audited browser fixtures reproduced their pinned hashes on
  two builds.
- `npm --prefix web ci` and the production build passed on macOS arm64. The
  Wasm build used Rust 1.99.0 and a local linker library-path override.
- `npm --prefix web test -- --grep-invert '301 seconds' --reporter=list`
  passed all 152 tests in 6.2 minutes on Chromium 149.0.7827.55, including all
  17 Pages subpath tests. Coverage includes native selector focus and input,
  mobile touch, both display preferences, GPU fallback and actionable audio
  and save messages. The first full run found the narrow portrait slider
  below its 58 px minimum. The final layout passed its existing drag test and
  kept all 12 SNES buttons in view.
- The live GitHub repository README linked to `demos/NOTICE.md`, and the
  notice blob matched this checkout. Both shipped ROMs match the README hashes.

The command excludes the optional user-supplied `301 seconds` cases. Remote
GitHub Actions, physical mobile devices and screen-reader checks were outside
this local run.

## Required post-merge verification

After merge, inspect the new `main` Pages workflow and the
[published emulator](https://takahirox.github.io/snes-rust/) at the three
viewports above. Verify the GitHub destination and both selectors during live
play. Record the workflow and revision with the results. If the published
check finds a defect, track it in a separate linked task when invoked.
