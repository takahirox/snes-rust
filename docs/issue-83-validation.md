# Issue #83: SPC700 live timer targets

## Specification reconciliation (before implementation)

Inspected on **2026-10-24**, before changing the timer implementation:

- [nocash Fullsnes](https://problemkaputt.de/fullsnes.htm#snesapuspc700ioports),
  headed “extracted from no$sns v1.5”: **SNES APU SPC700 I/O Ports**,
  entries **00F1h - CONTROL**, **00FAh/00FBh/00FCh - T0DIV/T1DIV/T2DIV**,
  **00FDh/00FEh/00FFh - T0OUT/T1OUT/T2OUT**, and **SNES Timings / APU
  Timings**. The divider passage says “01h..FFh=Divide by 1..255, or
  00h=Divide by 256”; the output passage says “reset to 0 after reading”.
  It documents four output bits, zero upper bits, timer enable, and clock
  divisions of 128/128/16 SPC cycles. It does **not** settle the live comparator
  or internal wrap width. Its CONTROL reset shorthand differs from Anomie's detailed enable/scaler
  account; disabled-divider phase and enable-edge alignment are outside this
  change.
- [Anomie's SPC700 Doc](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/spc700.txt),
  **revision 1126, 2007-04-21 15:07:05 -0400**, **SPC700 REGISTERS**, entries
  **$00f1** and **$00fa–$00ff**. This independently attributed hardware research
  credits **TRAC** for the three timer stages. Its decisive description is
  “0-255 wraparound counter and a post-increment comparator”. It explicitly
  specifies equality of the incremented stage with the target, clearing that
  stage on a match, and an output tick. Its running-target example says that
  changing target from stage 2 to value 1 waits through wrap. Thus a target
  write retains the counting stage. The continuously running first scaler and
  separately read-cleared four-bit output support retaining phase and output
  on target writes, without a write-generated event. The document warns of a
  target-write race just after a scaler tick, attributing it to blargg; this
  change makes no claim about that edge. No emulator implementation or output
  is used as an oracle.

The reconciled running-timer contract is therefore the Issue's candidate:
increment the **eight-bit** stage modulo 256 on each enabled divider pulse,
then compare for **equality** with the current eight-bit target. A match clears
the stage and increments output modulo 16. Zero matches on wrap. Noncoincident
target writes change only the target; output reads clear only output. The
divider's existing enable/start approximation is retained, not validated as
physical hardware timing. Instruction accesses still execute as whole
instructions; physical read/write edges and the documented race remain open.

The following are **derived arithmetic**, not hardware captures: the positive
distance is `(target - stage) mod 256`, with zero distance meaning 256 pulses.

| Old target | Stage | New target | Pulses until first event |
| --- | ---: | ---: | ---: |
| 200 | 150 | 100 | 206 |
| 200 | 150 | 150 | 256 |
| 100 | 50 | 200 | 150 |
| 0 | 254 | 1 | 3 |
| 200 | 150 | 0 | 106 |
| 0 | 250 | 255 | 5 |
| 200 | 150 | 200 | 50 |

## Validation record

The production correction changes only `timer_stage` from `u16` to `u8`,
uses `wrapping_add(1)`, and compares the result directly to the live target with
`==`. Target writes, output reads, divider scheduling and the existing fixed
cadence assertions are unchanged.

[`spc_timers.rs`](../crates/snes-core/tests/spc_timers.rs) contains six original
public-API tests. The fixed matrix covers all three timers, targets 1/255/0,
and three consecutive event boundaries. The live matrix covers all seven table
rows on all three timers at divider phases 4 and 10: **42 scenarios**, each
with three consecutive boundaries. Stages are reached by ordinary enabled
counting after two complete old periods, preserving two accumulated outputs
across the target write. No private timer state is assigned. Checks at boundary
minus two SPC cycles and at the boundary are the adjacent checkpoints supported
by the existing two-cycle firmware-free idle path; they do not claim
single-cycle physical access accuracy. The phases avoid coincident accesses.

The output matrix checks 15/16/17/31/32/33 accumulated events for each timer and
each fixed target, read-to-clear, and continued counting at the next original
boundary after a read six cycles into a period. The independence control runs
all three timers, changes only T2's target and reads different outputs at
different times, checking that T0/T1 keep their accumulated outputs and phases.

### Original uploaded guest and independent windows

The original guest uses the existing firmware-free uploader to load generated
instruction bytes at $0200. It writes T0/T1/T2 targets 200/255/0, enables them,
then lowers T0 to 100 at elapsed **19205 SPC cycles**: stage 150, divider phase
5 in the existing API. Each delay block is independently counted as
`MOV Y,#n` (2 cycles) plus `n-1` taken `DBNZ Y` (6 each) and one final `DBNZ Y`
(4), totaling **6n**. The guest never polls a timer to measure its own delay.
MOV target-immediate takes 5 cycles; reading an output and storing it to RAM
takes 3+5 cycles. The target write's five-cycle instruction lies wholly within
the slow divider interval, even allowing the instruction's physical write
point to differ from the current whole-instruction model.

T0's first new event is at absolute pulse 356 / cycle **45568**, followed by
pulse 456 / cycle **58368**. T1 retains period **32640** and T2 period **4096**.
The guest's timer read times and independently derived records are:

| Row | T0 read cycle | T1 read cycle | T2 read cycle | Required output triple |
| --- | ---: | ---: | ---: | --- |
| 1 | 21610 | 21618 | 21626 | 0, 0, 5 |
| 2 | 33634 | 33642 | 33650 | 0, 1, 3 |
| 3 | 46858 | 46866 | 46874 | 1, 0, 3 |
| 4 | 60082 | 60090 | 60098 | 1, 0, 3 |

All read windows are hundreds of cycles away from an output event, tolerating
the existing instruction granularity. Records occupy $1000–$100B; port 0 = $83
and a halted CPU must be observed by **1,300,000 master clocks after start**.
Every upload acknowledgement is bounded by 1000 calls of 21 master clocks;
the complete upload must take fewer than 100,000 master clocks. Guest loops
are bounded by construction (no timer-driven wait loop). No assembler or
external executable bytes are required. See the
[MIT authorship/provenance record](../tests/roms/PROVENANCE.md#issue-83-original-spc700-timer-vectors-and-guest).

Whole and irregularly partitioned runs use identical start commands and fixed
master-clock checkpoints **480000, 740000, 1010000, 1300000** after upload.
At each checkpoint both runs separately require the completed independent
record prefix, FLG=$20, master-left volume=37, and silence with no keyed voices.
They also compare every PCM sample, all 128 DSP registers, all 64 KiB RAM,
input/output ports and SPC PC. Partition equality establishes determinism;
the explicit timer records establish behavior. Existing numerical audible DSP
and Memory sound regressions remain required separately.

### Actual baseline behavior

Before changing production code, `cargo test --locked -p snes-core --test
spc_timers` compiled and ran against the unmodified timer implementation from
**4f7ab0d8b36b390a7316eb6a968e3ec86eb37418** (the intervening GitWeave commits
contain execution metadata). **Three passed, three failed behaviorally**:

- Fixed targets, modulo accumulation/read continuation, and timer independence
  passed.
- The live-target test failed for T0, old target 200, stage 150, new target 100,
  phase 4, just before the required first event: output **3**, required **0**.
- Both guest tests failed on row 1: actual **[1,0,5]**, required **[0,0,5]**.

These are executed assertion failures, not analytical predictions, compilation
failures, hardware measurements or captured reference-emulator output. After
the production correction, all six focused tests pass. Local logs are under
`.cache/issue-83`; durable results are recorded here.

### Mandatory checks

Host: macOS arm64, native Rust 1.94.0; rustup stable Rust 1.99.0 selected for
spcasm and Wasm via the README `RUSTC` override; Node 24.12.0, Python 3.13.13.

| Command | Actual result |
| --- | --- |
| `cargo test --locked -p snes-core --test spc_timers` | Passed: six tests, zero failed/ignored |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --workspace --locked` | Passed: 430 tests, zero failed/ignored; fixed timer/cadence, BRR-end, directory, noise, pitch, envelope, mixing and echo assertions retained |
| `python3 tools/make-fixture.py .cache/smoke.bin` and `cmp tests/roms/smoke.bin .cache/smoke.bin` | Passed: byte-identical |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Passed: all three hash-pinned tools built |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Passed: CPU full/basic and SPC, 600 frames each; terminal markers 0649/0452/0557 |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Passed: 11 grayscale bands, each 16 scanlines, final shade persists through line 223 |
| `python3 tools/build-cleared-games.py` twice | Passed: two fresh builds, identical expected Castle/Memory hashes |
| `npm --prefix web ci` | Passed: zero audit vulnerabilities |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Passed |
| `npm --prefix web exec -- playwright install chromium` | Passed |
| `RUSTC="$(rustup which rustc)" SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'` | Passed: all 37 selected tests, zero failures/skips, 2.9 minutes |

`git diff --cached --check` also passed, including both newly added files.
No mandatory check failed, was skipped, or remains unperformed. The deliberate
baseline assertion failures above are the regression evidence. The browser
filter excludes only existing optional unaudited tests named `301 seconds`;
it retains the cleared games' 18,100-frame replays, Castle's full five-map and
credits route and freeze rejection, Memory gameplay/music-note durations and
finite effects, gesture-activated Web Audio, and queue/drift checks. It also
builds all eight existing CPU-built browser fixtures. No commercial ROM or
firmware was acquired.

Fixture/source/tool pins and browser assertions are unchanged. The two game
builds both reproduced Castle SHA-256
`197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`
and Memory SHA-256
`6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
Smoke SHA-256 is
`6e5518b3959883cc22e914621fc5b5d9f5c0fe4eb96ee61000769afdbb68d6e5`.
The CPU full/basic, SPC and HDMA fixture hashes match the unchanged
[Issue #79 hash record](issue-79-validation.md#mandatory-validation).

## Inventory and remaining work

All 24 inherited categories retain their full scope and outstanding work: 22
partial, `ppu-raster` and `pal` unimplemented, none verified. The table corrects
stale historical descriptions: EXTBG and selected hidden-envelope behavior are
implemented, while their complete scope and physical accuracy remain partial.

| Category | Status | Retained scope and limitations |
| --- | --- | --- |
| `cpu` | partial | Complete 65C816 semantics, interrupt edges, bus order/count and revision-specific timing; existing licensed suites do not prove all edges |
| `bus` | partial | Cartridge maps, mirrors, I/O and open bus; unsupported maps/chips and electrical floating values remain |
| `scheduler` | partial | CPU/PPU/APU/DMA synchronization and NTSC events; exact access alignment, arbitration and physical blanking latency remain |
| `dma` | partial | Transfer modes, direction, access restrictions and WRAM conflicts; full timing, electrical values and interaction matrix remain |
| `hdma` | partial | Initialization, descriptors, counters, direct/indirect reloads and phases; exact startup/overheads, arbitrary enabling and same-channel contention remain |
| `controllers` | partial | Serial and automatic joypad reads, busy/completion and input; complete device/protocol/timing coverage remains |
| `ppu-bg` | partial | Planar tiles/maps, scroll, priorities, direct color and mosaic; complete mode interactions and timed mosaic state remain |
| `ppu-offset` | partial | Modes 2/4/6 H/V descriptors, sizes and boundaries; mosaic interactions and exact fetch/raster behavior remain |
| `ppu-hires` | partial | Ordinary Mode 5/6 and math-disabled SETINI output; hires-math dispute, color-window clipping/prevention, ordinary-hires composition, mosaic, full OBJ behavior, interlace/overscan and switching/fetch timing remain |
| `ppu-mode7` | partial | Transform/direct color, implemented EXTBG and identity pseudo-hires control; complete transform/overflow/edge behavior remains |
| `ppu-objects` | partial | OAM attributes, rendering, priorities and simple screen/window routing; physical evaluation/fetch and overflow assertion timing, rotation oddities, complete size/mode interactions, interlace and hires behavior remain |
| `ppu-color` | partial | Palettes, direct color, composition, brightness and math; comprehensive windows, clipping/prevention, subtraction/saturation, halving and physical DAC behavior remain |
| `ppu-ports` | partial | VRAM/OAM/CGRAM ports, access ownership, latches and counters; exact rendering-owned corruption, latch retention, access latency and pixel-fetch arbitration remain |
| `ppu-raster` | unimplemented | Dot-level pipeline, mid-scanline writes, fetch arbitration and physical mode-switch timing |
| `ppu-display` | partial | Packed framebuffer, geometry, blanking and display shape; interlace/overscan, physical scanout and switching timing remain |
| `spc` | partial | SPC700 instructions, running live-target equality/wrap timers and communication; disabled-divider phase, physical instruction/access timing, communication races, SLEEP/STOP differences, register readback, reset retention and comprehensive instruction semantics remain unresolved or incompletely tested |
| `ipl` | partial | Firmware-free high-level upload; IPL instruction timing and general firmware execution remain |
| `dsp-voices` | partial | BRR decode, ordered dry/send mixing, interpolation and key-on behavior; independent BRR-filter 1–3 evidence, exhaustive interpolation phases and simultaneous key/mixing combinations remain |
| `dsp-noise` | partial | Shared noise generator/rates and NON routing; DSP subcycle sampling and write races remain |
| `dsp-pitch` | partial | Pitch/PMON, interpolation advancement and noise interaction; subcycle PMON sampling and torn/live writes remain |
| `dsp-envelope` | partial | ADSR/GAIN shared rates, release/retrigger, selected hidden pre-clamp envelopes and bent-GAIN boundaries; exhaustive transitions, combinations, physical retention and live subcycle interactions remain |
| `dsp-echo` | partial | Fixed-register FIR/feedback arithmetic, saturation, even writeback and numerical PCM/RAM vectors; live ESA/EDL sampling, subcycle register/RAM races, SPC bus synchronization, physical startup/reset retention remain |
| `dsp-timing` | partial | Deterministic sample cadence and browser queues; DSP subcycles, exact hardware timing and audible fidelity remain |
| `pal` | unimplemented | PAL timing, video geometry and audio synchronization |

These scopes include all prior Issue #75/#77/#79 qualifications. Physical
KON/header lookahead and publication timing, live-header sampling, directory/key
races (including KON-delay selection changes), torn directory/pitch writes,
SPC/DSP/echo RAM interactions and reset retention remain unresolved. Independent
BRR-filter 1–3 evidence, exhaustive interpolation phases and simultaneous-key
combinations remain outstanding. Ordinary zero-pitch waveform preloading and a
complete lookahead pipeline are not implemented. NON/PMON progression remains a
separate dispute. Hidden-envelope corrections and physical audio fidelity are
not established by these tests.

The only production change is the running timer counting-stage width and
post-increment comparator. The $F0 clock controls and coincident read/write
edges remain outside this validation. No disabled-divider, SPC bus/DSP subcycle scheduler,
firmware, frontend or unrelated change is included. This does not establish
full SPC accuracy, commercial compatibility or Issue #3 completion. Issue #83
requires no optional validation or post-merge verification.
