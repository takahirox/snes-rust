# Issue #101: combined HTIME=0 IRQ at master H=10

## Source reconciliation (before implementation, 2026-10-24)

Independently retrieved [Fullsnes](https://problemkaputt.de/fullsnes.htm),
SNES Timing H/V Events detailed list and SNES PPU Interrupts, and
[Anomie's Register Doc, revision 1157, dated 2007-07-12](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt),
NMITIMEN/HTIME/VTIME/TIMEUP. Fullsnes explicitly places combined HTIME=0
alongside V-only at 2.5 timer units on the VTIME-selected line. Multiplying
by four yields 10 master clocks. Its nonzero horizontal formula instead adds
3.5 units; using that formula at zero would incorrectly yield 14 clocks.
Anomie describes IRQ following comparison, with V equality gating combined
mode. Neither document's terse register equality summary establishes immediate
assertion. The detailed numerical event resolves the earlier H=0 test
approximation, consistent with the reconciliations for #97 and #99.

Assertion is distinct from acknowledgement and CPU recognition. Fullsnes
qualifies read acknowledgement during a 4–8-clock condition window; Anomie's
older description also mentions write clearing. Those differences remain
unresolved and outside this change. Access-end reads, immediate acknowledgement,
DMA phases and instruction-boundary CPU service remain model conventions.

The supported domain is preconfigured NTSC non-interlaced combined mode with
both HTIME bytes zero, on naturally entered matching lines. Initial reset V=0
retains its no-event policy, without claiming verified physical reset alignment.
H-only zero, late line-spanning comparisons/VTIME ownership, live configuration,
TIMEUP write/coincident-read hardware behavior, instruction-level recognition,
NMI delay, DMA alignment, PAL/interlace/overscan and pixel fetch remain outside
scope. This supersedes only combined-zero approximation statements in #97/#99.

## Implementation and independent coverage

The existing V-only crossing at 10 is also used for combined mode with zero
HTIME. The combined-mode rollover assertion is removed; H-only-zero rollover
and nonzero horizontal scheduling remain unchanged. No event is synthesized
on initial reset V=0. Large ticks still split at scanline boundaries.

`cargo test --locked -p snes-core combined_zero_irq` runs seven unit tests
(including the corrected older combined-zero timer test) and one CPU guest:

- Fresh independent observations at H=0/9/10/11 cover VTIME
  0/1/224/225/239/240/241/261 in naturally reached fields 1 and 2. Earlier
  flags are cleared at target start minus one, before entry, so a wrong H=0
  event cannot be hidden by acknowledgement. Early reads preserve the future
  event; acknowledgement never duplicates delivery later in the line.
- Nonmatching lines and unreachable 262/511 are rejected. Pending survives
  lines, fields and another selected event. The reset-created V=0 control
  records retained policy, not physical hardware evidence.
- One-clock, irregular 1/7/1361/2/43/8193/5 and whole intervals use independently
  derived literal events: 1374, 357378/714742, 327370/684738 and
  328734/686098. Geometry uses ordinary 1364-clock lines, short field-1
  V=240 of 1360 and field starts 0/357368/714732. Final geometry is
  714752, field 2/V=0/H=20. Stereo sample count is independently
  `2*floor(714752*32000/21477272)`. PCM partition equality establishes
  determinism only.
- Six-clock CPU TIMEUP reads end at H=0/9/10/11 on ordinary, natural V=0,
  short V=240 and following V=241 lines. Access-end sampling and immediate
  coincident acknowledgement remain emulator conventions.
- Real fixed-A $004211 -> WMDATA DMA starts at event-21/-20/-19, sampling
  at event-1/+7/+15, event/+8/+16 and event+1/+9/+17. Bit-7 destinations
  are [0,$80,0], [$80,0,0], [$80,0,0]. Checks cover counts, fixed addresses,
  all three destinations, WMDATA pointer, pending-mask clearing, final clocks/
  raster and acknowledgement without reassertion. Existing phases are
  8 global + 8 channel setup and read +4/write +8 per byte, not physical
  alignment verification.
- A real 100-byte WRAM -> VRAM DMA starts at 1353 and crosses event 1374
  with an unmasked waiting CPU. Existing costs are 16+800+40=856, ending
  at 2209/V=1/H=845. Device geometry, audio sample count and every payload
  byte are checked. CPU PC, stack pointer, stack memory and instruction count
  remain unchanged throughout DMA, with IRQ retained. Resuming executes one
  IRQ entry (three stack bytes) and handler NOP under the existing CPU-service
  convention; TIMEUP remains pending until acknowledged.
- The original source-built guest preconfigures combined mode, both HTIME
  bytes zero, VTIME=240 and SETINI=0. It naturally visits four fields and
  checks all 32 literal counter/parity/acknowledgement bytes in executed
  65C816 code, with explicit failure/success markers and both deadlines.
  [Provenance](../tests/roms/PROVENANCE.md#original-combined-zero-irq-diagnostic-issue-101)
  records MIT authorship, construction and independent clock derivations.

## Exact-baseline behavioral counterexamples

Exported exact baseline c6d12caa8a0e5d4eb66c1ca88e94d55ee6de68d2 and copied
only the new test sources and module registration, with no runtime changes:

```sh
mkdir -p .cache/issue101/baseline
git archive c6d12caa8a0e5d4eb66c1ca88e94d55ee6de68d2 | tar -x -C .cache/issue101/baseline
cp crates/snes-core/src/machine/combined_zero_irq_tests.rs .cache/issue101/baseline/crates/snes-core/src/machine/
cp crates/snes-core/tests/combined_zero_irq.rs .cache/issue101/baseline/crates/snes-core/tests/
cat >> .cache/issue101/baseline/crates/snes-core/src/machine.rs <<'RS'

#[cfg(test)]
#[path = "machine/combined_zero_irq_tests.rs"]
mod combined_zero_irq_tests;
RS
cargo test --manifest-path .cache/issue101/baseline/Cargo.toml --locked -p snes-core --lib combined_zero_irq_fresh_boundary_matrix_and_no_duplicate
cargo test --manifest-path .cache/issue101/baseline/Cargo.toml --locked -p snes-core --test combined_zero_irq
```

Both compile successfully and exit 101 on behavioral assertions. The boundary
matrix finds TIMEUP set at field 1/V=0/H=0 where it must be clear. The guest
collects four fields and rejects baseline H=7,8,7,8 instead of 10,10,10,10;
all other record bytes agree. It writes failure=$EE, success=$00. Corrected
implementation passes both, with failure=$00 and success=$A5.

## Executed acceptance gates (2026-10-24)

| Command | Actual result |
| --- | --- |
| `cargo test --locked -p snes-core combined_zero_irq` | Pass: 7 unit tests + 1 guest |
| `cargo test --locked -p snes-core v_only_irq` | Pass: 7 unit tests + 1 guest |
| `cargo test --locked -p snes-core horizontal_irq` | Pass: 7 unit tests + 1 guest; H-only-zero and late controls preserved |
| `cargo test --locked -p snes-core --test ntsc_timing` | Pass: four-field geometry guest |
| `cargo test --locked -p snes-core --lib machine::ntsc_timing_tests` | Pass: 8 tests |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Pass |
| `cargo test --workspace --locked` | Pass: 495 tests, 0 failed, 0 ignored |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Pass: three pinned assemblers |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Pass: full/basic CPU and SPC suites |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Pass: all 11 grayscale HDMA bands |
| `python3 tools/build-cleared-games.py` (twice) | Pass: both pinned hashes match on both builds |
| `npm --prefix web ci` | Pass |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Pass: fresh Wasm and browser artifacts |
| `npm --prefix web exec playwright install chromium` | Pass |
| `SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'` | Pass: all 37 checks, 2.9 minutes |

The initial default-compiler assembler build failed because Homebrew rustc
1.94 cannot compile the pinned SPC assembler. Re-running with the README's
rustup compiler selection succeeds, without changing tool pins or sources.
The first browser invocation could not bind occupied port 8080; use the
README-supported `SNES_TEST_PORT=18085` override for this worktree.

Hardware gates retain existing licensed source pins, font replacements and
native assertions. Full/basic CPU suites reach $0081A2 with last tests
$0649/$0452; SPC reaches $008060 with $0557. The HDMA cartridge hash remains
`aef339e5d4a7319db31de577b253d977b45f5c45f09adfaa99a302ca300a957a`.
Cleared-game SHA-256 hashes remain Castle
`197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`
and Memory
`6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.

All existing CPU/WAI, geometry, controller, DMA/HDMA, PPU-port, audio and game
regressions remain intact. No other timing-derived expectations were changed.
Inventory scopes remain unchanged: `cpu`, `bus`, `scheduler`, `dma`, `hdma`,
`controllers`, `ppu-bg`, `ppu-offset`, `ppu-hires`, `ppu-mode7`, `ppu-objects`,
`ppu-color`, `ppu-ports`, `ppu-display`, `spc`, `ipl`, `dsp-voices`, `dsp-noise`,
`dsp-pitch`, `dsp-envelope`, `dsp-echo` and `dsp-timing` remain partial;
`ppu-raster` and `pal` remain unimplemented. None is fully verified. This
completes one scheduler sub-behavior and does not complete Issue #3 or establish
commercial-game compatibility. There is no required post-merge verification.

The browser gate uses freshly built Wasm and the pinned source-built games.
Castle and Memory each execute 18100 fields (301.170679 emulated seconds);
Castle's separate controller route reaches all five maps and credits in 11097
fields. Memory completes 18 wins. Existing looping music, independent finite
effects and gesture-activated Web Audio checks pass. Maximum cumulative audio
drift is 1.421 stereo samples for Castle and 1.420 for Memory, below the
unchanged 128-sample bound. All 37 normal browser checks pass; the optional
wall-clock test excluded by the normal documented command remains excluded.

Reproducible gate commands are in the table above; local raw logs are under
`.cache/issue101/`, hardware summaries under `.cache/hardware-tests/results.json`
and `.cache/ppu-test/results.json`, game hashes under
`.cache/cleared-games/results.json`, and browser evidence under
`web/test-results/`. These ignored build artifacts are not checkpoint files;
this record preserves the executed outcomes and commands.
