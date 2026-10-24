# Issue #73: ordered dry and echo-send saturation

Evaluation baseline: `5b9c5acbaa3686247d125107ef8040f91efbf7aa`.
Scope: sample-level voice accumulation and its existing consumers. The only
production change clamps the two independent stereo accumulators after each
voice addition. Source Issue #73 was read from GitHub, including its acceptance
criteria; it remained open at implementation time.

## Reference reconciliation

References inspected on 2026-10-24:

- [Anomie/jwdonal revision 1212, SOUND GENERATION and VOLUME CONTROL & ECHO](https://gist.github.com/nyanpasu64/a0d916ce6924912a7116682bf778e9a0#file-apudsp_jwdonal-txt):
  separate ordered dry/EON sums clamp after each addition; signed voice products
  precede master scaling. SOUND GENERATION's S4/S5 and interleaved cycle list
  establish ascending voice order for each channel. Echo feedback follows sends;
  storage drops the low bit.
- [Fullsnes, SNES APU DSP Volume Registers, VxVOLL/VxVOLR, MVOLL/MVOLR and Output Mixer](https://problemkaputt.de/fullsnes.htm#snesapudspvolumeregisters):
  explicit voices 0 through 7, per-add overflow handling, signed volumes and SAR.
  Its SAR 6 uses half-amplitude samples; our full-amplitude post-envelope output
  uses SAR 7. Anomie resolves overflow handling as saturation.
- [Fullsnes, SNES APU DSP Echo Registers, EON/EFB/EVOL/ESA](https://problemkaputt.de/fullsnes.htm#snesapudspechoregisters):
  EON selects independent sends before master volume; ESA specifies stereo
  little-endian words with bit 0 zero. EVOL and EFB have separate consumers.

The multiplication endpoints retain the [Issue #71 reconciliation](issue-71-validation.md#reconciled-contract-established-before-golden-vectors).
Signed-16 narrowing of scaled terms is the established research-emulator
convention consistent with documented intermediate widths; prose alone does not
fully specify every endpoint. No new emulator-source comparison or physical
measurement was performed. Fullsnes's final analog post-amplifier inversion stays
outside the existing digital PCM convention. No requirement here needs revision.

## Implemented arithmetic and independent derivations

For each stereo side, let `C16` clamp to [-32768,32767], `W16` wrap to signed
16 bits and `SAR` round toward negative infinity. Products use wide signed
arithmetic. The contract, established before running the vectors, is:

```text
D = 0; S = 0
for i = 0..7 in ascending order:
    A = (post_envelope_output[i] * signed8(VOL[i])) SAR 7
    D = C16(D + A)
    if EON bit i: S = C16(S + A)
B = W16((F * signed8(EFB)) SAR 7)
RAMword = C16(S + B) & -2          # unless FLG.5 disables writes
M = W16((D * signed8(MVOL)) SAR 7)
E = W16((F * signed8(EVOL)) SAR 7)
PCM = C16(M + E)                  # zero when FLG.6 mutes
```

`F` retains #71's half-amplitude RAM reads, seven wrapping FIR terms,
separately narrowed last term, final saturation and low-bit masking. The dry
and send sums cannot share a clipped result. No changes to FIR, feedback,
scaled-term narrowing, key delay, noise generation or scheduler are required.

Original held noise at rate 0 is -32768. Direct GAIN=64 gives envelope 1024,
post-envelope -16384. Voice volume -128 yields +16384, +64 yields -8192,
+127 yields -16256 and -64 yields +8192. Expectations below were calculated
from these integers, not recorded from production output or production helpers.

| Contributions in voice order | D (S with all EON) | PCM, MVOL=64 | Echo word, EFB=0 |
| --- | ---: | ---: | ---: |
| 16384,16384,16384 | 32767 | 16383 | 32766 |
| 16384,16384,16384,-8192 | 24575 | 12287 | 24574 |
| 16384,-8192,16384,16384 | 32767 | 16383 | 32766 |
| -16256,-16256,-16256 | -32768 | -16384 | -32768 |
| -16256,-16256,-16256,8192 | -24576 | -12288 | -24576 |
| -16256,8192,-16256,-16256 | -32768 | -16384 | -32768 |

The positive and negative cancellation/reordering pairs have identical wide
sums but distinct ordered results. MVOL=127 on D=32767 yields
`floor(4161409/128)=32511`, establishing the polarity regression.
For the second row EON=7 sends 32767 (RAM 32766) while EON selecting only the
cancelling voice sends -8192. EON=0 sends zero without changing dry output.
A single selected positive voice sends 16384 while dry clips to 32767.

Asymmetric stereo uses three (-128,+127) voices and a late (+64,-64) voice:
D=(24575,-24576), PCM=(12287,-12288). Sends are respectively
(24574,-24576), (32766,-32768), (-8192,8192), or (0,0) after even writeback.
Three voices alone give D=(32767,-32768); MVOL=(-1,-128) gives
(-256,-32768), testing floor rounding and +32768 signed narrowing.
MVOL=(-128,127) gives (-32767,-32512).

A second rounding vector clocks the original LFSR thirteen times from $4000
to $0002, then freezes it. Source 4 at half envelope yields 2. Voice 0 at
(127,1) contributes (1,0), voice 7 at (-128,-1) contributes (-2,-1).
Dry/send=(-1,-1), MVOL=127 preserves (-1,-1), RAM is `FE FF FE FF`.

With the four asymmetric voices, initial echo words (16384,-16384) and FIR7=64
produce F=(8192,-8192). EFB=EVOL=64 produce (4096,-4096).
All EON writes (28670,-28672); EON=7 saturates to (32766,-32768);
late-only EON writes (-4096,4096); EON=0 writes (4096,-4096).
All four give PCM=(16383,-16384). EFB=-64 and EVOL=-128 with EON=7
instead give RAM=(28670,-28672), PCM=(4095,-4096).
Mute zeros PCM with writes intact; write-disable preserves initial bytes
with audible composition intact. Existing #71 FIR/endpoint tests are unchanged.

## Original guest and partition coverage

[`dsp_mixing.rs`](../crates/snes-core/tests/dsp_mixing.rs) builds the original MIT
SPC700 guest directly from Rust instruction source. It uploads at $0200 through
the firmware-free protocol (including byte-counter wrap), configures voices
0,1,2,7 via $F2/$F3, keys them on and acknowledges readiness. Three voices
initially overflow both stereo accumulators. Command 1 records echo bytes and
EON; command 2 enables cancellation from voice 7; command 3 records cancellation
and selects EON=7; command 4 records that send and selects EON=$80; command 5
records the final send and executes STOP. Four five-byte records occupy
$1000..$1013. The host injects only original executable code, never private voice
state, accumulator state or expected output bytes.

Independent steady PCM is initially (16383,-16384), then (12287,-12288).
Echo records are `FE 7F 00 80 87`, `FE 5F 00 A0 87`,
`FE 7F 00 80 07`, `00 E0 00 20 80`. Guest upload is below 512 bytes;
each acknowledgement has a 1000-call bound of 21 master clocks, readiness is
before 200 samples, final completion before 350 samples. STOP, ENVX/OUTX and
absence of core errors are asserted. Each 20-sample settling interval numerically
checks all samples after its first ten; command transitions are not claimed as
subcycle timing evidence. See [provenance](../tests/roms/PROVENANCE.md#issue-73-original-ordered-mixing-spc700-guest-and-vectors).

The separate partition test uploads the same guest into two APUs and runs the
multi-voice overflow, cancellation and EON phases with EDL=0/1. Whole calls are
compared against repeating chunks [1,7,666,1001,21,8193]. At each phase's
1,8,9,511,512,513,1024,1025,1537-sample checkpoints it compares every PCM sample,
all 128 DSP registers, the entire $6000..$67FF echo region and guest records.
It asserts exact cadence using the unchanged 32-cycle / 21477272:1024000 clock
ratio, checks numerical steady PCM and each relevant RAM word, and checks final
STOP and no errors. Existing APU cadence assertions remain unchanged.

## Exact-baseline experiment

An isolated detached implementation-stage worktree was created, leaving the
assigned worktree and baseline implementation intact:

```sh
git worktree add --detach .cache/issue-73/baseline 5b9c5acbaa3686247d125107ef8040f91efbf7aa
cp crates/snes-core/tests/dsp_mixing.rs .cache/issue-73/baseline/crates/snes-core/tests/dsp_mixing.rs
(cd .cache/issue-73/baseline && cargo test --locked -p snes-core --test dsp_mixing issue_49152_master_127_reproduction)
(cd .cache/issue-73/baseline && cargo test --locked -p snes-core --test dsp_mixing ordered_overflow_cancellation_and_voice_slots)
cargo test --locked -p snes-core --test dsp_mixing
```

Actual baseline exits: **101**, (-16768,-16768) versus (32511,32511);
**101**, (20480,20480) versus (12287,12287) on cancellation. The earlier
initial-overflow-first table run also failed with (24576,24576) versus
(16383,16383). The final table starts with cancellation so that its baseline
failure directly demonstrates that case. Identical final tests on the corrected
implementation: **7 passed**, exit 0. No production helpers generate expectations.

## Mandatory validation

Host: macOS arm64, Homebrew Rust 1.94.0, rustup compiler selected for Wasm and
SPC assembler, Node 24.12.0, Python 3.13.13. README's compiler selection and test
port override were used; no tool/source/dependency pins changed. Logs are ignored
under `.cache/issue-73`, hardware JSON in `.cache/hardware-tests` and
`.cache/ppu-test`, browser artifacts in `web/test-results`.

| Command | Actual result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --workspace --locked` | 392 passed, 0 failures, 0 ignored |
| `cargo test --locked -p snes-core --test dsp_mixing` | 7 passed |
| `cargo test --locked -p snes-core --lib dsp::echo_tests` | 11 passed |
| `cargo test --locked -p snes-core --test dsp_echo` | 2 passed |
| `cargo test --locked -p snes-core --test dsp_noise` | 4 passed |
| `cargo test --locked -p snes-core --test dsp_pitch` | 3 passed |
| `cargo test --locked -p snes-core --test dsp_envelope` | 1 passed |
| `python3 tools/make-fixture.py .cache/smoke.bin` | Passed |
| `cmp tests/roms/smoke.bin .cache/smoke.bin` | Byte-identical, SHA-256 `6e5518b3959883cc22e914621fc5b5d9f5c0fe4eb96ee61000769afdbb68d6e5` |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | All pinned assemblers built |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | CPU full/basic and SPC passed, 600 frames each |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | 11 bands, 16 scanlines each, last shade through line 223 |
| `python3 tools/build-cleared-games.py` twice | Both passed, same pinned Castle/Memory hashes |
| `npm --prefix web ci` | Passed, 0 vulnerabilities |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Passed |
| `(cd web && npx playwright install chromium)` | Passed |
| `SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'` | 37 passed, no failures/skips, 2.8 minutes |

All mandatory checks passed. The two exact-baseline failures above are expected
regression evidence; no mandatory check remains failed, skipped or unperformed.
Both cleared 18,100-frame replays, Castle's complete route and freeze negative
control, Memory's numerical music/effect assertions, gesture-activated Web Audio
and audio queue/drift checks passed unchanged. Only the existing optional
unaudited-game tests matching `301 seconds` were excluded.

Hardware fixture terminal evidence and hashes are identical to the
[Issue #71 table](issue-71-validation.md#mandatory-commands): CPU full $0649 at
$0081A2; basic $0452 at $0081A2; SPC $0557 at $008060; HDMA bands as above.
Cleared Castle remains `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`;
Memory remains `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
Fixtures were built before browser execution. Source pins, provenance gates,
replay lengths, controller/DMA/HDMA assertions and numerical music/effect
checks are retained. Only original or provenance-approved source-built fixtures
were used; no commercial ROM or firmware was acquired.

## Inventory and remaining work

The supplied Issue #73 inventory is retained: **24 scopes, 22 partial,
ppu-raster and pal unimplemented, none verified**. Older records label dsp-voices
implemented_unverified; the supplied partial status here reflects its remaining
work, without promoting hardware accuracy. All inherited scopes remain below.

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
| `ppu-mode7` | partial | Transform/direct color and identity pseudo-hires control; EXTBG and complete transform/overflow/edge behavior remain |
| `ppu-objects` | partial | OAM attributes, rendering, priorities and simple screen/window routing; physical evaluation/fetch and overflow assertion timing, rotation oddities, complete size/mode interactions, interlace and hires behavior remain |
| `ppu-color` | partial | Palettes, direct color, composition, brightness and math; comprehensive windows, clipping/prevention, subtraction/saturation, halving and physical DAC behavior remain |
| `ppu-ports` | partial | VRAM/OAM/CGRAM ports, access ownership, latches and counters; exact rendering-owned corruption, latch retention, access latency and pixel-fetch arbitration remain |
| `ppu-raster` | unimplemented | Dot-level pipeline, mid-scanline writes, fetch arbitration and physical mode-switch timing |
| `ppu-display` | partial | Packed framebuffer, geometry, blanking and display shape; interlace/overscan, physical scanout and switching timing remain |
| `spc` | partial | SPC700 instructions, timers and communication; full timing and register/RAM races remain |
| `ipl` | partial | Firmware-free high-level upload; IPL instruction timing and general firmware execution remain |
| `dsp-voices` | partial | BRR decode, ordered dry/send mixing, interpolation and key-on behavior; independent BRR-filter 1–3 evidence, exhaustive interpolation phases and simultaneous key/mixing combinations remain |
| `dsp-noise` | partial | Shared noise generator/rates and NON routing; DSP subcycle sampling and write races remain |
| `dsp-pitch` | partial | Pitch/PMON, interpolation advancement and noise interaction; subcycle PMON sampling and torn/live writes remain |
| `dsp-envelope` | partial | ADSR/GAIN shared rates, release/retrigger and boundaries; hidden/bent GAIN, exhaustive transitions and live interactions remain |
| `dsp-echo` | partial | Fixed-register FIR/feedback arithmetic, saturation, even writeback and numerical PCM/RAM vectors; live ESA/EDL sampling, subcycle register/RAM races, SPC bus synchronization, physical startup/reset retention remain |
| `dsp-timing` | partial | Deterministic sample cadence and browser queues; DSP subcycles, exact hardware timing and audible fidelity remain |
| `pal` | unimplemented | PAL timing, video geometry and audio synchronization |

Live echo-register sampling, hidden envelopes, subcycle register/RAM interactions,
SPC bus synchronization and physical startup/reset behavior remain separate work.
No DSP subcycle scheduler, IPL implementation, frontend or cartridge change is
included. Passing these checks establishes neither full hardware accuracy nor
commercial compatibility and does not complete Issue #3. Physical-device and
unaudited-game tests were not performed; no post-merge verification is required.
