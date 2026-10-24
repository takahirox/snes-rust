# Issue #87: ordinary SPC700 I/O readback

## Contract established before production edits

Inspected on 2026-10-24:

- [Anomie's SPC700 Doc](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/spc700.txt),
  revision 1126, dated 2007-04-21, **SPC700 REGISTERS**: the introduction
  explicitly specifies zero reads for write-only registers, and register
  overlay reads instead of underlying RAM. TEST `$00F0` and CONTROL `$00F1`
  are marked write-only, so both return zero. The `$00FA–$00FC` target
  entries additionally specify zero reads explicitly. Writes still update
  writable state and backing RAM. The timer description assigns output
  clearing to TnOUT reads, counting to enabled scaler pulses, and counting/
  output reset to rising enable. CONTROL port clearing is a write action.
- [Fullsnes, SNES APU SPC700 I/O Ports](https://problemkaputt.de/fullsnes.htm#snesapuspc700ioports),
  live document identifying itself as extracted from **no$sns v1.5** (no
  separate revision displayed): TEST, CONTROL and each timer divider are
  marked `(W)`. Their entries do not specify a returned value. This is no
  independent zero-value confirmation and no contrary value is given.
  Output reads clear their four-bit counters; CONTROL describes port-latch
  reset as a write function. Its CONTROL timer-reset wording differs from
  Anomie's stage-specific account; the existing
  [Issue #85 reconciliation](issue-85-validation.md) remains in force.

The supported ordinary read contract is therefore **zero at all five exact
page-0 addresses**, independently of backing RAM and previous writes. The
absence of write effects, output clearing, scaler restart or stage changes on
these reads is a derived expectation from those operation descriptions, not
a newly measured electrical claim. Reads of TnOUT retain their separate clear
effect. Neighboring controls test routing without asserting their complete
hardware semantics. TEST clock/RAM-control effects, physical bus accesses and
coincident edges, communication races and reset retention remain unresolved.
The guest will not write TEST; direct RAM sentinels are fixture inputs, not
physical power-on state or expected results.

No reference emulator is used as an oracle.

## Implementation and independent tests

Production adds one exact-address match arm in `Apu::read`. It returns zero
without touching RAM, CONTROL, timer or port state. `Apu::write`, timer
advancement, DSP and uploader code are unchanged. Page 1 remains ordinary RAM.

The eight original MIT tests in
[`spc_io_readback.rs`](../crates/snes-core/tests/spc_io_readback.rs) include:

- Separate TEST, CONTROL and T0/T1/T2 target tests. Distinct backing inputs are
  `$A1/$B2/$C3/$D4/$E5`; each read is repeated three times and preserves backing
  bytes. CONTROL and targets receive zero and nonzero writes; TEST receives
  only the documented normal `$0A`, never an unsafe zero/clock-mode write.
  CONTROL reads preserve restored nonzero inputs even after a port-clear write;
  actual CONTROL writes still clear the specified inputs and preserve outputs.
- Each of five selected addresses is read while each of three timers runs.
  For divider D=128/128/16, enable at `3D+D/4` with target 3. At `7D+D/2`,
  four enabled pulses have left counting=1 and output=1. Write live target 2,
  then read the selected register repeatedly. At `8D-2`, require accumulated
  output 1; at `8D`, require a new 1. A reset counting stage, phase restart,
  changed target or cleared output is distinguishable. Repeat reads at
  `9D+D/2`, requiring 0 at `10D-2` and 1 at `10D`. Then write zero; require
  0 at `266D-2` and 1 at `266D` (256 new pulses), with intervening selected
  reads. These are 15 scenarios with separated windows and explicit arithmetic.
- Neighbor controls require DSP index/data routing, high-index read masking
  and ignored writes, all four independent communication directions, F8/F9
  storage, ordinary RAM at `$00EF/$0100`, every selected page-1 address, and
  timer-output writes that preserve output followed by read-to-clear. Targets
  1/1/8 give exactly five accumulated events at cycle 648 on all three timers.

Default zero scaler phase, two-cycle idle advancement and instantaneous public
API operations are fixture conventions, not physical initialization or bus
measurements. No private timer fields are injected. All eleven existing
`spc_timers` tests and every original numeric guest record are unchanged.

### Uploaded guest and independent records

The original Rust builder emits SPC700 instructions uploaded to `$0200` through
this emulator's firmware-free port protocol. It stores actual reads at
`$1200–$1237`; record RAM starts at `$EE` to detect missing stores, not expected
zeros. The five distinct backing sentinels and page-1 values
`$61/$72/$83/$94/$A5` are fixture inputs. No expected results are injected.
[Provenance](../tests/roms/PROVENANCE.md#issue-87-original-spc700-io-readback-vectors-and-uploaded-guest)
records original authorship, MIT licensing and generated code/data provenance.

The independent 56-byte record contract, in execution order, is:

| Group | Required bytes | Reason |
| --- | --- | --- |
| Initial five registers, twice | ten zeros | Register overlay, independent of five nonzero backing bytes |
| DSP low/high index and data | `0C 25 8C 25` | Index readback, masked read and ignored high-index write |
| Four input ports after distinct output writes | `01 42 43 44` | Host inputs remain independent of outputs `50 51 52 53` |
| F8/F9 and ordinary RAM | `98 A9 76 87` | Written storage and fixture inputs |
| SETP direct-page selected addresses | `61 72 83 94 A5` | Page-1 RAM, no page-0 overlay |
| CLRP direct-page selected addresses | five zeros | Restores exact page-0 register decoding |
| After target/CONTROL writes | five zeros | Writes remain effective without readable backing |
| First timer window | `01 01 01` | One event on each timer |
| Selected reads between windows | five zeros | Readback while timers run |
| Second timer window | `01 01 01` | One further event each |
| Selected reads between windows | five zeros | Readback while timers run |
| Third timer window | `02 02 02` | Two further events each |

DSP master-left volume is 37 (`$25`) and FLG=`$20` disables echo writes.
No voices are keyed; silence is independently required. Audible DSP regressions
and exact 32 kHz cadence remain exercised by the existing suites.

The host starts at fixture cycle 8192, a shared scaler boundary. As in #85,
the 9-cycle polling loop, 7-cycle exit and instruction overshoot conservatively
place the first post-handshake instruction at `8192+s`, `0<=s<=18`. MOV dp,#imm
costs 5 cycles, MOV A,dp plus record store costs 3+5, SETP/CLRP cost 2, and
absolute read plus store costs 4+5. Each delay uses MOV Y,#n plus DBNZ Y:
`2+6(n-1)+4=6n`, with NOP (2) or harmless RAM read (3) for remainders.
No timer output determines a delay or a guest branch.

Targets are 3/3/24; CONTROL enables at relative cycle 600. Slow pulses at
640/768/896 give the first match at 896, then 1280/1664/2048. For T2, even
allowing the full five-cycle write interval w, matches are
`600+s+w+384*k-q`, `0<=w<=5`, `0<=q<=15`, `k>=1`. Selected reads at 620,
1300 and 1800 do not alter these schedules. Output reads begin at
1100/1110/1120, 1500/1510/1520 and 2200/2210/2220, relative to the instruction
origin. Entire read instruction windows remain between events for every s,
w and q above. Thus the required triples are independently 1/1/1, 1/1/1 and
2/2/2; this does not assert physical coincident-edge timing.

Code must be below 512 bytes, upload below 100000 master clocks, and each
acknowledgement below 1000 steps of 21 master clocks. Ready must still be
observable at cycle 8192. Completion port 0=`$87` and STOP are required by
60000 master clocks after start. Every advance rejects execution errors.
At master-clock checkpoints 10000/26000/36000/60000, each whole and irregular
run separately requires record prefixes of 32/40/48/56 bytes. Only then are all
PCM samples, DSP registers, RAM, input/output ports and public SPC registers,
PC and halted state compared. Final output ports must be `87 51 52 53`, and
inputs `01 42 43 44`. Finally both runs disable counting and compare public
timer outputs and their read-clear effects. Partition equality proves
software determinism, not hardware accuracy.

### Executed baseline failures

Before production edits, all eight new tests compiled and failed on the
unchanged selected worktree. An isolated source snapshot from
`git archive b98759ae45df6234f43d01511af6c12073a12710`, with only the new test
file copied in, reproduced **0 passed / 8 behavioral failures** using:

```sh
cargo test --locked --manifest-path .cache/issue-87/baseline/Cargo.toml \
  -p snes-core --test spc_io_readback
```

The separate register tests returned `$A1` for TEST, `$B2` for CONTROL and
`$C3/$D4/$E5` for T0/T1/T2 targets, each instead of the source-derived zero.
The guest's first 32 records returned the same five sentinels twice and again
after CLRP; all distinguishing DSP/port/storage/RAM/page-1 records in that
prefix matched their independent expectations. These failures establish each
corrected class and address without using emulator output to choose the oracle.
After the production correction, all eight tests pass. Local raw logs are in
`.cache/issue-87`; this record preserves the evidence without generated binaries.

### Mandatory commands and actual results

Executed on macOS arm64 on 2026-10-24, with native Rust 1.94.0, rustup stable
Rust 1.99.0 for the SPC assembler/Wasm, Node 24.12.0 and Python 3.13.13. The
README's `RUSTC` override selects the compiler with the installed Wasm target;
it changes no emulator dependency or source pin.

| Command | Actual result |
| --- | --- |
| `cargo test --locked -p snes-core --test spc_io_readback` | Passed: 8 tests, zero failed/ignored |
| `cargo test --locked -p snes-core --test spc_timers` | Passed: all 11 tests, original records unchanged |
| `cargo test --locked -p snes-core --lib apu::timing_tests` | Passed: both cadence/envelope/PCM tests |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --workspace --locked` | Passed: 443 tests, zero failed/ignored |
| `python3 tools/make-fixture.py .cache/smoke.bin` and `cmp tests/roms/smoke.bin .cache/smoke.bin` | Passed: byte-identical |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Passed after the download recovery below; all three hash-pinned assemblers built from source |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Passed: CPU full/basic and SPC, 600 frames each; final markers 0649/0452/0557, PCs 0081a2/0081a2/008060 |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Passed: 11 grayscale bands, 16 scanlines each; final shade persists through line 223 |
| `python3 tools/build-cleared-games.py` twice | Passed: two fresh builds, both original expected hashes |
| `npm --prefix web ci` | Passed: zero audit vulnerabilities |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Passed |
| `(cd web && npx playwright install chromium)` | Passed |
| `RUSTC="$(rustup which rustc)" SNES_TEST_PORT=18087 npm --prefix web test -- --grep-invert '301 seconds'` | Passed: all 37 selected tests, zero failures/skips, 2.9 minutes |

The browser selection retains Castle's 18100-frame gameplay and complete
five-map/credits route with freeze rejection; Memory's controller playthrough,
music-note duration/finite-effect checks; gesture-activated Web Audio; and all
queue/drift assertions. The filter excludes only the existing optional
unaudited `301 seconds` tests. No browser assertion or existing test was
removed or relaxed. Existing audible DSP and exact 32 kHz cadence assertions
are retained. All required gates ultimately passed; none remain failed,
skipped or unperformed.

The first assembler build rejected a Codeberg download with a checksum
mismatch. A separately bounded full transfer timed out after 90 seconds
(curl exit 28, 1126808 of 23872736 bytes). The server supports byte ranges;
32 bounded range requests to the same pinned archive URL assembled 23872736
bytes with the exact unchanged SHA-256
`979b0fbe19830ffb49d2f187a5512044b7514914f1d289686c340ffd1d6a6e63`.
The documented build command then passed, including the SPC assembler under
Rust 1.99. No checksum, source, dependency or tool pin was changed. These
resolved download attempts and the intentional baseline assertion failures
are the only failed validation attempts.

Both cleared-game builds reproduce Castle SHA-256
`197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`
and Memory SHA-256
`6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
Smoke remains
`6e5518b3959883cc22e914621fc5b5d9f5c0fe4eb96ee61000769afdbb68d6e5`.
Existing licensing/provenance decisions, source/tool pins and game hashes are
preserved. Source-built fixtures also reproduce their original hashes:

| Fixture | SHA-256 |
| --- | --- |
| CPU full | `1727e7c46ebb5df55711598264ba19005986bd34e915621383966adf674b3cfa` |
| CPU basic | `514d58950ae1ca940d6673c2a028b258b2ab17293bbc7cbd7cb54dded32abc5b` |
| SPC | `279d6a92d051ff8a98e190ce73e40482ff2cc981812509c7bca35a047484507e` |
| PPU/HDMA | `aef339e5d4a7319db31de577b253d977b45f5c45f09adfaa99a302ca300a957a` |

Local Markdown file/anchor checks and staged whitespace checks passed. Final
scope is the exact read-dispatch correction, the original Rust tests and
README/validation/provenance documentation. No external write, push, PR,
merge or comment is part of this node.

## Inventory and remaining qualifications

All 24 scopes and each scope's full outstanding qualifications in the
[Issue #83 inventory](issue-83-validation.md#inventory-and-remaining-work)
and [Issue #85 record](issue-85-validation.md#inventory-and-remaining-work)
remain in force. `cpu`, `bus`, `scheduler`, `dma`, `hdma`, `controllers`,
`ppu-bg`, `ppu-offset`, `ppu-hires`, `ppu-mode7`, `ppu-objects`, `ppu-color`,
`ppu-ports`, `ppu-display`, `spc`, `ipl`, `dsp-voices`, `dsp-noise`, `dsp-pitch`,
`dsp-envelope`, `dsp-echo` and `dsp-timing` remain **partial**; `ppu-raster` and
`pal` remain **unimplemented**. No category is promoted by this correction.

Only ordinary readback at these five addresses is resolved here. TEST clock/
RAM-control implementation, physical instruction/bus and coincident timer edges,
communication races, reset retention, SLEEP/STOP distinctions, comprehensive
SPC instruction/register semantics, IPL execution and DSP subcycles remain
outstanding. Existing BRR/filter/interpolation/envelope, live-header/directory/
key timing, PMON/NON, echo-RAM interaction and physical audio qualifications
are retained. Neighbor controls do not establish their complete semantics.
These tests do not establish commercial compatibility or complete Issue #3.
No optional or required post-merge check applies.
