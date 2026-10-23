# Issue #5 first-playable validation

This record supersedes PR #4's partial-release descriptions. [#3](https://github.com/takahirox/snes-rust/issues/3)
was closed as a handoff; [#5](https://github.com/takahirox/snes-rust/issues/5)
is the completion tracker. The [delivery checklist](issue-3-progress.md) maps
every original implementation and pre-merge criterion to code and evidence.
Machine-readable results are in [issue-5.json](../tests/results/issue-5.json).

## Target inventory and selection

The cleared games are the exact modified source builds in
[CLEARED-GAMES.md](../tests/roms/CLEARED-GAMES.md). Their source allowlists, linked
modules, immutable commits, per-input hashes, ROM hashes and generated notices
are unchanged. Two fresh builds reproduce both ROMs. The audit now explicitly
accepts independently developed **game programs**, without inventing a different-
author/library gate. Castle's map/entity platformer and Memory's card/door/match
puzzle have separate gameplay sources, assets and progression. Memory replaces
Gothicvania for documented unresolved runtime/audio/build rights, not to bypass
a failing test. No original-release ROM or unverified asset is acquired.

The pinned `src/interrupts.s` in each game saves/restores native CPU state and
acknowledges RDNMI. Castle's `src/gameloop.s` and Memory's `src/main.s` enable
NMI and automatic joypads (`$81`), not timer IRQs. Their controller routines use
autojoy completion. Memory's linked `routines/random.s::Rnd_U8A` uses hardware
division and waits while advancing its RNG. Transfers run in forced blank or
VBlank. Neither program enables HDMA or relies on CPU writes within a visible
scanline. The separate licensed fixture exercises guest-programmed HDMA raster
changes; an original regression additionally forces HDMA/DMA contention.

Both games select Mode 1. Castle uses a scrolling 4bpp metatile BG, animated OAM
entities, palettes/priorities and a tile-based credits screen. Memory uses two
4bpp BGs for cards/doors, shared scroll latches and OAM cursor/text. Their game
rendering and transformations execute through the generic CPU, bus and PPU.
No program identifier or per-game exception exists in the core or frontend.

Castle is silent. Memory's original SPC extension uses ports, timer 0 for
finite effects, timer 1 for a looping score, filter-0/range-9 looping BRR,
direct gain, independent pitch and signed stereo volumes. It disables echo
writes and uses neither noise, pitch modulation nor ADSR. These are concrete
source-derived requirements, not conclusions inferred from non-silent samples.

## Timing and PPU changes and independent expectations

- Math results commit after 8 multiply / 16 divide CPU bus/internal cycles,
  including quotient side effects and divide-by-zero. Tests vary fast ROM,
  slow ROM, WRAM and joypad access speed. Arithmetic outputs are independently
  calculated (13 × 17 = 221; 1000 / 7 = 142 remainder 6).
- A 40-master-clock refresh hold advances the APU/raster without completing
  another CPU operation. The deterministic phase is H=538; CPU-revision phase
  variation is not modeled. A read straddling it takes 48 rather than 8 clocks.
- Pending bus events run at CPU bus/internal-cycle boundaries. DMA separates
  source sampling and destination commit into two four-clock phases. A write
  crossing line 224→225 succeeds in VRAM; 261→0 is blocked. This regression
  checks the access order, not just aggregate frame timing.
- HDMA preempts general DMA between bytes. An original brightness table must
  render consecutive white/136-gray/17-gray rows during a 600-byte DMA (also across frame initialization with 800 bytes); deferring
  all HDMA until DMA completion loses those rows. Licensed guest HDMA additionally
  produces eleven independently calculated grayscale bands on every visible row.
- NMI generated during DMA remains pending while CPU state is held and the APU
  advances, then enters once at the CPU boundary. Existing counting-bus tests
  verify the seven/eight entry cycles, stack order and RTI. Other tests cover
  NMI enable edges/status acknowledgement, H/V/combined IRQ comparisons across
  rollover, masked IRQ/WAI and the 4,224-clock automatic-controller busy period.
  Completed joypad values are published at the end of that period.
- VRAM rejects active-display/HBlank writes while still incrementing its address;
  forced/VBlank transfers work. Tests verify prefetch/duplicated-first-word sequence,
  qualified by the [Issue #41 contract](issue-41-validation.md#evidence-and-contract-recorded-before-implementation):
  an accessible address write makes the first word valid immediately; a dummy
  read discards one duplicate. These old write-gating assertions did not verify
  restricted-period reads. The later record adds read-specific evidence and
  explicitly identifies the invalid-fetch buffer approximation. Tests also cover
  remapping/increments, OAM paired writes/high-table mirroring, CGRAM pairs/open
  bit, and held H/V counter values with independent read flipflops and status
  acknowledgement. Existing rendering tests cover 16×16 tile wrapping/flips,
  subscreen/backdrop math and actual diagnostic framebuffer execution.

Expectations come from the [WDC bus-cycle tables](https://www.westerndesigncenter.com/wdc/documentation/w65c816s.pdf),
[Fullsnes hardware register specification](https://problemkaputt.de/fullsnes.htm),
[SNESdev timing](https://snes.nesdev.org/wiki/Timing) and pinned fixture sources.
No emulator implementation was imported. `gaussian.rs` contains only the S-DSP
hardware's numerical coefficient data transcribed from the register reference;
all processing code is original Rust.

## Numerical sound and browser synchronization

Linear interpolation is replaced with the four-tap hardware Gaussian table,
including partial-sum wrap, final saturation and even-sample truncation. Unit
vectors check each phase-zero impulse (370, 1304, 374, 0), reversed endpoints,
and the documented negative full-scale overflow (32752). BRR tests independently
check all signed nibbles, invalid ranges and looping. Direct gain/stereo tests
calculate settled PCM levels (3500, −3529), five silent key-on samples, release
by eight envelope units per sample, retrigger and mute. KON register readback is
preserved. APU tests require exactly 32,000 stereo frames per master-clock second,
identical samples across whole/split tick batches, finite normalized samples and
numerically expected timer ticks. These check sound values and duration, beyond
counting that samples exist.

In Chromium, Memory's existing waveform test recovers repeated score pitches,
rests and 12–16-frame quarter-second notes from PCM zero crossings; it verifies
that a controller-triggered card effect ends while music continues. It emits
numeric JSON and a stereo WAV. The five-minute replay separately checks music
and effects in every window; both games bound cumulative sample/raster drift to
128 interleaved samples (2 ms) after startup.

The frontend now derives its frame period from the same master clock/raster,
caps stale video debt at three frames, and bounds audio schedule endpoints to
150 ms ahead with at most 12 live sources. An underrun/stall rebases both video
wall time and audio scheduling. It intentionally discards stale wall time rather
than attempting an endless catch-up after a blocked main thread.

The real Web Audio test observes actual `AudioBufferSourceNode.start/stop`
operations without replacing audio buffers or the device clock. It requires
finite normalized samples, nonnegative scheduling lead, <=151 ms endpoint lead,
<=12 live sources, and generated-audio/device-clock duration difference <120 ms
in each four-second interval, including recovery from a 400 ms main-thread stall.
It verifies user-gesture activation, stopped sources on pause/reset, no paused
scheduling, twice-repeated playback and a silent reset title. Separate keyboard
checks verify idle music across a full score loop and finite card effects reaching
Web Audio. The limits are executable assertions, not a subjective listening gate.

## Gameplay, bounds and final commands

Each required game starts from reset and runs 18,100 frames (>301 emulated
seconds), with 20 complete 900-frame windows that must independently demonstrate
controller interaction, active play and changing guest-generated framebuffer
hashes. Castle requires movement and grounded jumps throughout, plus death/
restart; a negative control freezes observed player X after a valid window and
must fail the following one. Memory selects shuffled cards only with emulated
joypad presses, wins and restarts repeatedly and requires music/effects throughout.
It reads the shuffled layout to choose moves; it does not change game memory.
These runs include title/load/death/restart time, not 301 seconds of uninterrupted
active gameplay. Each has a 180-second wall limit and regular yielding.

Castle's separate fixed-input route traverses all five maps, operates chain and
bridge switches, reaches `YOU WIN` and completes the credits scroll at row 287.
It has 20 state/RAM/framebuffer checkpoints and rejects any death. The inputs,
ROM and all later hashes are unchanged. The former frame-6099 checkpoint sampled
partially initialized player coordinates inside `LoadNewMap`; it moves to frame
6120, after initialization, checking map 3/state 0, (280,1525) and hash `c1fe1581`.
The complete recording remains 11,097 frames. This is a more stable gameplay
checkpoint, not a tolerance or a skipped progression assertion.

Run the exact [README commands](../README.md#validate). Final local gates include:
format; Clippy with warnings denied; Rust unit/integration/doc tests; original
fixture reproduction; pinned assembler builds; repeated cleared-game builds;
licensed full/basic/SPC suites and HDMA fixture; native release; Wasm/production
build; all mandatory Chromium tests, including UI/privacy/scaling/gamepad/errors
and SRAM persistence/recovery. `SNES_TEST_PORT=18085` avoids an occupied default
port without using another process's server. No optional unaudited ROM is used.
Detailed results/hashes and durations are retained in the JSON validation record.

The prior PR head's GitHub run [39575513039](https://github.com/takahirox/snes-rust/actions/runs/39575513039)
passed. That is historical evidence only. This node executes the configured job
commands locally and commits files; its explicit authorization prohibits push,
PR creation, merge and comments. GitHub CI/review must run on the proposed commit
before merge in the downstream authorized workflow. No live deployment or
post-merge confirmation is a delivery condition.

## Optional hardware limits

This delivery verifies the selected programs and licensed fixtures, not all
SNES hardware/catalog behavior. Unused features remain optional: enhancement
chips, PAL, extended mappers, save states, hires/interlace/overscan, offset-per-tile
modes, exact object overflow/rotation, Mode 7 edge cases, dot-level raster fetches,
active-display OAM/CGRAM corruption, every CPU-revision IRQ/NMI/refresh phase,
SPC IPL instruction execution, noise/pitch modulation, exact ADSR rate phase,
DSP subcycle polling and reference echo saturation. The selected source inventory
and gameplay/audio tests establish why these do not mask a missing target path.
Future failures affecting a mandatory target require implementation, not another
limitations waiver. Physical gamepads, other browsers, subjective listening,
proprietary ROMs and hardware/reference comparisons are optional.
