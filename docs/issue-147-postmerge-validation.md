# Issue 147 post-merge verification

Completed on 2026-11-17 for [Issue #147](https://github.com/takahirox/snes-rust/issues/147)
and [PR #148](https://github.com/takahirox/snes-rust/pull/148).
The [implementation validation](issue-147-validation.md) remains the historical
pre-merge record; its pending post-merge work is completed by this report.

## Merge and deployment

- Immediately before the merge attempt, the GitHub Issue was fetched and confirmed
  open. The PR head matched `29824bee6ee5171aa039c449057a0bdc6b0227fb`.
- The complete [PR checks](https://github.com/takahirox/snes-rust/actions/runs/44022509818)
  passed. A duplicate push-triggered run was still in progress at merge time.
  GitHub reported no branch protection or applicable ruleset requirements, no
  submitted blocking reviews or review threads, and allowed merge commits.
- GitHub merged the PR at `2026-11-17T05:22:58Z`, using the expected-head SHA
  condition and the `merge` method. The resulting commit is
  `4c7685ed44d1df3adf76873ecebd5a74642c7a75`, with parents
  `847c1623ae86a6ca6c7cb9352b8ae634a65b6cff` and the reviewed PR head above.
  Issue #147 closed automatically.
- The new [main Pages workflow](https://github.com/takahirox/snes-rust/actions/runs/44023716737)
  passed its core, browser and deployment jobs for that merge commit. Its logs
  report **519 Rust tests passed** and **149 browser tests passed (7.7 minutes)**.
  The core job also completed its fixture, CPU/SPC700 and HDMA/PPU gates.
  The deployment reported <https://takahirox.github.io/snes-rust/> at
  `2026-11-17T05:32:25Z`.

## Published browser checks

Ran a focused Node/Playwright script against the public HTTPS app from
`2026-11-17T05:32:40Z` to `2026-11-17T05:32:43Z`, using Playwright 1.61.1 and
headless Chromium `149.0.7827.55`, with a 1280 × 900 viewport. Each scenario
started in a fresh browser context. The script wrapped Wasm `run_frame` to count
calls while invoking the original export; application assets were served by Pages.
Playback used the repository's original MIT `tests/roms/smoke.bin` diagnostic via
the local ROM chooser. No ROM or save was uploaded.

All checks passed:

1. **Default CRT:** storage initially had no `snes-rust:display` preference, and
   the selector automatically showed `crt`. After loading the diagnostic, the
   CRT canvas was visible, had a current WebGL program and nonzero RGB readback,
   and its captured frame changed. Wasm frame progression continued.
2. **Explicit Pixel persistence:** selected Pixel through Help / Controls,
   confirmed the saved value was `pixel`, and reloaded the page. The selector
   remained Pixel. Loading the diagnostic produced a lit source canvas and
   continuing Wasm frames, with no CRT overlay attached.
3. **WebGL unavailable:** a separate fresh context made WebGL `getContext`
   requests return `null`, retaining the real Canvas 2D path. Startup selected
   Pixel and exposed the existing "Using Pixel"/CRT retry status. The diagnostic
   produced lit frames and ongoing Wasm progression. ArrowRight press/release
   changed the diagnostic input marker to 1/0. The display preference stayed
   absent.
4. **Loading and errors:** Wasm initialized and the local ROM chooser became
   enabled in both contexts. No page errors, console errors, failed requests or
   HTTP responses with status 400 or above were recorded during these scenarios.

The unavailable-WebGL case is a controlled browser API simulation. Physical
mobile devices and additional native GPU configurations were not tested here.
Broader preference, switching, failure and demo coverage remains in the passing
production-browser CI suite. Optional user-supplied `301 seconds` game tests were
excluded by the existing CI command. No post-merge defect was observed, so no
follow-up task was created. This records a completed verification session, without
background monitoring.

## Published assets

All four requests returned HTTP 200. HTML, JavaScript and CSS SHA-256 values
matched the reviewed worktree files byte for byte. Wasm had the required
`application/wasm` MIME type and initialized during playback.

| Asset | Bytes | SHA-256 |
| --- | ---: | --- |
| `index.html` | 7022 | `4cd8b9d7685a261b1102792edd743cea83e4613f75c826bea8b8462ba493af18` |
| `app.js` | 29079 | `871042da583b4a35506d237329f44070332454b37ef0ae2adf9c8c40b0665e36` |
| `style.css` | 8155 | `3175142a42e43ccfe42c3a9b1d6ef3db37e5abd10ea9cfc935a21c0a347b828e` |
| `snes_wasm.wasm` | 100323 | `a17fdf23fd3fee29085dfafbf59b55b1583d7cc90d50695734fd18b343cfc078` |
