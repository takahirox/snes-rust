# Issue #45: automatic OAM reload at H=10 dots

For [Issue #45](https://github.com/takahirox/snes-rust/issues/45), evaluation
baseline `11e4c0ba10893f10f1a0caec9f461620a37f3adc`. Original code and data
are MIT licensed. No commercial ROM or firmware is acquired.

## Specification reconciliation (recorded before production changes)

Retrieved 2026-10-24 directly from both cited URLs. Complete raw copies are
ignored in `.cache/references/`; hashes match Issue #43's record exactly.

| Reference | Version / SHA-256 | Exact sections and raw lines |
| --- | --- | --- |
| [Fullsnes](https://problemkaputt.de/fullsnes.htm) | no$sns v1.5 / `3853b65a7331bf12eea431ff494a2948a1c78ef22f7e819db4b139fd5a79f6c6` | SNES Memory OAM Access (Sprite Attributes), 503–543; INIDISP, 973–989; SNES Timing H/V Events, 26986–27022 |
| [Anomie's Register Doc](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt) | revision 1157, 2007-07-12 / `19dfdf3f1c206655b8ede11bcd6dc053e36634f876819e40bb39cfb88a237dfd` | INIDISP, 48–63; OAMADDL/H, 89–115; SPRITES/OAM, 1368–1401 |

Fullsnes's detailed H/V list separately places the VBlank flag at H=0 and
OAMADD reload at H=10 on V=225. Its vertical summary groups these activities on
the first VBlank line. Its OAM register section and Anomie describe reload at
VBlank beginning without a horizontal coordinate. We interpret those as general
line/period descriptions and use the detailed coordinate to refine the timing.
This explicitly supersedes Issue #43's H=0 automatic-reload convention and tests.

Bounded contract: fixed NTSC non-overscan lines of 1364 master clocks, VBlank
availability at V=225/H=0, automatic reload at H=40 master clocks (10 dots).
The event samples the current forced-blank bit: clear reloads both internal
cursors to twice the programmed nine-bit word, even phase; set suppresses it.
Each crossing fires once, even in a multi-line advance, and recurs each frame.
Neither reference establishes clearing the shared data latch, so retention is
explicit model policy, not independently verified hardware behavior.

Address-register writes still reload immediately. The separate first-VBlank-line
INIDISP old-forced-blank write rule remains as documented for Issue #43, including
its source disagreement. Immediate INIDISP effects, CPU completion-time accesses,
DMA source sampling at +4 and destination commitment at +8, and event-before-access
ordering on coincident clocks are existing model assumptions. Fullsnes explicitly
leaves physical blanking latency uncertain; these tests cannot establish electrical
latency or physical coincident bus strobes. Rendering-owned corruption addresses,
object-fetch timing, CGRAM arbitration, PAL, overscan and interlace remain separate.

## Implementation and independent numerical vectors

`SystemBus::tick` detects the crossing of master clock 40 while V=225 inside
its existing scanline loop. The strict old-H comparison prevents a second reload
on subsequent advances; line/frame transitions allow recurrence. The PPU's line
start method only handles rendering ownership now. A separate automatic reload
entry point checks forced blank and calls the existing address reload, preserving
the latch. No scheduler rewrite or persistent event flag is needed.

Four new original machine tests use literal addresses/bytes, never production
reload helpers as expected-value oracles:

- `automatic_reload_waits_for_h40_and_is_partition_invariant`: programmed word
  `$104`, retained latch `$65`, odd cursors. With display enabled, the policy
  rendering cursor/rotation pair is `(513,521)` at H=0 and H=38, becomes
  `(520,520)` at H=40, and stays there at H=42. With forced blank held, it stays
  `(521,521)` throughout. Registers remain `$04,$01`, including the ninth address
  bit. A read advances once; several subsequent line starts do not reload. The
  next frame's H=40 repeats the event (or suppresses it with forced blank held).
  Single two-clock and split one-clock advances through H=40 agree.
- `automatic_reload_crossing_lines_in_one_advance_matches_partitions`: a single
  advance from V=224/H=100 to V=226/H=200 agrees with partitioned advances across
  those lines: both cursors are byte 8, latch `$C3`, programmed word 4, and an
  ordinary read returns `$12` and advances to 9.
- `cpu_oam_access_completion_before_at_after_h40`: ordinary timed CPU reads and
  writes complete at H=38/40/42. The pre-event policy odd byte `$B2` and cursor
  `(514,10)` give way to source-backed programmed even byte `$12` and `(9,9)`.
  A write before the event commits high-odd `$65` without capturing the latch;
  one at/after the event captures low-even `$65` without committing a pair.
- `dma_oam_sampling_and_pair_commit_before_at_after_h40`: two-byte forward and
  reverse DMA transfers use normal arming, overhead and shared transfer paths.
  Reverse source at H=38 samples policy `$B2`; the event at 40 resets both
  cursors before A-bus commitment at 42, without changing the sampled value.
  The next read returns source-backed `$12`. Source at 40/42 instead yields
  `$12,$34`. Forward destination at 38 writes high-odd `$65`, then the second
  byte captures low-even `$78` after reload. Destination at 40/42 captures `$65`
  and then commits `$65,$78` to the low pair. In particular, destination at 42
  uses the restored address despite its source having been sampled at 38.
  A1T advances `$0400→$0402`, DAS becomes zero, bank `$7E` is retained, pending
  DMA clears, and later ordinary reads advance without another reload.

All coincident H=40 assertions use event-before-access model ordering, not a
claim about measured physical strobes. Restricted pre-event byte assertions are
explicit rendering policy; the guest below avoids relying on those bytes.

The old H=0 DMA vector now expects the preserved high-table cursor rather than
an implicit low-table restore. Access availability still changes at H=0; software
can explicitly program the address there. Its frame-wrap controls retain the
rendering transition. The existing PPU blanking vector now asserts no H=0 reload
before delivering the separate reload event; rotation tests use that event too.
All other latch, mirror, rotation, CPU, DMA/HDMA and game assertions are retained.

## Original CPU guest and derived windows

`crates/snes-core/tests/ppu_oam_reload.rs` constructs an original in-memory
32 KiB MIT LoROM. It initializes low-table bytes `$12,$34,$56,$78`, enables
display, programs H/V IRQ at V=224/H=330 dots, and executes SEI/WAI. Under the
retained scheduler convention, IRQ comparison is at `HTIME*4`, and WAI advances
in six-clock idles: with even clocks it wakes at H=1320..1324. This is a model
synchronization derivation, not physical IRQ latency verification (Fullsnes lists
an additional IRQ offset). All following bus accesses precede the next refresh.

| Executed operation | Cost / completion on V=225, master clocks |
| --- | --- |
| LDA immediate 4; STA `$2102` | 16 + 30; H=2..6, explicitly restores byte 8 during VBlank |
| LDA `$2138` | 30; H=32..36, reads `$12` before automatic reload |
| STA WRAM `$1800` | 32; H=64..68, crossing H=40 |
| LDA `$2138` | 30; H=94..98, reads restored `$12` after automatic reload |
| Subsequent STA/LDA pairs | 62 each; read `$34`, then `$56`, proving no duplicate reload |

The guest's H=2..6 address write makes even the early read independent of any
rendering-corruption address or byte. Later values follow the source-backed
programmed-address reload and byte increment rules. It publishes four WRAM bytes
and marker `$18F0=$A5`, then STP. The host supplies only the cartridge and
observes results/deadlines: 100,000 steps and fewer than two frame wraps. No
CPU, bus, raster, PPU, memory or result state is patched. Provenance is recorded
in `tests/roms/PROVENANCE.md`; no generated ROM is committed.

## Baseline counterexamples

Before changing production files, added the new tests to this node's upstream
checkout. Its production files match evaluation baseline
`11e4c0ba10893f10f1a0caec9f461620a37f3adc`.

1. `cargo test --locked -p snes-core automatic_reload_waits_for_h40_and_is_partition_invariant`
   failed (exit 101): at H=0 actual `(520,520,$65)`, expected preserved
   `(513,521,$65)`. This detects the early reload directly, independently of
   any expected-value helper.
2. `cargo test --locked -p snes-core --test ppu_oam_reload` failed (exit 101):
   actual guest results `$12,$34,$56,$78`, expected `$12,$12,$34,$56`. The old
   H=0 event happens before the guest's address write and cannot reload after
   its early read, so this distinguishes the placements using executed code.

Both pass after the correction. The first focused run also exposed the obsolete
H=0 DMA expectation `(8,8)` versus the now-preserved `(513,9)`; its derivation is
corrected above instead of changing the source-backed H=40 contract.

## Mandatory validation

Run on 2026-10-24. Cargo/Clippy use installed Homebrew Rust 1.94; the SPC
assembler and Wasm build use the README's `RUSTC="$(rustup which rustc)"`
selection (rustup stable 1.99). The initial unqualified
`python3 tools/build-test-tools.py --spc` failed because Rust 1.94 lacks the
assembler's `if let` guards; selecting the documented compiler passed without
changing pinned source or flags. Ignored logs/artifacts are in `.cache/` and
`web/test-results/`; results below are recorded for the checkpoint.

| Actual command | Outcome |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --workspace --locked` | Passed, 234 tests, including the existing OAM guest and all CPU/DMA/HDMA/VRAM/counter/rendering regressions |
| `cargo test --locked -p snes-core --lib oam` | Passed, 17 focused tests |
| `cargo test --locked -p snes-core --test ppu_oam_reload` | Passed, original executed guest |
| `python3 tools/make-fixture.py .cache/smoke.bin` | Passed, fixture SHA-256 `73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb` |
| `cmp tests/roms/smoke.bin .cache/smoke.bin` | Passed, byte-identical |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Passed, all three pinned assemblers built |
| `RUSTC="$(rustup which rustc)" python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Passed, all 3 suites: full CPU terminal `$00:81A2`, last `$0649`; basic CPU `$00:81A2`, last `$0452`; SPC `$00:8060`, last `$0557` |
| `RUSTC="$(rustup which rustc)" python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Passed, 11 grayscale bands and final shade through scanline 223 |
| `python3 tools/build-cleared-games.py` (first run) | Passed, pinned Castle/Memory hashes |
| `python3 tools/build-cleared-games.py` (second fresh build) | Passed, same pinned hashes |
| `npm --prefix web ci` | Passed |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Passed, production Wasm/static assets |
| `(cd web && npx playwright install chromium)` | Passed |
| `SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'` | Passed, all 18 browser checks in 2.6 minutes; documented alternate port |
| `git diff --check` | Passed |

Both fresh builds retained Castle SHA-256
`197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`
and Memory SHA-256
`6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
Fixture source pins/hashes, notices, licenses, replay inputs, durations and
assertions are unchanged. The grep excludes only optional external games; it
preserves both cleared-game five-minute replays and all other mandatory checks.

Browser evidence confirms 18,100 frames (301.172 emulated seconds) and all 20
windows per game. Castle's separate 11,097-frame route traversed all five maps,
recorded 20 checkpoints and finished credits. Memory won/restarted 18 times;
its 600-frame audio check recovered 40 notes and the finite card effect while
music continued. Gesture-activated Web Audio recorded 423 music buffers and nine
effect buffers, with pause/reset silence. Queue/drift/stall/repeat playback
checks reported zero invalid schedules/samples. UI, save recovery and Pages
checks also passed. No browser assertion, game recognition or fixture was changed.

## Remaining uncertainty and inventory

This establishes the bounded automatic reload coordinate and its existing bus
integration, not full hardware accuracy or commercial compatibility. Forced-blank
electrical latency, physical coincident strobes, OAM corruption/fetch behavior,
CGRAM arbitration, VRAM invalid-fetch values, external counter inputs and
physical reset retention remain unresolved. Immediate INIDISP, IRQ alignment,
rendering-address substitutes and latch retention retain their qualifications.
PAL, overscan/interlace and dot-level object/pixel fetches are outside scope.

All 24 inventory categories retain their outstanding scopes from the
[baseline inventory](issue-27-validation.md#remaining-uncertainty-and-inventory):
`cpu`, `bus`, `scheduler`, `dma`, `hdma`, `controllers`, `ppu-bg`, `ppu-offset`,
`ppu-mode7`, `ppu-objects`, `ppu-color`, `ppu-ports`, `ppu-display`, `spc`, `ipl`,
`dsp-noise`, `dsp-pitch`, `dsp-envelope`, `dsp-echo`, `dsp-timing` remain partial;
`dsp-voices` remains `implemented_unverified`; `ppu-hires`, `ppu-raster`, `pal`
remain unimplemented. No category or historical Issue #3 requirement is completed.
No optional forced-blank transition guest was added; no post-merge verification
is required.
