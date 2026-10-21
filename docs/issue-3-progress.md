# First-playable delivery checklist: #3 continued in #5

Original requirements: [#3](https://github.com/takahirox/snes-rust/issues/3).
Active completion tracker: [#5](https://github.com/takahirox/snes-rust/issues/5).
#3's closure records a tracking handoff, not completion of PR #4.

The first-playable implementation now has target-specific timing, PPU, numerical
DSP and real Web Audio synchronization evidence in addition to sustained gameplay.
The [validation record](issue-5-validation.md) describes exact checks and limits.
Remote CI/review for the proposed commit must still be checked before merge;
this worktree node is prohibited from pushing or opening a PR. Passing the old
PR's CI is not evidence for a new commit.

## Original implementation requirements

| #3 item | Current implementation and reproducible evidence |
| --- | --- |
| Workspace/core/native/Wasm/frontend; no imported emulator | `crates/snes-core`, `crates/snes-wasm`, `web`; dependency-free Rust workspace; format/lint/tests and production build |
| Cartridge/header/mappers/reset/WRAM/SRAM/open bus | `cartridge.rs`, `machine.rs`; cartridge/mirror/open-bus and copier-header/save-identity regressions; ordinary NTSC LoROM/HiROM |
| 65C816 modes/opcodes/flags/stack/interrupts/WAI/STP/tested bus timing | `cpu.rs`; all 256 decodings; licensed full/basic suites; independent counting-bus addressing, RMW, jump/call/stack and interrupt tests; machine boundary tests |
| Master-clock CPU/PPU/DMA/HDMA/APU; blanking/interrupts/timers/controllers | `machine.rs`; refresh hold, 8/16 CPU-cycle math latency, HDMA preemption, DMA read/write phases, NMI across DMA, autojoy completion; H/V/combined IRQ comparisons; licensed raster fixture |
| Target PPU graphics/registers/raster effects | `ppu.rs`; Mode 1 BG/scroll/OAM/palette rendering through actual guest code; VRAM prefetch/remap/write gate, write-pair and counter/bus latch tests; diagnostic, licensed HDMA bands and game framebuffer/progression assertions |
| SPC upload/execution/timers/DSP/music/effects | `apu.rs`, `dsp.rs`; licensed SPC suite; exact clock/chunk determinism; BRR/interpolation/gain/stereo/key/release vectors; recovered Memory score/effects through Wasm and Web Audio |
| Browser picker/controls/scaling/errors/gesture/audio sync/privacy | `web/app.js`; real Chromium diagnostic/UI checks and Memory queue/drift/stall/repeat checks; local files only |
| Battery-backed saves/session/load/reset semantics | Browser SRAM persistence, copier-header identity, quota/security/read/corrupt-size failures and recovery tests; reset preserves SRAM |
| Bounded runner/debugging/traces/replay/frame and state assertions | `snes-run`; CPU/SPC traces, RAM/terminal-PC assertions, PPM, hashes, frame/wall deadlines; fixed controller recordings |
| CI and accurate developer documentation | `.github/workflows/ci.yml` runs format/lint/Rust, pinned source fixtures, repeated game builds, Wasm and real Chromium; README commands and this mapping |

## Original pre-merge acceptance

| #3 criterion | Evidence |
| --- | --- |
| Both targets compile; documented gates pass | Workspace checks, native release and Wasm/static build; local equivalents of both CI jobs in the final validation record |
| At least two distinct, fully audited actual games in one browser core | Exact modified Castle and Memory builds, unchanged SHA-256; [complete audit and independent-game decision](../tests/roms/CLEARED-GAMES.md#independent-game-selection-decision-for-3--5) |
| Reset-to-gameplay, sustained inputs/changing video, >=5 minutes each, progression | 18,100 frames per game, 20 independently asserted 900-frame windows; Castle movement/jumps/death/restart plus all five maps/credits; Memory card/match/win/restart; explicit bounds and late-freeze negative control |
| Game-generated non-silent music/effects and interactive playback | Memory's uploaded SPC driver/score; recovered ordered notes/rests/durations and finite independent card effects; real gesture-activated Web Audio queue/drift/stall/pause/reset/repeat assertions |
| Licensed CPU/SPC and at least one PPU/DMA timing fixture | gilyon full/basic/SPC terminal PCs and last markers; source-built zlib HDMA framebuffer bands |
| CI real-browser run, deterministic/time-bounded headless tests | Mandatory cleared-game browser suite in CI; runner frame/time limits; 180-second long browser tests; pinned source/ROM hashes |
| Keyboard/gamepad/play/pause/reset/privacy/scaling/errors/audio activation | Diagnostic UI suite with real Wasm and Web Audio; simulated standard gamepad; local-file network checks |
| No emulator engine or unaudited ROM/assets; notices/legal-user-ROM guidance | Rust dependency inventory; pinned input allowlists/licenses; generated per-ROM notices; original sound/font code; README/provenance |
| Accurate compatibility/limitations | Target inventory and optional hardware section in [#5 validation](issue-5-validation.md); README; no commercial/PAL/chip compatibility claim |

## #5 reconciliation

The old checklist mixed target requirements with full hardware fidelity. That
made passing game tests insufficient and left concrete bugs unaddressed. #5 adds
implementation and independently expected regressions for math latency, refresh,
DMA/HDMA interleaving/access order, PPU latches/access, Gaussian interpolation,
key-on delay, numerical sound and browser queue/drift recovery. It preserves the
already validated core, fixtures, game sources, hashes and all UI/save coverage.
The source inventory establishes which hardware paths the selected artifacts
actually require; unsupported features are optional only when unused by them.

All current evidence is linked from [issue-5-validation.md](issue-5-validation.md).
The older `tests/results/validation.json` and `review-*.json` records describe
historical PR #4 revisions and their then-partial status; they are not claims
about this revision. No original-release investigation is substituted for a
cleared game. No post-merge verification/deployment gate is required.
