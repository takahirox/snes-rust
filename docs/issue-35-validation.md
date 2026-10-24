# Issue #35: independent scanline HDMA phase

Implemented for [Issue #35](https://github.com/takahirox/snes-rust/issues/35),
starting at checkpoint `b4a0cb0ab40c141b3add7e36779d1f1a9f8589c3`.
Production baseline is `f86dc701828bf6f7b3b537454f2b75eb5c96a84d`.
All added instructions, tables and tests are original MIT work.

## Evidence and event terminology

Retrieved both cited references before changing production behavior on
2026-10-23. Ignored raw copies and logs are in `.cache/issue-35/`; third-party
reference text is not redistributed. No emulator implementation was consulted.

| Reference | Version / raw SHA-256 | Sections checked |
| --- | --- | --- |
| [Fullsnes](https://problemkaputt.de/fullsnes.htm) | no$sns v1.5; `3853b65a7331bf12eea431ff494a2948a1c78ef22f7e819db4b139fd5a79f6c6` | Timing H/V Counters; Timing H/V Events (raw HTML 26984–27030); DMA and HDMA Start/Enable Registers, Channel Registers and Notes (692–887) |
| [Anomie's Register Doc](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt) | Revision 1157, 2007-07-12; `19dfdf3f1c206655b8ede11bcd6dc053e36634f876819e40bb39cfb88a237dfd` | DMA AND HDMA → HDMA (raw 2422–2513), plus 420C and 43x0–43xA |

Fullsnes distinguishes HBlank status at H=274 from HDMA activity at H=278.
Anomie places the scanline procedure approximately at H=$116, including V=0
through V=$E0. Neither cited section establishes a separate earlier internal
request edge or an exact first-byte strobe at that position. Anomie separately
describes approximately 18 master clocks of scanline overhead, eight per active
channel and additional indirect-address work. These facts do not establish that
H=274 is a hardware request followed by exactly 16 clocks of startup.

Accordingly, H=278 is **modeled service eligibility**: crossing 1112 master
clocks latches `hdma_pending`; the next CPU bus boundary or boundary between DMA
bytes consumes it. The name `pending` describes emulator scheduling state, not a
claim about a physical request pin or internal edge. Service starts the retained
byte procedure without adding startup. Exact hardware request/startup/bus-strobe
alignment remains unresolved. This bounded correction does not claim that the
first hardware source read occurs at 1116 or commitment at 1120.

The following early horizontal events use four master clocks per dot and an
ordinary 1364-clock scanline. `S` denotes the actual modeled service boundary, which can be later
than 1112. Times below are emulator conventions except for the reference phase
positions and algorithmic ordering explicitly identified.

| Event | Master clock / operation | Basis and limit |
| --- | --- | --- |
| Scanline-0 initialization eligibility | 24, then service at next bus boundary | Retained H≈6 convention; startup/alignment omitted |
| HBlank status and completed-line rendering | 1096; render V=1..224, status on every line | H=274 status from Fullsnes; whole-line rendering here is a retained emulator convention |
| Scanline HDMA eligibility | 1112 on V=0..224, including line 224; none in VBlank | Reference H=278 activity mapped to service eligibility, independently of rendering |
| Internal request versus startup | No additional earlier request or startup interval modeled | References do not justify an exact earlier edge; global/per-channel overhead remains incomplete |
| First payload source sampling | S+4 | Retained `transfer` half-byte timing; no claim of exact hardware alignment |
| First payload destination commitment / pointer advance | S+8 | Retained byte completion, then successive bytes/channels in priority order |
| Decrement and row-end reload | After **all** eligible payloads; once per enabled active channel, including holds | Preserve Issue #33's source-backed two-pass ordering; new rows wait until the next event |
| Descriptor/pointer reads | First reload read at end of payload pass, eight clocks per read | Retained `hdma_read` convention; publishes descriptor after its read interval and pointer after its reads |

For service exactly at 1112, one-byte direct `$81` payload samples at 1116,
commits at 1120, reads the next descriptor at 1120 and finishes at 1128.
The analogous indirect row reload adds two pointer reads and finishes at 1144.
An access beginning at 1110 can finish at 1116 (six clocks) or 1118 (eight),
creating eligibility during the access; its read/write completes before HDMA
service at the following boundary. Thus a CPU payload write ending at 1118 is
sampled by the later transfer. These are explicit model-relative latencies.

`tick` checks each threshold independently within each scanline. Service clears
the pending bit before doing work. Ordinary CPU accesses, DMA byte boundaries,
and maximum supported HDMA work are shorter than a scanline; service therefore
cannot accumulate a second supported scanline event before consuming the first.
Raster wrap comparisons also avoid re-requesting an event on the next line.
Initialization, programmed HDMAEN, priority, held rows and terminating indirect
reads use the unchanged Issue #33 machinery.

The Issue #31/#33 records' references to the old H=274 request are historical,
not supported request-edge evidence. Their terminology is corrected by this
record. No existing test assertion, fixture pin, game input, frame count, audio
check or freeze control was weakened or changed.

## Original regressions and baseline observations

Five tests in `machine/hdma_phase_tests.rs` configure registers and original
WRAM tables before natural initialization. Clock positioning uses only normal
six/eight-clock CPU accesses, not injected raster/pending/active/transfer/line
state or direct `tick`/`transfer` calls.

- Direct/indirect vectors check before/at/after HBlank and 1112, exact-threshold
  arrival, crossing accesses, subsequent service, live payload edits, table and
  indirect pointers, line counters, and no same-event next-row transfer.
- The visible-line sentinel is unrendered at 1094, white after HBlank entry,
  and stays white after HDMA changes brightness from 15 to 8. The next rendered
  line is `[136,136,136,255]`. A 12-clock access across the scanline boundary
  leaves the counter unchanged; the next phase terminates the row.
- All 225 supported scanline events are checked: `$80` transfers once then
  holds, `$E1` supplies the final 97 events, termination occurs at line 224's
  corrected phase. Every VBlank line excludes a pending event. The following
  frame reinitializes at 24 and transfers at 1112 again, retaining `$420C`.
- Reverse payloads read SLHV: the latched dot is 279 when service starts at
  1112, distinguishing source sampling at +4 from the subsequent write at +8.
  The retained deterministic SLHV return/open byte is tested as model behavior.
- A different-channel DMA starts at 1082: 16 setup clocks end at 1098, bytes
  finish at 1106 and 1114, then HDMA preempts. A two-byte run produces CGRAM
  `00 01 11 12`, whereas baseline produces `11 12 00 01`. A 400-byte run retains
  the entire 406-byte interleaved stream without wrapping CGRAM; a 600-byte run
  also checks circular wrap. Both span three events and compare every CGRAM
  slot, detecting misplaced, duplicated and lost bytes. HDMA inserts before
  DMA indices 2/165/328 (direct) or 2/163/326 (indirect), derived from eight
  clocks per byte, metadata work and 40-clock refresh holds. The 400/600-byte runs finish DMA at
  `$7E1190`/`$7E1258`, count zero, HDMA terminated with correct table/pointer effects,
  raster beyond line 3, and APU cadence within one sample. Aggregate clocks
  are 5040 direct or 5064 indirect: 16 setup + 4800 DMA + 160 refresh + 48 HDMA
  payload + 16/40 metadata. The 400-byte totals are 3360/3384, with two refresh
  holds instead of four. These are accounting checks, not hardware claims.

On baseline production, the focused phase test fails because HBlank entry
already sets `hdma_pending`. The short DMA order also fails as recorded above.
The two CPU guests in `tests/hdma_phase.rs` independently fail on baseline:

| Guest | Baseline WRAM `$1800..$1806` (driven status bits masked) | Corrected |
| --- | --- | --- |
| Direct | `03 03 10 82 00 00 40` | `01 03 10 82 00 00 40` |
| Indirect | `06 06 10 82 00 13 40` | `03 06 10 82 00 13 40` |

Guests configure during VBlank and execute WAI with masked H/V IRQ at V=0,
HTIME=267 after [Issue #99](issue-99-validation.md): `267*4+14=1082`
assertion clocks. In this model, six-clock WAI polling wakes at 1082..1086.
An eight-bit slow-ROM `LDA abs` takes three eight-clock instruction fetches plus a six-clock
register read. Its data access starts at 1106..1110 and ends at 1112..1116.
The corrected phase cannot service before that read. The historical baseline
results above used the original HTIME=270 guest and immediate-comparison
convention; its old HDMA phase serviced during operand fetches and exposed
already-reloaded metadata. Later accesses still see row-end state and HBlank.
The IRQ assertion offset is now source-derived; WAI polling, service boundaries
and complete CPU alignment remain model conventions. The guest numerically
distinguishes the phases without a broad wait past both. Host code supplies only the cartridge and observes WRAM,
terminal state and bounded deadlines. [Provenance](../tests/roms/PROVENANCE.md#original-hdma-phase-cpu-guests-approved)
records the original MIT code/data; no firmware or commercial ROM is used.

## Mandatory validation

All mandatory gates passed. Rust workspace checks use Homebrew Rust 1.94.0;
Wasm and the auxiliary SPC assembler use rustup Rust 1.99.0 via the README's
`RUSTC="$(rustup which rustc)"` selection. Logs and generated evidence remain
ignored under `.cache/` and `web/test-results/`.

| Command | Actual result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Pass |
| `cargo test --workspace --locked` | Pass: 195 tests, none failed/ignored; doctests pass |
| `cargo test --locked -p snes-core --lib machine::` | Pass: 84 tests, including five new phase tests |
| `cargo test --locked -p snes-core --test dma_access` | Pass: six existing CPU guests |
| `cargo test --locked -p snes-core --test hdma_init` | Pass: two existing CPU guests |
| `cargo test --locked -p snes-core --test hdma_reload` | Pass: two existing CPU guests |
| `cargo test --locked -p snes-core --test hdma_phase` | Pass: two new CPU guests |
| `python3 tools/make-fixture.py .cache/smoke.bin` | Pass |
| `cmp tests/roms/smoke.bin .cache/smoke.bin` | Pass: identical |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Pass: all three pinned assemblers built; initial download failure described below |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Pass: full CPU `$0649`, basic CPU `$0452`, SPC `$0557`; expected terminal PCs, 600 frames each |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Pass: eleven grayscale bands, final shade through line 223 |
| `python3 tools/build-cleared-games.py` (twice) | Pass: both fresh builds reproduce both expected hashes |
| `npm --prefix web ci` | Pass: zero reported vulnerabilities |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Pass |
| `(cd web && npx playwright install chromium)` | Pass |
| `SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'` | Pass: all 18 checks, 2.6 minutes |
| `git diff --check` and staged whitespace check | Pass |

Both Castle and Memory retain 18,100 frames (301.17 emulated seconds), twenty
gameplay windows and freeze controls. Castle completes all five maps and credits
at frame 11,097; Memory wins/restarts eighteen times. Independent music and
finite effects, gesture-activated Web Audio, queue/drift, pause/reset, SRAM
failure recovery and Pages subpath checks pass. No game tests or inputs changed.
Port 18085 was selected before starting the browser suite.

Reproduced hashes:

- Smoke: `73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb`.
- Castle: `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`.
- Memory: `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
- Licensed HDMA: `aef339e5d4a7319db31de577b253d977b45f5c45f09adfaa99a302ca300a957a`.

The initial tool build rejected an SPC archive hash mismatch. A separate bounded
HTTP/1.1 download ended after 7,021,552 of 23,872,736 advertised bytes. Five
byte-range responses supplied the remainder; the complete archive exactly
matched the unchanged `979b0fbe19830ffb49d2f187a5512044b7514914f1d289686c340ffd1d6a6e63`
pin before entering the tool cache. The final mandatory build and hardware gates
then passed. No tool source, pin or licensing decision changed.

During test development, exact-clock helper arithmetic and initial guest status
placement were corrected before the retained baseline comparison and passing
gates. The final focused phase test, short DMA order test and both guests were
also run with production `machine.rs` from the evaluation baseline, then the
corrected file was restored. All failed as described above. Final workspace,
clippy and focused gates passed again after strengthening the new DMA vector
with the non-wrapping 400-byte case. No mandatory check remains pending or skipped.

## Remaining limits

HDMA remains partial: exact request/startup/alignment/full overhead, CPU revision
differences, arbitrary mid-frame enabling, same-channel contention, electrical
open bus, PAL, interlace/overscan and pixel-fetch rendering are outside this
change. Limited passes establish neither complete hardware accuracy nor
commercial compatibility. Existing provenance decisions remain in force.

All inventory scopes/statuses remain unchanged: `cpu`, `bus`, `scheduler`,
`dma`, `hdma`, `controllers`, `ppu-bg`, `ppu-offset`, `ppu-mode7`, `ppu-objects`,
`ppu-color`, `ppu-ports`, `ppu-display`, `spc`, `ipl`, `dsp-noise`, `dsp-pitch`,
`dsp-envelope`, `dsp-echo`, `dsp-timing` remain partial; `dsp-voices` remains
implemented_unverified; `ppu-hires`, `ppu-raster`, `pal` remain unimplemented.
This resolves one scheduling gap, not whole-category completion or completion
of Issue #3. No optional or required post-merge verification is specified.

Issue #95 qualifies this conversion: latched H=323/327 last six clocks on
ordinary lines, and field 1 V=240 is 1360 clocks. The early events above precede
those dots; `H*4` is not a universal PPU counter formula.
