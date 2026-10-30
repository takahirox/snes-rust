# Issue #116: published Pages verification

[PR #117](https://github.com/takahirox/snes-rust/pull/117) merged on
2026-10-30 as `41b4c64357afd454c469553150128df072b1647a`, with reviewed
head `457a838f74d7e366b684704736bc38f436ddf4e5` as its second parent.
Issue #116 was read immediately before merging and confirmed open. GitHub
accepted the merge with that expected head SHA; no checks or reviews were
bypassed. The merge closed the Issue.

## Deployment

[Pages run 41006942035](https://github.com/takahirox/snes-rust/actions/runs/41006942035)
tested and published that merge commit. Core and browser jobs passed; the browser
log reports **69 passed (6.0m)**. The
[deployment job](https://github.com/takahirox/snes-rust/actions/runs/41006942035/job/123905941092)
completed successfully at 05:17:47 UTC. Deployment `7431326094` reports success
and the public URL <https://takahirox.github.io/snes-rust/>.

## Actual published-site checks

At 05:18:24 UTC, Playwright 1.61.1 / Chromium 149.0.7827.55 opened the live HTTPS
site in three fresh browser contexts. A one-off runner observed real Wasm
loading, frame execution and audio resume without replacing the cartridge,
network responses or emulator behavior. The
[machine-readable results](../tests/results/issue-116-pages.json) preserve
asset hashes, geometry, scene hashes and observed outcomes.

| Context | Viewport (CSS px) | Result |
| --- | --- | --- |
| Desktop | 1280 × 900 | One-click launch, running frames/audio, pause/resume, SRAM persistence passed |
| Touch portrait | 375 × 667 | One-tap launch, all twelve controls visible/hit-testable, touch gameplay passed |
| Touch landscape | 667 × 320 | One-tap launch, controls beside the display, touch gameplay passed |

- Audio resume occurred in the initial user gesture before ROM loading; no audio
  was activated on page load. Each context loaded the cartridge exactly once.
- Real CDP touch input on SNES A changed the game's title background into its
  story scene in both mobile contexts; framebuffer hashes differed.
- While playing, each mobile context resized through 568 × 320, 320 × 740,
  375 × 667 and 667 × 320. All twelve controls stayed within the viewport and
  hit-testable, with targets at least 44 CSS pixels and no horizontal overflow.
  Canvas retained the 256:224 aspect ratio; emulation stayed playing without
  loading another cartridge.
- Pause stopped frames, persisted the demo's 8192-byte SRAM under its pinned
  per-ROM key, and resume restarted frames without another fetch/load.
- Each context made exactly five same-origin GET requests under `/snes-rust/`:
  the page, CSS, JavaScript, Wasm and Shionari ROM. All returned HTTP 200 with
  appropriate MIME types, including `application/wasm`. Published HTML, CSS,
  JavaScript and ROM bytes matched the committed files. The ROM retained SHA-256
  `fec1f087f5cc6fbac6f073821b6212188d7b77a2e8191fa82f6056f80fb75343`.
- No failed requests, 404s, CSP/MIME errors, error-level console messages,
  page exceptions or application errors occurred. Actual desktop, portrait and
  landscape screenshots were captured and visually inspected during this check;
  local captures remain in ignored `.cache/issue-116-post-merge/`.

The required post-merge verification passed; no follow-up failure Issue was
needed. This checkpoint adds validation records only. Physical iOS/Android,
Bluetooth controllers and assistive technology remain untested; Chromium mobile
emulation does not establish physical-device behavior. No background monitoring
is scheduled.
