# Issue #3 delivery checklist and evidence

Source Issue: <https://github.com/takahirox/snes-rust/issues/3>.
**Status: partial implementation, keep the Issue open.** This checkpoint goes
beyond a scaffold: the generic core runs two actual homebrew games in Chromium
with real input-derived graphics and Gothicvania audio. It does not yet
meet all legal audit, timing fidelity and gameplay acceptance requirements.

## Current review fixes

The two cleared gameplay targets are now **modified source builds of Castle
Platformer and Memory Game**, with complete pinned input inventories and exact
ROM hashes in the [audit](../tests/roms/CLEARED-GAMES.md). Castle's unidentified
credits font is removed; original lettering replaces it. Memory is a separate
card-matching game chosen because Gothicvania's runtime/audio provenance remains
blocked. Both are by The UnDisbeliever and share a game library; they are distinct
gameplay programs. Their sources are compiled locally, never supplied as prebuilt
images. The original-release investigations further below remain historical,
optional evidence and do not clear those release images.

Both cleared artifacts pass 18,100-frame browser regressions with assertions
throughout the sample. Castle verifies movement, grounded jumps, active play,
changing video and death/restart. Memory performs complete wins and restarts
using controller input, with cursor/selection/video assertions in every window.
The build runs twice with exact output-hash verification, and CI now builds and
tests both artifacts. The modified Memory Game now includes original MIT sound
effects executed by its uploaded SPC program, with sustained non-silent sample
and interactive Web Audio assertions. Upstream Memory and Castle are silent;
no upstream music-driver compatibility is claimed. Castle finish/credits
progression and hardware-fidelity gaps also remain open.

Failed browser SRAM writes now block ROM replacement and preserve the machine's
recoverable save; Play and Reset retain the warning until saving succeeds.
Failed restorations now block all writes and playing until an explicit retry
succeeds; read, Base64 and size failures have Chromium recovery regressions. Two
Chromium regression tests cover quota/security failures, reset, cartridge
switch/reopen attempts and recovery after storage is restored. Current validation
results are recorded in
[review-audio-save-fixes.json](../tests/results/review-audio-save-fixes.json).
PR #4 references #3 without a closing directive. No push or merge is performed
by this node.

## Implementation

- [x] Platform-independent Rust core, native runner, WebAssembly adapter, static
  frontend, deterministic tests, build instructions and CI configuration.
- [x] Ordinary NTSC LoROM/HiROM loading, copier headers, reset vectors, WRAM/SRAM,
  cartridge metadata, bus mirrors, open bus and basic memory-mapped I/O.
- [x] All CPU opcode decodings, width/emulation modes, decimal arithmetic,
  interrupts, stack/wrapping edge cases and WAI/STP. Licensed semantic suites pass.
- [ ] Hardware-accurate CPU internal timing, IRQ/NMI latency, math-operation
  latency, refresh, DMA/HDMA contention and all memory access ordering.
- [x] Master-clock scheduling, scanline video, VBlank/HBlank, NMI/IRQ, joypad
  serial/auto reads, DMA and direct/indirect HDMA used by current targets.
- [x] Actual planar tile/tilemap/sprite framebuffer rendering; scrolling,
  priority, windows/color math and basic Mode 7 implementation.
- [ ] Validate all PPU features used by the target games against hardware;
  improve register latches/access restrictions, mid-line effects and other
  limitations listed in the README. Additional PPU suites are unrun.
- [x] Firmware-free IPL protocol, SPC700 interpretation/timers/ports, BRR voices,
  stereo audio buffering, simple ADSR/gain and echo. Licensed SPC semantic suite
  passes; Gothicvania emits non-silent audio in the browser.
- [ ] Verify audio fidelity/synchronization. Gaussian interpolation, noise,
  pitch modulation, precise envelopes and key-on delays remain incomplete.
- [x] Local ROM picker, play/pause/reset, keyboard/gamepad controls, integer
  scaling, Web Audio with gesture activation and visible errors.
- [x] Battery SRAM persistence, reset semantics and browser restoration tests.
- [x] Bounded headless runner, input replay, markers/terminal PC assertions,
  frame hashes/PPM export, bounded CPU/SPC traces and useful failures.
- [x] Audit two exact modified ROM/build/art/font inventories and source-build
  both games reproducibly; keep the unaudited original releases ignored/local.
- [x] Audit modified Memory Game with original game-event sound effects and
  require non-silent samples and interactive Web Audio playback in CI.
- [ ] Establish sufficiently representative sustained gameplay coverage and
  independently verified transitions, accounting for idle/death/restart time.

## Reproducible local validation

