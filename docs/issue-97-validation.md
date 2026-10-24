# Issue #97: V-only IRQ assertion at master H=10

## Specification and bounded contract

Re-read on 2026-10-24 before completing implementation:

- [Fullsnes](https://problemkaputt.de/fullsnes.htm), **SNES Timing H/V Events**
  detailed list, and **SNES PPU Interrupts**, **4200h NMITIMEN**,
  **4209h/420Ah VTIME**, **4211h TIMEUP**. The freshly retrieved HTML SHA-256 is
  `3853b65a7331bf12eea431ff494a2948a1c78ef22f7e819db4b139fd5a79f6c6`,
  matching the [#95 reconciliation](issue-95-validation.md).
- [Anomie's Register Doc](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt),
  revision 1157, **4200 NMITIMEN**, **4209/420A VTIME**, **4211 TIMEUP**.
  The [#23 access reconciliation](issue-23-validation.md) supplies the retained
  DMA status-register access rule.

Fullsnes's detailed event list specifies V-only assertion at H=2.5, V=VTIME;
its H and nonzero-HV rule is HTIME+3.5. Anomie describes V-only IRQ shortly
following V equality. Early dots have four master clocks, hence 2.5*4=10
elapsed clocks. This establishes the bounded V-only runtime offset. The same
Fullsnes list also mentions HV with HTIME=0; that mode is outside #97's scope
and remains an explicitly approximate control along with H-only/HV generally.
The terse equality descriptions are not evidence for immediate H=0 assertion.

Both references describe read acknowledgement; Fullsnes qualifies coincident
reads with a short condition window. Anomie's older write-clear statement
also differs from Fullsnes's read-only presentation. #97 preserves existing
register semantics and does not resolve either physical coincident-edge races
or writes to TIMEUP. Tests model the existing access-end sample and immediate
read acknowledgement, including at H=10, as an emulator convention.

Scope: NTSC non-interlaced operation, configuration established before entering
the matching scanline. VTIME chooses that line; HTIME is ignored. TIMEUP stays
clear through H=9 and asserts at H=10 once per matching line. Acknowledgement
consumes the flag; a pending flag survives subsequent lines/fields. VTIME 262
and 511 cannot match this raster. Startup retains the earlier convention:
initial V=0 is not entered by rollover and synthesizes no V-only event. VTIME=0
first fires after natural rollover. Physical reset and live configuration
semantics are unverified.

The scheduler detects an old-H/new-H crossing of 10 within each matching line
and removes its old V-only H=0 rollover assertion. Large ticks naturally split
at line boundaries; CPU and DMA reads see all elapsed clock events before
sampling. The IRQ flag represents line assertion. `Machine::step` still samples
interrupts at instruction boundaries, after pending DMA completes. This change
does not add within-instruction CPU interrupt recognition.

## Independent clock arithmetic and coverage

The source-derived #95 geometry remains unchanged: 1364 clocks per ordinary
line, 1360 at field 1 V=240, 262 lines, alternating field totals
357368/357364. Field starts are 0,357368,714732,1072100. For any target,
add 1364*VTIME and subtract four only after V=240 in an odd field; then add ten
for assertion. Original literal event vectors include:

| Field | VTIME | Absolute assertion clock |
| --- | --- | --- |
| 0 | 1 | 1374 |
| 0 | 240 | 327370 |
| 0 | 241 | 328734 |
| 1 | 0 | 357378 |
| 1 | 240 | 684738 |
| 1 | 241 | 686098 |
| 2 | 0 | 714742 |

`cargo test --locked -p snes-core v_only_irq` executes seven unit regressions
(including the corrected earlier VTIME=0 contract) and one CPU guest:

- VTIME 0/1/225/239/240/241/261, both field parities, HTIME 0/321/511,
  H=0/9/10/11, nonmatching lines, unreachable VTIME 262/511, immediate
  acknowledgement, no later same-line assertion, next-field delivery, and
  pending retention through a subsequent field/event. Fresh before/at/after
  runs prevent read acknowledgement from hiding the flag.
- One-clock, irregular 1/7/1361/2/43/8193/5-clock chunks and whole intervals
  use independent absolute events, including field rollover and the short
  line. They also check final geometry and independently calculated audio
  counts, then compare PCM across partitions for determinism.
- Six-clock CPU TIMEUP reads end exactly at H=0/9/10/11 in matching lines,
  including both natural V=0 rollovers and short-line rollover into V=241.
  Early reads do not consume the future event. This is access-end sampling
  convention coverage, not physical bus-edge verification.
- Real general DMA uses fixed A=$004211 to B=WMDATA for three bytes. Existing
  phases are 8 global setup + 8 channel setup, then read at +4/write at +8
  per byte. Starting event-21 gives samples at event-1/+7/+15, hence bit-7
  records 0/$80/0; starting event-19 gives +1/+9/+17, hence $80/0/0. Counts,
  address retention, WMDATA increments, final raster/clocks, acknowledgement
  and subsequent clear reads are checked across ordinary and rollover events.
- A 100-byte real DMA crosses assertion, continues audio/PPU/raster advancement,
  retains IRQ and holds a waiting, unmasked CPU until completion. From clock
  1353 it takes 16+100*8+40=856 clocks (one refresh), ending at clock 2209,
  V=1/H=845. No CPU instructions or stack writes occur during DMA; the next
  `Machine::step` takes exactly one three-byte emulation-mode IRQ frame and
  executes the handler NOP. Every destination byte and final transfer count
  is checked. These are documented existing DMA phases, not verified hardware
  startup/alignment timings.
- The original source-built 65C816 guest naturally visits four V=240 events in
  fields 0,1,0,1, records counter/parity/TIMEUP data and self-checks all records
  before publishing success. Its expected H=10 captures distinguish baseline
  H=7/8. Construction, MIT authorship, independent arithmetic and both bounded
  deadlines are in [PROVENANCE](../tests/roms/PROVENANCE.md#original-v-only-irq-timing-guest-issue-97).
  The host injects no timing/IRQ/result state. The #95 geometry guest and all
  existing geometry assertions are preserved.

## Exact-baseline behavioral counterexamples

Exported the exact evaluation baseline with:

```sh
mkdir -p .cache/issue97/baseline
git archive a06fe54568a651c45c550053cc1dee4429e44d1c | tar -x -C .cache/issue97/baseline
```

Copied only the new unit and guest source files, registered the unit module and
made the existing test-only `timer_bus`/`timeup` helpers visible to that sibling
module. No baseline runtime code changed. Both commands compiled, then exited
101 on behavioral assertions:

```sh
cargo test --manifest-path .cache/issue97/baseline/Cargo.toml --locked -p snes-core --lib v_only_irq_boundary_matrix
cargo test --manifest-path .cache/issue97/baseline/Cargo.toml --locked -p snes-core --test v_only_irq
```

The boundary test finds TIMEUP set at field 1 V=0/H=0, where it must be clear.
The guest collects all four fields, then self-check STPs on its first H mismatch:
actual H=7,8,7,8, expected 10,10,10,10. Other record bytes agree (V=240, parity
0/1/0/1, first TIMEUP=$80, second=$00). The baseline never publishes success.
The corrected implementation passes all eight focused tests.

## Executed acceptance gates (2026-10-24)

Detailed transient logs are under ignored `.cache/issue97`; durable results are
recorded here. No test was removed or tolerance relaxed.

| Command | Actual result |
| --- | --- |
| `cargo test --locked -p snes-core v_only_irq` | Pass: seven unit regressions and one self-checking CPU guest |
| Exact-baseline boundary and guest commands above | Both compile and fail behaviorally as expected; no runtime transplantation |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Pass |
| `cargo test --workspace --locked` | Pass: 479 tests (208 core unit tests), zero failures/ignored tests; CPU interrupt/WAI, #95 geometry guest, controllers, DMA/HDMA, PPU ports and audio regressions retained |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Pass: all three pinned assemblers built; README compiler selection used for the auxiliary SPC tool |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Pass: full/basic CPU suites (terminal $0081A2, last cases $0649/$0452) and SPC suite (terminal $008060, last case $0557) |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Pass: all 11 grayscale bands of 16 lines, final shade through line 223 |
| `python3 tools/make-fixture.py .cache/issue97/smoke.bin` and `cmp tests/roms/smoke.bin .cache/issue97/smoke.bin` | Pass: SHA-256 `6e5518b3959883cc22e914621fc5b5d9f5c0fe4eb96ee61000769afdbb68d6e5` |
| `python3 tools/build-cleared-games.py` twice | Both pass: fresh pinned source builds with unchanged Castle/Memory ROM hashes |
| `npm --prefix web ci` | Pass |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Pass: fresh release Wasm and browser artifact |
| `npm --prefix web exec playwright install chromium` | Pass |
| `SNES_TEST_PORT=18097 npm --prefix web test -- --grep-invert '301 seconds'` | Pass: 37 tests in 3.0 minutes, including gameplay, traversal/progression, music/effects, replay, persistence, deadlines and audio drift/queue assertions |
| `git diff --cached --check` | Pass |

The required emulated game replays each complete 18,100 frames /
301.1706794 emulated seconds. Castle maximum cumulative audio drift is
1.42055 samples; Memory is 1.41970 (existing limit 128). Memory completes
18 wins. Castle's unchanged 11097-frame route traverses all five maps and
reaches credits with state 3, scroll 287, hash `97663f45`. No golden values,
input sequence, deadline or tolerance was edited.

Castle ROM SHA-256 remains
`197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`;
Memory remains `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
Hardware fixture source pin remains `5ecdf555da920f0bd7b157542141965a8120186d`;
PPU fixture source pin remains `ac6ef8006809c0ff5dabc8ff137a4623967697fe`.
Assembler sources and license decisions remain in the existing manifests and
provenance records.

## Remaining uncertainty and inventory

No physical device corroboration was performed; it is optional for this bounded
correction. The optional excluded 301-second real-time browser soaks were not
run. The required emulated five-minute game replays remain in the browser gate.
No post-merge verification is required.

H-only/HV delays and late HTIME, live enable/VTIME writes, physical coincident
read/write races, startup/warm-reset retention, NMI flag/assertion delays, CPU
sampling within instructions, DMA startup/alignment, interlace, overscan, PAL
and broader device synchronization remain unresolved. The earlier #95 record's
V-only H=0 limitation is superseded only for the bounded runtime case above.

All 24 inventory categories remain represented: `cpu`, `bus`, `scheduler`,
`dma`, `hdma`, `controllers`, `ppu-bg`, `ppu-offset`, `ppu-hires`, `ppu-mode7`,
`ppu-objects`, `ppu-color`, `ppu-ports`, `ppu-display`, `spc`, `ipl`, `dsp-voices`,
`dsp-noise`, `dsp-pitch`, `dsp-envelope`, `dsp-echo`, `dsp-timing` are still
partial (22); `ppu-raster` and `pal` are still unimplemented (two). None is fully
hardware verified. No commercial ROM or firmware was acquired. Existing licensed
source-built fixtures, provenance decisions and source/tool/game pins remain
unchanged. This correction does not complete Issue #3 or establish broad
commercial compatibility.
