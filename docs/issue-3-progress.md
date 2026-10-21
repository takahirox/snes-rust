# Issue #3 delivery checklist and evidence

Source Issue: <https://github.com/takahirox/snes-rust/issues/3>.
**Status: partial implementation, keep the Issue open.** This checkpoint goes
beyond a scaffold: the generic core runs two actual homebrew games in Chromium
with real input-derived graphics and Gothicvania audio. It does not yet
meet all legal audit, timing fidelity and gameplay acceptance requirements.

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
- [ ] Finish both games' entire ROM/build/art/audio/font/license audit before
  bundling or automatically fetching. Their release ROMs remain ignored/local.
- [ ] Establish sufficiently representative sustained gameplay coverage and
  independently verified transitions, accounting for idle/death/restart time.

## Reproducible local validation

See [README commands](../README.md#validate) and the machine-readable
[validation record](../tests/results/validation.json). No CI run was triggered
and no PR, push, comment, merge, deployment or Issue closure was performed.

- Format and Clippy with warnings denied: pass (Rust 1.99; core also compiled
  and tested with Rust 1.94). Workspace unit/integration/doc checks: pass,
  including 13 integration tests. Native release build: pass.
- WebAssembly release/static production build: pass. Chromium checks: 5 pass
  when both optional game images are supplied. Default CI runs the 3 cleared
  diagnostic smoke tests and does not download external games.
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
ROM hashes, reviewed framebuffer hashes and aggregate audio metrics are in the
validation record. Images themselves and screenshots remain ignored because
full binary/build audits are pending; see [provenance](../tests/roms/PROVENANCE.md).

| Sample | Emulated duration | Observations | Audio |
| --- | --- | --- | --- |
| Gothicvania v1.0.0 | 18,100 frames / 301.17 s | Title → Start → right movement → B jump / Y attack → Game Over; repeating input cycle restarts play | >11 million nonzero interleaved samples; RMS about 0.186 |
| Castle Platformer v1.04 | 18,100 frames / 301.17 s | Starts in gameplay; right movement / B jump; death and button-driven restart cycles | Silent in this demo; not used for audio acceptance |

Each recorded 900-frame input cycle presses Start at 180–181, right during
240–599, B during 340–359 and 520–539, and Y during 400–409; it repeats for the
full sample. The browser asserts reviewed reset/title, gameplay, jump and death
frame digests for the exact ROM versions, >100 distinct frame hashes, and
non-silent finite Gothicvania samples. Death/end screens create idle periods;
301 seconds of total emulation is **not** 301 seconds of continuous active play.
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

1. Audit the exact two release images' embedded sound/font/runtime content and
   toolchain/submodule provenance, ideally reproduce them from cleared source.
   Record every artifact hash and all notices. Replace a candidate if clear
   redistribution rights cannot be established. No general MIT assumption.
2. Strengthen gameplay replays to account for active play duration, assert useful
   interactions/transitions against independent source/hardware evidence, and
   cover reachable finish/credits behavior where practical.
3. Resolve timing/audio/PPU deviations that affect those paths. Continue running
   additional licensed timing fixtures and report failures explicitly. The
   passing CPU suites are semantic evidence, not cycle accuracy.
4. Run the configured Linux CI gates on GitHub when a later workflow is
   authorized to push/open a PR. This node only prepares and commits files.

Do not use this checkpoint's green checks to close Issue #3 or describe it as
broad commercial-game compatibility. No post-merge deployment is required.