See [README commands](../README.md#validate) and the machine-readable
[validation record](../tests/results/validation.json). This revision does not
push, merge, deploy or close the Issue. PR #4's closing directive was removed;
it references #3 as partial work.

- Format and Clippy with warnings denied: pass (Rust 1.99; core also compiled
  and tested with Rust 1.94). Workspace unit/integration/doc checks: pass,
  including 16 integration tests. New counting-bus instruction regressions cover
  direct-page indexing with zero/nonzero indexes, page-crossing and 16-bit-index
  read penalties, fixed indexed store/RMW cycles, stack-relative indirect Y, and
  unpenalized long indexing. Native release build: pass.
- WebAssembly release/static production build: pass. Chromium checks: 7 pass
  when both optional game images are supplied (two long replays and two late
  freeze negative controls). Those are historical original-release results.
  Current CI runs eight diagnostic/UI/SRAM tests, two cleared source-built
  gameplay tests and one interactive cleared-game audio test; it never downloads
  prebuilt game images.
- Original 32 KiB diagnostic reproduces byte-for-byte, exercises actual DMA,
  planar graphics, frame counter, controller input, uploaded SPC execution and
  non-silent BRR audio. Browser tests assert keyboard and virtual gamepad
  markers, changing pixels, audio buffers, pause/reset, scale, invalid ROMs,
  SRAM reload and copier-header-independent save identity. No network upload.
- Source-built gilyon full CPU suite: 1,610 cases, final test `$0649`, terminal
  PC `$00:81a2`. Basic CPU suite: 1,107 cases, final `$0452`, same terminal PC.
  SPC700 suite: 1,368 cases, final `$0557`, terminal PC `$00:8060`.
  Both last-test markers and terminal PCs are asserted, not merely elapsed
  frames. These suites explicitly do not verify instruction timing or DSP.
- Source-built zlib HDMA timing demonstration: 60 frames; 11 grayscale bands
  change every 16 scanlines, last shade persists through scanline 223. Checks
  cover three x positions on every visible row. It verifies scanline HDMA
  sequence/line counters, not every bus edge or DMA/HDMA conflict.
- Every headless invocation has frame and wall limits. Browser tests have
  30-second smoke / 180-second per-game timeouts, with 120-frame yielding.

The assemblers used for final source-suite checks were rebuilt from the exact
hash-pinned archives using `tools/build-test-tools.py`. The auxiliary SPC tool
was compiled with Rust 1.99 and its experimental-feature flag, without its
separate emulator package. No external emulation implementation is used.

## Game investigations

Both images were loaded from reset into the **same unmodified emulator core**.
Game-specific logic exists only in input/snapshot tests, never in the core.
ROM hashes are checked before execution; per-window state/video/audio evidence
and aggregate audio metrics are in the validation record. Images themselves and
screenshots remain ignored because full binary/build audits are pending;
see [provenance](../tests/roms/PROVENANCE.md).

| Sample | Emulated duration | Observations | Audio |
| --- | --- | --- | --- |
| Gothicvania v1.0.0 | 18,100 frames / 301.17 s | 20 windows with movement and grounded jumps; 22 title→play→game-over→title cycles | Non-silent in every window; finite samples |
| Castle Platformer v1.04 | 18,100 frames / 301.17 s | 20 windows with movement and grounded jumps; 40 death/restart cycles | Silent in this demo; not used for audio acceptance |

The revised replay presses Start at residues 60–61 modulo 120 (both Game Over
and title require fresh presses in Gothicvania), right during residues 120–779
modulo 900, B during 30–41 modulo 90, and Y during 85–89 modulo 120. Each of the
20 complete 900-frame windows must contain at least 20 small positive player
displacements under Right, one grounded-to-airborne B jump with upward velocity,
90 active gameplay frames, and 20 distinct rendered hashes. Gothicvania must
produce non-silent finite audio in every window. Source/address derivations are
recorded in [provenance](../tests/roms/PROVENANCE.md#replay-marker-derivation).
The old first-900-frame golden assertions were replaced by these source-derived
checks throughout the sample. Negative controls freeze the observed player X
after frame 900 while the real core/video/audio continue, and verify rejection
by the movement assertion at frame 1799 for both games.

Gothicvania's checked windows contain 12,833 active frames (about 213.5 s),
66–488 movement frames and 1–6 grounded jumps per window; Castle's contain
13,881 active frames (about 231.0 s), 478–537 movement frames and 6–8 grounded
jumps per window. Death/title periods remain; this is **not** 301 seconds of
continuous active play. Neither test may pass using only its early progress.

After the first 900 frames, audio buffers must contain 960–1170 interleaved
samples and cumulative sample/raster drift must remain within 128 samples
(2 ms at 32 kHz stereo). Observed maximum drift is 72.44 samples (1.14 ms)
for Gothicvania and 2.09 samples (0.04 ms) for Castle. Boot DMA is excluded from
this cumulative bound. This bounds the core's internal audio/video cadence,
not its fidelity to a reference machine or the frontend's audible latency.
These are accelerated core/framebuffer runs in real Chromium, not real-time
interactive listening sessions. The separate browser smoke checks actual Web
Audio scheduling using our cleared diagnostic.

Gothicvania's Game Over screen and Castle's death/restart were observed.
Gothicvania's finish screen, Castle's credits and later maps were **not** reached
or verified. No commercial ROMs, PAL or enhancement chips were tested. There is
no hardware/reference audio comparison and no proof of absence of all
uncontrolled desynchronization. Physical gamepads and other browsers are unrun;
a standard gamepad is simulated in the automated UI test.

## Remaining release requirements

1. Add a cleared game with working music/sound. The two audited modified builds
   resolve the two-game licensing blocker through source builds and replacements,
   but both are silent. Gothicvania's original runtime/audio/build terms and the
   original Castle credits/build match remain unresolved; neither original
   release is cleared. Do not use their optional runs for licensed audio
   acceptance. The [new audit](../tests/roms/CLEARED-GAMES.md) specifies exactly
   which modified artifacts are approved and why.
2. Extend progression beyond the currently asserted recurring movement/jump and
   death/restart paths, including reachable finish/credits behavior. Per-window
   progress and active-duration accounting now pass, but no reference hardware
   comparison or full level traversal has been performed.
3. Resolve timing/audio/PPU deviations that affect those paths. Continue running
   additional licensed timing fixtures and report failures explicitly. The
   passing CPU suites are semantic evidence, not cycle accuracy.
4. Run the configured Linux CI gates on GitHub when a later workflow is
   authorized to push/open a PR. This node only prepares and commits files.

Do not use this checkpoint's green checks to close Issue #3 or describe it as
broad commercial-game compatibility. No post-merge deployment is required.
