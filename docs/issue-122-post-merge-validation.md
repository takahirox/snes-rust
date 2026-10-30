# Issue #122 post-merge verification

[PR #123](https://github.com/takahirox/snes-rust/pull/123) merged on
2026-10-30 at 08:10:45 UTC with merge commit
`6d715316e9f0dbe8477d6e0e64dd73a7ec12494c`. Issue #122 was read immediately
before merging and confirmed open. The merge API required reviewed head
`f5382f1ba6af48665c6218e297528ccfdc0e0beb` and used a merge commit.

## Deployment

The merge-triggered [Deploy GitHub Pages run 41024717592](https://github.com/takahirox/snes-rust/actions/runs/41024717592)
completed successfully for that merge commit. Core checks, browser checks and
deployment all passed; deployment reported success at 08:17:25 UTC.
The browser job reported **93 passed**. Its configured command excludes the
optional `301 seconds` user-ROM checks; those were not exercised by this run.

Published URL: <https://takahirox.github.io/snes-rust/>.

## Published-app checks

After deployment, real Chromium 149.0.7827.55 checked the published HTTPS app
with fresh browser contexts, beginning at 08:17:31 UTC. No network responses or
cartridge-loading behavior were mocked. A wrapper counted calls to the actual
Wasm frame runner and exposed its exports for input assertions.

| Viewport | Both sources auto-start | Escape / outside dismissal | Layout stays fixed | Input after dismissal |
| --- | --- | --- | --- | --- |
| Desktop 1280 × 900 | Passed | Passed | Passed | Keyboard Right/release passed |
| Touch portrait 320 × 740 | Passed | Passed | Passed | All 12 SNES buttons/release passed |
| Touch landscape 667 × 320 | Passed | Passed | Passed | All 12 SNES buttons/release passed |

Each context began with `Inserted cartridge: None` and no audio activation.
Selecting the bundled demo produced actual non-black emulated frames and the
common `Pause` control without another Play click. Opening/dismissing the picker
kept playback running; it offered exactly two source buttons. Selecting a local
`.sfc` through the real Chromium filechooser event automatically started the
original MIT diagnostic with the same two-byte controller probe used by the
existing touch tests. No local ROM bytes were sent over the network.

Switching back to the demo from a paused local cartridge automatically resumed
gameplay. Canvas, cartridge slot and playback geometry matched the empty state
in loaded and paused states. Both mobile layouts had no page overflow, and all
12 controls fit in the viewport and reached the actual guest joypad registers.
Audio unlock was requested within user gestures and AudioContext reached
`running` in all three contexts. No page, console, failed-request, HTTP or CSP
errors were observed; all page requests were same-origin GETs.

The served HTML, JavaScript and CSS matched the reviewed worktree bytes in all
three contexts. Their SHA-256 values, observed frame counts, requests and results
are in [published-results.json](issue-122/published-results.json).

## Actual published-app screenshots

| Viewport | Demo playing | Local diagnostic with picker open |
| --- | --- | --- |
| Desktop | [Demo](issue-122/published-desktop-demo.png) | [Picker](issue-122/published-desktop-local-picker.png) |
| Portrait | [Demo](issue-122/published-portrait-demo.png) | [Picker](issue-122/published-portrait-local-picker.png) |
| Landscape | [Demo](issue-122/published-landscape-demo.png) | [Picker](issue-122/published-landscape-local-picker.png) |

Required post-merge verification passed. No failure follow-up Issue was needed.
Physical iOS/Android devices and assistive technology were not tested; the
mobile results above use Chromium touch emulation. No background monitoring
was established.
