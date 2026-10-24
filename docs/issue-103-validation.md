# Issue #103: H-only HTIME=0 IRQ at master H=14

## Source reconciliation (2026-10-24)

Retrieved [Fullsnes](https://problemkaputt.de/fullsnes.htm), SNES Timing H/V
Events detailed list and SNES PPU Interrupts, and
[Anomie's Register Doc revision 1157, dated 2007-07-12](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt),
NMITIMEN/HTIME/VTIME/TIMEUP. Fullsnes gives H-only assertion at HTIME+3.5
in timer units; four master clocks per unit give `4*(0+3.5)=14` at HTIME=0.
Its separate 2.5-unit event explicitly covers V-only and combined HTIME=0,
yielding H=10; it does not include H-only. Nonzero eligible horizontal
comparisons retain `4*HTIME+14`. This follows the distinction already recorded
in the [#99 reconciliation](issue-99-validation.md#source-reconciliation-recorded-before-implementation)
and [#101 reconciliation](issue-101-validation.md#source-reconciliation-before-implementation-2026-10-24).

Fullsnes's terse register descriptions describe equality without the detailed
assertion delay. Anomie describes mode gating and assertion following the
comparison, but provides no independent numerical measurement of 14 clocks.
Those summaries do not establish immediate H=0 assertion. The detailed event
list supersedes the old H-only-zero approximation, rather than promoting a
passing approximation test to a hardware contract. No retrieved numerical
H-only-zero timing contradicts the 3.5-unit expression. Acknowledgement is a
separate disputed behavior: Fullsnes qualifies reads during a 4–8-clock condition
window; Anomie's older text also mentions write clearing. Those differences
remain unresolved. Access-end sampling, immediate read acknowledgement, DMA
phases and instruction-boundary service remain emulator conventions.

The bounded domain is preconfigured NTSC non-interlaced H-only mode, both
HTIME bytes zero before a naturally entered line. Initial reset-created V=0
retains its no-event policy; this is explicitly unverified physical reset
alignment. This supersedes only obsolete H-only-zero approximation/status
statements in #97/#99/#101. Combined zero and V-only remain at ten clocks.

## Implementation and independent coverage

`SystemBus::tick` includes H-only zero in the existing horizontal crossing at
14 and removes its rollover assertion. It excludes initial reset V=0 only for
zero, preserving nonzero behavior. Large ticks continue splitting at natural
line boundaries. No new pending-event state or timing subsystem is introduced.

`cargo test --locked -p snes-core horizontal_zero_irq` runs seven unit tests
and one original guest:

- Fresh buses cover H=0/9/10/13/14/15 on naturally reached
  V=0/1/224/225/239/240/241/261 in fields 1 and 2, each with VTIME=0/240/262/511.
  Earlier flags are cleared at target start minus one, never H=0. Early reads
  preserve the H=14 delivery, acknowledgement occurs once, no later same-line
  or next-H=0 duplicate occurs, and the next line delivers again at 14.
- Pending retention crosses lines, fields and later assertions without reads;
  subsequent acknowledgement clears once. Initial reset-created V=0 has a
  separate no-event control. The corrected older once-per-line test visits all
  786 lines of three complete natural fields (fields 1–3), including both
  short lines, and ends at literal clock 1429464/field 4/V=0/H=0.
- One-clock, irregular 1/7/1361/2/43/8193/5 and whole intervals cover literal
  events 1378 (initial V=1), 357382/714746 (natural V=0), 327374/684742 (V=240)
  and 328738/686102 (V=241). Earlier pending flags are cleared before each
  target line. Independent geometry uses ordinary 1364-clock lines, odd-field
  V=240 length 1360 and field starts 0/357368/714732. Each run ends at
  event+1, with raster H=15, the expected V/field/parity, and stereo samples
  `2*floor((event+1)*32000/21477272)`. PCM equality is determinism evidence only.
  No expected value uses a production scheduler or PPU conversion helper.
- Six-clock CPU TIMEUP reads end at H=0/9/10/13/14/15 across ordinary,
  short-line, following-line and natural field boundaries. Sampling at access
  end and immediate acknowledgement at the coincident event are conventions.
- Real fixed-A $004211 -> WMDATA DMA starts at event-21/-20/-19. Existing
  phases are 8 global + 8 channel setup and read +4/write +8 per byte:
  samples at event-1/+7/+15, event/+8/+16 or event+1/+9/+17. Destination
  bit-7 sequences are [0,$80,0], [$80,0,0], [$80,0,0]. All three complete
  destination bytes overwrite sentinels (lower bits retain initial zero open
  bus), WMDATA pointer is 3, fixed source and exhausted channel count are
  checked, pending/enable masks clear, and clocks/raster and acknowledgement
  are checked. These are model phases, not physical DMA alignment evidence.
- A real 100-byte WRAM -> VRAM DMA crosses event 1378 while an unmasked CPU
  waits, starting with no earlier IRQ. Existing 16 setup + 800 payload + one
  40-clock refresh hold ends at 2209/V=1/H=845. All payload bytes, pointers,
  channel completion, PPU advancement and independently calculated audio
  count are checked. PC, instruction count, stack pointer and stack memory
  stay unchanged during DMA; IRQ stays pending. Resumption performs one
  three-byte emulation IRQ entry and handler NOP, with TIMEUP retained until
  acknowledged. This verifies integration under existing CPU conventions.
- The MIT source-built 65C816 guest establishes H-only zero before V=240 in
  four natural fields, keeping deliberately nonmatching VTIME=239. It checks
  all 32 literal counter/parity/acknowledgement bytes in executed guest code,
  with explicit failure/success markers and bounded step/clock deadlines.
  Host code only loads, steps and observes. The
  [provenance](../tests/roms/PROVENANCE.md#original-horizontal-zero-irq-diagnostic-issue-103)
  records construction, authorship, independent geometry and CPU-cost arithmetic.

The older horizontal zero/late control now checks zero at 14 and retains all
late-comparison controls. The NTSC four-field IRQ/NMI test also contained an
obsolete H=0 expectation: its first run failed at line 271 with 0 versus 128.
It now checks H=13/14 and no rollover duplicate, retaining four fields of NMI
checks and explicit initial-V=0 policy. All other timing assertions are retained.

## Exact-baseline behavioral counterexamples

Exported exact baseline `7f2e79aa55fbcaa5393bc81bb9deaff00252864f` and transferred
only the two new test sources and their unit module registration:

```sh
mkdir -p .cache/issue103/baseline
git archive 7f2e79aa55fbcaa5393bc81bb9deaff00252864f | tar -x -C .cache/issue103/baseline
cp crates/snes-core/src/machine/horizontal_zero_irq_tests.rs .cache/issue103/baseline/crates/snes-core/src/machine/
cp crates/snes-core/tests/horizontal_zero_irq.rs .cache/issue103/baseline/crates/snes-core/tests/
cat >> .cache/issue103/baseline/crates/snes-core/src/machine.rs <<'RS'

#[cfg(test)]
#[path = "machine/horizontal_zero_irq_tests.rs"]
mod horizontal_zero_irq_tests;
RS
cargo test --manifest-path .cache/issue103/baseline/Cargo.toml --locked -p snes-core --lib horizontal_zero_irq_fresh_boundary_matrix_and_no_duplicate
cargo test --manifest-path .cache/issue103/baseline/Cargo.toml --locked -p snes-core --test horizontal_zero_irq
```

Both compile successfully and exit 101 on behavioral assertions. The boundary
matrix sees TIMEUP=true at field 1/V=0/VTIME=0/H=0, where false is required.
The guest collects all four fields and rejects baseline H=7/8/7/8 instead of
12/11/12/11, with all other record bytes equal. It executes failure=$EE and
success=$00. The corrected implementation passes both, including guest
failure=$00/success=$A5. During test authoring, a stale third-field literal 10
from the adapted #101 test caused an initial corrected guest failure; it was
corrected to the independently derived 12 before the baseline comparison.

## Executed acceptance gates (2026-10-24)

| Command | Actual result |
| --- | --- |
| `cargo test --locked -p snes-core horizontal_zero_irq` | Pass: 7 unit tests + 1 guest |
| `cargo test --locked -p snes-core v_only_irq` | Pass: 7 unit tests + 1 guest |
| `cargo test --locked -p snes-core horizontal_irq` | Pass: 7 unit tests + 1 guest; all late controls retained |
| `cargo test --locked -p snes-core combined_zero_irq` | Pass: 7 unit tests + 1 guest |
| `cargo test --locked -p snes-core --test ntsc_timing` | Pass: four-field geometry guest |
| `cargo test --locked -p snes-core --lib machine::ntsc_timing_tests` | Pass: all 8 tests after correcting the obsolete zero offset described above |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Pass |
| `cargo test --workspace --locked` | Pass: 502 tests, 0 failed, 0 ignored |
| `python3 tools/make-fixture.py .cache/smoke.bin` and `cmp tests/roms/smoke.bin .cache/smoke.bin` | Pass: byte-identical original fixture |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Pass: all three pinned assemblers |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Pass: full/basic CPU and SPC suites |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Pass: all 11 grayscale HDMA bands |
| `python3 tools/build-cleared-games.py` (twice) | Pass: both pinned hashes match in both builds |
| `npm --prefix web ci` | Pass |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Pass: fresh Wasm and browser artifacts |
| `npm --prefix web exec playwright install chromium` | Pass |
| `SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'` | Pass: all 37 checks, 2.9 minutes |

The documented rustup compiler selection and port override were used from the
start; no compiler/port failure occurred in these gates. All focused gates and
the final workspace, formatting and Clippy runs pass. The initial NTSC unit
failure and the guest-authoring literal correction are recorded above; neither
was a compile failure or an unreported waived assertion.

Hardware suites retain licensed source pins and existing native assertions.
Full/basic CPU end at $0081A2 with last tests $0649/$0452; SPC ends at $008060
with $0557. PPU HDMA SHA-256 remains
`aef339e5d4a7319db31de577b253d977b45f5c45f09adfaa99a302ca300a957a`.
Cleared-game SHA-256 hashes remain Castle
`197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`
and Memory
`6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
No fixture source, pin, hash, replay input, gameplay/music assertion, deadline
or tolerance was changed. Only original diagnostics and existing licensed
source-built fixtures were used; no commercial ROM or firmware was acquired.

Browser Castle and Memory replays each execute 18100 fields (301.170679
emulated seconds). Castle's controller route traverses all five maps and
credits in 11097 fields; Memory completes 18 wins. Existing music, independent
finite effects and gesture-activated Web Audio checks pass. All CPU/WAI,
V-only, combined-zero, nonzero/late IRQ, geometry, controller, DMA/HDMA,
PPU-port and audio regressions pass in the final workspace run.

Local raw logs are under `.cache/issue103/`, hardware evidence under
`.cache/hardware-tests/results.json` and `.cache/ppu-test/results.json`, game
hashes under `.cache/cleared-games/results.json`, and browser evidence under
`web/test-results/`. These ignored build artifacts are not checkpoint files;
this document preserves commands, observed outcomes and baseline failures.

## Remaining limits

Late/line-spanning assertion and VTIME ownership, live timer writes,
startup/reset alignment, TIMEUP disputes, NMI delays, refresh/startup revisions,
physical access/recognition phases and full device synchronization (including
APU advancement before raster processing) remain unresolved. No physical-device
or commercial-cartridge accuracy claim follows from these tests.

Inventory scopes remain intact: `cpu`, `bus`, `scheduler`, `dma`, `hdma`,
`controllers`, `ppu-bg`, `ppu-offset`, `ppu-hires`, `ppu-mode7`, `ppu-objects`,
`ppu-color`, `ppu-ports`, `ppu-display`, `spc`, `ipl`, `dsp-voices`, `dsp-noise`,
`dsp-pitch`, `dsp-envelope`, `dsp-echo` and `dsp-timing` remain partial;
`ppu-raster` and `pal` remain unimplemented. No category becomes verified.
This bounded correction does not complete Issue #3 or establish commercial
compatibility. The optional 301-second wall-clock browser test remains excluded
from the normal browser command. No post-merge verification is required.
