# Synchronize queued-audio and paused CRT probes

Pages verification of the unchanged bundled ROM at `d26a15a` was blocked by two
independent browser failures in run
[44103358166](https://github.com/takahirox/snes-rust/actions/runs/44103358166):

- Attempt 1: 148 passes; the local-diagnostic CRT test read `pixels.lit=false`
  immediately after paused CRT selection (Issue #157).
- Attempt 2: CRT passed, but the queued-volume test read input RMS 0 after its
  40 ms main-thread busy wait; 148 other tests passed (Issue #151).

Both tests use the existing smoke diagnostic, not Yonaki. The ROM, renderer,
audio scheduler, gain smoothing, production CSP and artifact are unchanged here.

## Audio: save real samples before the queue expires

A positive scheduled-source count does not guarantee a non-silent input at the
end of an arbitrary wall-clock delay. The test now suspends the audio context at
a fresh real source start and holds subsequent animation callbacks. After the
suspension completes, it records frame/source counters, dispatches the actual
volume input, and resumes the existing queue.

A test-only AudioWorklet records both stereo taps in the same audio quantum.
It waits seven of the production gain's 5 ms time constants, then saves the first
non-silent measurement without filtering for the desired ratio. The main thread
is deliberately blocked for 250 ms, longer than the normal queue horizon. The
saved measurement must still have non-silent input and a ratio of 0.25, with zero
new core frames or sources and a sample time inside the already queued window.
No synthetic sound, replacement source, gain stage or emulator samples are used.

Observed focused-run evidence:

| Field | Value |
| --- | ---: |
| Pending real sources | 3 |
| New frames / sources | 0 / 0 |
| Gain change audio time | 0.164 s |
| Queued source end | 0.226375 s |
| Saved audio-quantum time | 0.200 s |
| Input RMS / output RMS | 0.0255975 / 0.00640927 |
| Measured ratio | 0.2503867 |
| Stereo channels | 2 |
| Main-thread stall | 250.4 ms |
| Live analyser input after stall | 0 |

This reproduces the finite-window sampling failure mechanism with real queued
audio while preserving correct attenuation evidence. It does not claim the
exact duration of every historical CI stall was measured.

The test server serves the observer at one fixed same-origin path. Production
CSP remains in force. The module stays outside `web/dist`; the Pages artifact
check still requires exactly the six existing production files. All other test
requests use those production artifacts. No test-only module is deployed.

## CRT: wait for observable output

The paused CRT test now waits for non-black source and WebGL pixels and a live
context before taking its comparison sample. It retains all orientation,
brightness, opacity and canvas-stream assertions. Readiness state is attached
even when the wait fails. A persistent black canvas still fails the test.

The original intermittent GPU/readback cause was not reproduced locally and is
not claimed to be a renderer fix. Issue #157 remains a follow-up for that cause.

## Validation

- Both changed tests passed three focused repetitions each: 6 passed (4.9 s).
- The output probes plus the Pages artifact/subpath smoke check passed together:
  3 passed (2.3 s), including the measurements above.
- Web artifact rebuild passes. Its expected file set and the updated Yonaki ROM
  hash remain intact; production source files and both bundled ROMs are unchanged.
- Required full CI and post-merge Pages/live verification are recorded in the PR.
