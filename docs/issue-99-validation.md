# Issue #99: same-line horizontal timer IRQ delay

## Source reconciliation recorded before implementation

Read on 2026-10-24: [Fullsnes](https://problemkaputt.de/fullsnes.htm),
**SNES Timing H/V Events**, detailed list, and **SNES PPU Interrupts**
(NMITIMEN, HTIME/VTIME, TIMEUP); [Anomie's Register Doc, revision 1157](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt),
NMITIMEN, HTIME/VTIME and TIMEUP. Fullsnes supplies the numerical delay;
Anomie describes assertion following comparison. The terse register equality
summaries do not override the detailed event timing.

For nonzero horizontal comparisons Fullsnes specifies HTIME+3.5 timer units,
with V equality required only in combined mode. CPU timer units have four
master clocks, including near the PPU's long dots: assertion is
`4 * (HTIME + 3.5) = 4 * HTIME + 14`, not a PPU counter conversion.
The independent offsets for HTIME 1/64/256/321/323/327/336/337 are
18/270/1038/1298/1306/1322/1358/1362. The strict same-line domain is
`0 < HTIME && 4 * HTIME + 14 < line_length`: 1..337 on 1364-clock
ordinary lines, 1..336 on the 1360-clock field-1 V=240 line.

Fullsnes explicitly gives combined HTIME=0 the V-only offset of 2.5 units,
whereas its H-only expression would give 3.5. This contradicts treating the
existing zero comparator as verified hardware. Zero is excluded here: the
retained H=0 rollover behavior is only an approximation, not an older Issue
requirement elevated to hardware fact. The detailed list mentions combined
HTIME through 339 and CPU last-dot differences, but does not supply a complete
line-spanning delivery/VTIME ownership rule. No rollover rule is inferred.
Late comparisons retain their old immediate-comparison approximation, including
HTIME=337 on short lines. Unreachable old comparisons remain unreachable.

The implementation changes only eligible same-line crossings. V-only remains
at master H=10; pending retention, read acknowledgement, natural geometry,
field reporting and device advancement retain their existing behavior. Fullsnes
qualifies acknowledgement during its condition window; Anomie's old write-clear
wording also differs. Neither coincident physical reads nor TIMEUP writes are
resolved. Timed reads sample at access end and DMA uses existing model phases;
CPU service remains at instruction boundaries after DMA. Live configuration,
physical reset, NMI delay, PAL/interlace and raster rendering remain outside scope.

## Independent tests and preserved regressions

`cargo test --locked -p snes-core horizontal_irq` runs seven original unit
regressions and one original source-built guest. Unit expectations never call
production line-length, timer-event or PPU conversion helpers.

- The fresh before/at/after matrix covers all eight literal offsets above,
  modes $10/$30, V=0/1/224/225/239/240/241/261, and field 1 then field 2
  reached by natural rollover. H-only uses deliberately unreachable VTIME=511.
  Earlier events are discarded at target H=0; each observation uses a fresh
  bus, so acknowledgement cannot hide an early event. Reads acknowledge once,
  early reads preserve future assertion, and the rest of the line never
  duplicates it. Short-line HTIME=337 is excluded from this verified matrix.
- Combined-mode tests reject every intervening nonmatching line and unreachable
  VTIME=262/511. Pending flags survive lines, fields and later matching events
  in both modes. Separate zero/late compatibility controls explicitly constrain
  only the retained approximation, including short-line HTIME=337 and old
  unreachable HTIME=340/341 cases.
- One-clock, irregular 1/7/1361/2/43/8193/5 and whole intervals exercise
  ordinary/short lines and field boundaries in both modes. Literal events
  include 1634 (separately in timed reads), 357638/715002 for V=0,
  1382/358750 for V=1, 328718/686086 for V=240 HTIME=336,
  330086/687450 for V=241 HTIME=337 and 357366/714730 for V=261 HTIME=337.
  Final geometry is clock 715012, field 2/V=0/H=280. Independent sample count
  is `2*floor(715012*32000/21477272)`; PCM agrees across all partitions.
- Six-clock CPU TIMEUP reads end at event-1/event/event+1, including events
  after natural V=0, the short line and V=241 rollover. Access-end sampling
  and immediate acknowledgement are existing model conventions.
- Real fixed-A general DMA reads $004211 into WMDATA. Existing phases are
  8 global + 8 channel setup, read +4/write +8 per byte. Starts at event-21,
  -20 and -19 sample respectively -1/+7/+15, 0/+8/+16 and +1/+9/+17:
  expected driven bits are 0/$80/0, $80/0/0 and $80/0/0. Tests verify all
  destinations, counts, addresses, acknowledgement and final geometry in both
  modes around ordinary, natural field and short-line rollover events.
- A real 100-byte DMA starting at 1613 crosses the corrected event 1634,
  advances audio/PPU/raster and holds an unmasked waiting CPU until complete.
  Existing model costs give `16+100*8+40=856`, ending at 2469/V=1/H=1105.
  CPU PC, instruction count and stack remain unchanged during DMA; all 100
  VRAM bytes and channel counts are checked. The next Machine step takes one
  three-byte emulation IRQ frame and executes handler NOP. DMA phases and
  refresh placement are conventions, not verified startup/alignment timings.
- The guest exercises HV at V=240 and H-only at V=242 (VTIME stays 240) in
  four natural fields, checking every literal record in executed 65C816 code.
  Explicit guest failure/success markers and both host deadlines prevent silent
  hangs. [PROVENANCE](../tests/roms/PROVENANCE.md#original-horizontal-irq-delay-diagnostic-issue-99)
  records authorship, MIT construction and full independent clock derivation.

The existing NTSC guest still visits four fields and retains all geometry and
parity assertions. With `321*4+14=1298`, WAI wakes at
328660/686026/1043392/1400758 and SLHV completes at within-line
1330/1328/1330/1328: ordinary PPU H=331, short H=332. Counter-latch HTIME=256
now asserts at 1038; six-clock WAI wakes at 1040, giving A=(267,256),
B=(318,256), C=(81,258). Gating, selectors and acknowledgements are unchanged.

The OAM guest is deliberately retimed from HTIME=330 to 327:
`327*4+14=1322`. Even WAI wake 1322..1326 + address setup 46 - line 1364
places the address write at V=225/H=4..8; the next 30-clock read finishes at
H=34..38, strictly before H=40. Subsequent reads complete H=96..100 and later.
The original $12,$12,$34,$56 assertions still distinguish pre/post reload.
Merely accepting a post-reload first observation was not used.

The workspace run additionally exposed the two #35 HDMA phase guests' dependence
on immediate IRQ timing. Retiming HTIME=270 to 267 gives assertion 1082,
WAI wake 1082..1086, first read start 1106..1110 and finish 1112..1116.
Service cannot preempt a read that begins before eligibility at 1112. Original
pre-service table and later row-end/pointer/status bytes remain unchanged.
Relevant earlier validation/provenance statements are updated or explicitly
superseded; historical baseline/gate results remain historical evidence.

## Exact-baseline behavioral counterexamples

Exported the exact evaluation baseline and transferred only test sources and
unit module registration; no runtime implementation was transferred:

```sh
mkdir -p .cache/issue99/baseline
git archive 20116952ce5617cc487fed33fa7c5574ed86adf0 | tar -x -C .cache/issue99/baseline
cp crates/snes-core/src/machine/horizontal_irq_tests.rs .cache/issue99/baseline/crates/snes-core/src/machine/
cp crates/snes-core/tests/horizontal_irq.rs .cache/issue99/baseline/crates/snes-core/tests/
cat >> .cache/issue99/baseline/crates/snes-core/src/machine.rs <<'RS'

#[cfg(test)]
#[path = "machine/horizontal_irq_tests.rs"]
mod horizontal_irq_tests;
RS
cargo test --manifest-path .cache/issue99/baseline/Cargo.toml --locked -p snes-core --lib horizontal_irq_boundary_matrix_fresh_before_at_after
cargo test --manifest-path .cache/issue99/baseline/Cargo.toml --locked -p snes-core --test horizontal_irq
```

Both compile and exit 101 on behavioral assertions. The boundary test sees
TIMEUP set at field 1/V=0/H=17, HTIME=1/H-only, where it must still be clear.
The diagnostic collects four fields then fails its guest check: baseline HV
PPU H=328/329/328/329 versus required 331/332/331/332, and H-only H=71/72/71/72
versus 76/75/76/75. All other record fields agree. Corrected focused tests pass.

## Executed acceptance gates (2026-10-24)

Detailed transient logs are in ignored `.cache/issue99`; durable results follow.
The initial tool build used Homebrew Rust 1.94 and failed on the pinned SPC
assembler's newer language requirement. Re-running with the README's installed
rustup compiler selection (Rust 1.99) passed without changing any pin/source.
The first workspace run exposed the two HDMA phase synchronization failures
above. After retiming their original guest setup, the entire suite passed;
no hardware assertion, replay route, tolerance or licensing decision was relaxed.

| Executed command | Actual result |
| --- | --- |
| `cargo test --locked -p snes-core horizontal_irq` | Pass: seven new unit tests and one self-checking CPU guest |
| Exact-baseline focused commands above | Both compile, fail behaviorally (exit 101); diagnostic reports success=$00/failure=$EE |
| `cargo test --locked -p snes-core --test ntsc_timing --test ppu_counter_latch --test ppu_oam_reload` | Pass: all three preserved guests |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Pass |
| `cargo test --workspace --locked` | Pass: 488 tests, including 216 core unit tests; zero failures/ignored tests |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Pass: all three pinned assemblers |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Pass: all three audited source-built suites |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Pass: original source-built PPU bands/rows gate |
| `python3 tools/make-fixture.py .cache/smoke.bin` and `cmp tests/roms/smoke.bin .cache/smoke.bin` | Pass: identical original smoke fixture |
| `python3 tools/build-cleared-games.py` twice | Both pass; identical hashes below |
| `npm --prefix web ci` | Pass |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Pass: rebuilt actual Wasm and browser artifacts |
| `npm --prefix web exec playwright install chromium` | Pass |
| `SNES_TEST_PORT=18099 npm --prefix web test -- --grep-invert '301 seconds'` | Pass: 37 browser checks in 2.9 minutes, including gameplay/progression/music, real Wasm/canvas and audio queue assertions |
| `git diff --check` | Pass |

Both cleared-game builds retain Castle Platformer SHA-256
`197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`
and Memory Game
`6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
Smoke SHA-256 is
`6e5518b3959883cc22e914621fc5b5d9f5c0fe4eb96ee61000769afdbb68d6e5`.
No mandatory check remains unavailable. Physical-device corroboration and
optional 301-second real-time browser soaks were not performed.

## Remaining inventory

All 24 categories retain their existing scope. `cpu`, `bus`, `scheduler`, `dma`,
`hdma`, `controllers`, `ppu-bg`, `ppu-offset`, `ppu-hires`, `ppu-mode7`,
`ppu-objects`, `ppu-color`, `ppu-ports`, `ppu-display`, `spc`, `ipl`,
`dsp-voices`, `dsp-noise`, `dsp-pitch`, `dsp-envelope`, `dsp-echo`, `dsp-timing`
remain partial; `ppu-raster` and `pal` remain unimplemented. None is fully
verified. Scheduler zero/late IRQs, live configuration, NMI delays,
refresh/startup revisions, physical CPU sampling and broader synchronization
remain outstanding. Bounded passing tests establish neither complete hardware
accuracy nor commercial compatibility and do not complete Issue #3.
