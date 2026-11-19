# Issue #166 validation

The Pages run at `9b43e24` passed Core, reported 177 passed and two failed in
Browser, and skipped Deploy. GitHub lists zero artifacts for the run, so the
original `paused-crt-readiness` payload was unavailable.
[Browser job, attempt 3](https://github.com/takahirox/snes-rust/actions/runs/44254054644/job/135253359565)

## Paused CRT

On the baseline, ten ordinary focused repeats passed. To isolate the failure,
a diagnostic waited until the 2D source went black after frame 10 and clicked
Pause in that same browser task. The original readiness assertion then timed
out in all three repeats, including at guest counters 33 and 65. The smoke ROM's
NMI sets the foreground palette entry to the frame counter masked with 31, so
it periodically renders a genuinely black frame.

```json
{"frames":33,"guestFrame":33,"wasmLit":false,"sourceLit":false,"outputLit":false,"sourceSize":[256,224],"outputSize":[768,672],"drawingBufferSize":[768,672],"contextLost":false,"glError":0}
```

A black frame stayed black while paused. The updated test begins in Pixel,
samples the lit 2D source and pauses in one task, then selects CRT. Selection
submits a draw before revealing the CRT canvas. WebGL readback checks that draw,
and canvas capture receives a fresh presentation while the guest stays paused.
The test retains the original orientation, brightness, opacity and capture checks.
This reproduces the black readback reported in [#157](https://github.com/takahirox/snes-rust/issues/157).
The historical failure lacks the framebuffer data needed to identify its cause.

## Touch fullscreen exit

On Ubuntu Chromium, the original desktop Escape test passed all three repeats
and the touch test failed all three. Touch clicks on both buttons had
`detail=1`. The failing Enter keydown path began with
`#exit-fullscreen → #game-presentation → SECTION → MAIN → BODY`.

| Event | Active element | Fullscreen element | Playing | Keyboard / virtual / combined mask |
| --- | --- | --- | --- | --- |
| Touch click to exit | `#exit-fullscreen` | `#game-presentation` | true | `0 / 0 / 0` |
| Enter keydown | `#exit-fullscreen` | null | true | `0 / 0 / 0` |
| Enter-generated click, `detail=0` | `#exit-fullscreen` | null | true | `0 / 0 / 0` |
| Exit `fullscreenchange` begins | `#exit-fullscreen` | null | true | `0 / 0 / 0` |
| Re-entry `fullscreenchange` begins | `#video` | `#game-presentation` | true | `0 / 0 / 0` |

The guest read `0x0000`. Enter activated the still-focused Exit button before
`fullscreenchange` handed focus back, and the toggle requested fullscreen
again. The app now focuses the canvas before requesting exit after pointer
entry and guards the Exit button against re-entry. Keyboard entry still returns
focus to the Fullscreen button. Regression coverage exercises touch and mouse
Exit clicks, Escape, repeated transitions, Start `0x1000`, keyup, and Enter
activation of focused controls.

## Local checks

- Browser reproduction and validation used Playwright 1.61.1 and Chromium
  revision 1228 on Ubuntu 24.04 ARM64 with Node 24 and Rust 1.99.0. The failed
  Pages job used Ubuntu 26.04 x86-64.
- Five repeats of eight focused cases passed (40 passed, zero failed or
  skipped), including Pixel fallback for unavailable and lost WebGL.
- The first full browser run reported 179 passed and one failed. The existing
  portrait Tab-order test used canvas focus as its exit-completion signal.
  Canvas focus now returns before the toolbar becomes interactive, so the
  Tab-order test waits for the restored toolbar. The Start test still sends
  Enter immediately after the fullscreen element clears.
- The final Chromium gate (`--grep-invert '301 seconds'`) reported 180 passed,
  zero failed or skipped, including all 17 Pages tests. The four optional
  user-supplied ROM cases were outside the gate. The paused CRT readback
  showed a lit 256×224 source and a lit, opaque 768×672 drawing buffer with
  brightness ratio 0.980, mean channel error 19.03, and no WebGL error or
  context loss. GPU canvas capture passed while the guest remained paused.
- The production Wasm and site build passed. Two builds of the cleared ROM
  sources produced the same pinned hashes. The original smoke ROM reproduced
  byte-for-byte, and the packaged demos matched their pinned hashes.
- Core formatting and Clippy passed. The workspace suite reported 519 passed,
  zero failed or ignored. The pinned assemblers built successfully, and
  `cputest-full`, `cputest-basic`, `spctest`, and the HDMA fixture passed.
  The external assembler download failure in
  [#159](https://github.com/takahirox/snes-rust/issues/159) did not recur locally.

The PR can link this evidence in the update to #157. The new main Pages run
and live CRT, Pixel, fullscreen, and controller checks are post-merge work.
