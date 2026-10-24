# Issue #85: free-running SPC700 timer prescalers

## Specification reconciliation before production changes

Fetched and inspected on **2026-10-24**, before editing production code:

- [Anomie's SPC700 Doc, revision 1126](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/spc700.txt),
  dated 2007-04-21 15:07:05 -0400, **SPC700 REGISTERS**, `$00F1` and
  `$00FA–$00FF`, lines 123–136 and 179–226. The three-stage description credits
  TRAC. The decisive exact passages are “Stage 1 runs constantly, and cannot be
  stopped or reset.” and “Stage 1 'counter' is not reset.” CONTROL identifies
  “Stage 2 and 3 counters” as the counters cleared on a 0-to-1 transition.
  Stage 1 is the 128:1 or 16:1 scaler; stage 2 is the eight-bit post-increment
  equality counter; stage 3 is the four-bit output. The stage-2 increment is
  explicitly conditional on enable. Thus disabling gates counting without
  changing phase or clearing output; a repeated enabled write has no rising
  transition and clears neither counter. Output reads clear stage 3 alone.
  Target writes retain the counting stage and phase, including the documented
  wraparound example. These are documentation-derived noncoincident rules.
- [Fullsnes, extracted from no$sns v1.5](https://problemkaputt.de/fullsnes.htm#snesapuspc700ioports),
  **SNES APU SPC700 I/O Ports**, CONTROL, T0DIV/T1DIV/T2DIV and
  T0OUT/T1OUT/T2OUT; **SNES Timings / APU Timings**. Its exact CONTROL
  shorthand is “0=Disable, set TnOUT=0 & reload divider, 1=Enable”; output is
  “reset to 0 after reading”. The target entries specify divisors 1–256 and
  enable-gated output; timing specifies 1.024 MHz SPC, 128/128/16-cycle timer
  sources and 32 kHz DSP. Its disable-clear/reload shorthand conflicts with
  Anomie's explicit stage-specific rules. We follow the latter for this Issue;
  we do not interpret that shorthand as resetting the free-running scaler.

The implementation contract preserves the Issue #83 live comparator: each
enabled scaler pulse increments stage 2 modulo 256, compares the incremented
value to the live target, and on equality zeroes stage 2 and increments stage 3
modulo 16. Target zero therefore takes 256 pulses. Noncoincident target writes
and output reads never restart the scaler. Enable changes are per timer;
unrelated CONTROL port-clearing effects remain intact.

The zero phase in `Apu::default` is a deterministic emulator initialization
convention, **not physical power-on alignment**. Public API register writes take
no time. Idle fixtures advance in two-cycle units, so even-cycle observations
at boundary minus two and boundary bracket a pulse. For divider D, nonzero
enable phase p and target N (zero means 256), the first event is
`D - p + (N - 1)D` cycles after enable; later events are `N*D` apart.
For enable at `3D + D/2`, target 1, observations at `4D-2` and `4D` must be
0 and 1: slow timers first fire at 512 rather than 576; T2 at 64 rather than 72.
These are independently derived arithmetic expectations, not emulator captures.

Physical enable/read/write edges, coincident operations, target-write races,
TEST `$F0` clock controls, SPC bus scheduling and DSP subcycles remain unresolved
and outside this change. Whole-instruction scheduling must not be taken as
physical bus-access evidence. Guest observations will use separated windows.

## Implementation and validation

Production changes only ordinary prescaler advancement in `Apu::tick` and
rising-enable handling in `Apu::write($F1)`. Each scaler wraps continuously;
enable gates stage 2 at a pulse. Rising enable clears stage 2/output while
retaining scaler phase. Eight-bit live equality/wrap, read-clear/modulo output,
port clearing, DSP cadence and the firmware-free uploader are unchanged.

### Original public-API matrices

The five additions in [`spc_timers.rs`](../crates/snes-core/tests/spc_timers.rs)
retain all six #83 tests and every original assertion/guest record:

- Initially disabled pre-roll: all three timers, phases `D/4`, `D/2`, `3D/4`,
  targets 1/3/0, after three complete disabled divider wraps. This is 27
  scenarios, each checking the first and two subsequent event boundaries.
- Disable at `4D+D/4` with target 3: counting stage 1 and unread output 1.
  Re-enable at `9D+3D/4`, after 5.5 disabled divider periods, checking output
  retention while disabled, rising-enable clearing with and without a prior
  read, and boundaries `12D`, `15D`, `18D` on all timers. Retaining stage 1
  would fire early at `11D`; frozen-phase resume would first fire at
  `12D+D/2`; restart-on-enable would fire at `12D+3D/4`. Only free-running
  phase plus cleared counting matches `12D`.
- Repeated enabled CONTROL writes at `4D+D/2` retain unread output 1 and stage
  1, then match at `6D`, `9D`, `12D`. CONTROL input clearing is exercised with
  nonzero ports; CPU-facing output ports must retain their independent bytes.
- Each timer runs target 3 while both other timers are enabled at
  `4D+D/2+2`. Running count/output/phase and independently calculated new-timer
  pulse totals are checked. The extra two cycles avoid all scaler boundaries.
- The uploaded guest below supplies the fifth test, with numeric expectations
  required separately in whole and irregularly partitioned executions.

These fixtures use only public register/port/time APIs. No private timer state
is assigned; expected phases come from the documented divider periods and the
explicit initialization convention. The #83 live matrix, target equality/wrap,
zero, read continuation, modulo-16 and independence controls remain intact.

### Original firmware-free uploaded guest

`phase_guest` builds original SPC700 instructions in Rust, uploads them to $0200
through the existing firmware-free protocol, and records 20 timer observations
at $1100–$1113. No expected data are injected. Original authorship/license and
generated-test provenance are recorded in
[PROVENANCE.md](../tests/roms/PROVENANCE.md#issue-85-original-timer-phase-vectors-and-uploaded-spc700-guest).

The host starts at fixture cycle **8192**, a shared scaler boundary. The polling
loop is `MOV A,$F4` (3), `CMP A,#1` (2), taken `BNE` (4); exiting `BNE` takes
2. Allowing polling latency and whole-instruction overshoot gives a conservative
first post-handshake instruction origin `8192+s`, **0 <= s <= 18**. This bound
is derived from durations, not sampled timer output. All times below are
relative to that instruction origin. Each delay block is `MOV Y,#n` (2),
`n-1` taken `DBNZ Y` (6 each) and final `DBNZ Y` (4), exactly **6n**; remaining
cycles use NOP (2) or a harmless RAM read (3). No timer times its own delay.
Immediate direct-page writes take 5; reading and storing output takes 3+5.

| Relative cycle | Guest operation |
| ---: | --- |
| 0, 5, 10 | Set T0/T1/T2 targets to 1/3/8 |
| 52 | Enable all timers |
| 300 | Repeat enable; preserve accumulated output and counting |
| 330 | Set T0 target to 3, after pulse 256 and before pulse 384 |
| 860 | Disable T0/T2; keep T1 enabled |
| 1440 | Re-enable T0/T2, clearing their downstream stages only |

For arithmetic below, cycle numbers are relative to **8192**, not to enable.
T0's first pulses are 128 and 256. Its first read occurs at 152+s, after 128
and before a restarted clock could pulse at 180+s. The repeated write at 300+s
must preserve the unread event at 256. Changing target at 330+s produces the
next match at 640; the pulse at 768 leaves counting stage 1 and output 1 when
disabled at 860+s. Re-enable at 1440+s gives pulses 1472, 1600, **1728**, then
matches again at **2112**. Reading at 1780+s distinguishes the required 1728
match from the restarted clock's first event at 1824+s. T1 matches at every
multiple of 384 throughout all CONTROL operations.

For T2, enable occurs after 3 or 4 source pulses, so its first match is 176
or 192, then every 128 cycles. Both alternatives give the same required
records. After the read at 360+s, matches at 432/448, 560/576, 688/704 and
816/832 leave four events before disabling. After re-enable, the first match
is 1568/1584 and the next 1696/1712, then every 128. All output-read instruction
windows lie away from the corresponding output events for the full s range.
Slow CONTROL/target writes also fit between relevant scaler pulses; fast
controls tolerate either adjacent source pulse without asserting a physical
access edge. Physical coincident operations remain unresolved.

| Relative read cycle | Timer | Required output |
| ---: | --- | ---: |
| 152 | T0 | 1 |
| 160 | T1 | 0 |
| 208 | T2 | 1 |
| 340 | T0 | 1 |
| 350 | T1 | 0 |
| 360 | T2 | 1 |
| 1000 | T0 | 1 |
| 1010 | T1 | 2 |
| 1030 | T2 | 4 |
| 1460 | T0 | 0 |
| 1480 | T2 | 0 |
| 1650 | T2 | 1 |
| 1680 | T0 | 0 |
| 1690 | T1 | 2 |
| 1780 | T0 | 1 |
| 1790 | T1 | 0 |
| 1800 | T2 | 1 |
| 2160 | T0 | 1 |
| 2170 | T1 | 1 |
| 2180 | T2 | 3 |

Every upload acknowledgement has a 1000-by-21-master-clock bound; code must be
below 512 bytes and upload below 100000 master clocks. Ready must still be
observable at cycle 8192. Runtime is below 2214 SPC cycles from the host start;
port 0=$85 and STOP are required by **60000 master clocks** after start.
Every advance rejects APU errors, including SPC execution failures. The guest
never branches on a timer value. No firmware, commercial ROM, external
executable fixture or reference emulator is used as an oracle.

Whole/irregular ticks use checkpoints 10000/25000/40000/60000 master clocks after
start; independently required record-prefix lengths are 6/9/17/20. Only after
both runs require those numeric records do they compare every PCM sample, all
DSP registers, all RAM, input/output ports, PC and halted state. Final completion,
no errors, silence without keyed voices, FLG=$20 and master-left volume 37 are
required separately. Partition equality establishes determinism only. Existing
audible DSP regressions and the exact 32 kHz cadence assertion remain required.

### Preserved #83 uploaded observations

The old guest's enable-relative timing prose is corrected in
[its record](issue-83-validation.md#original-uploaded-guest-and-independent-windows)
and source comments. Its relative read times and all twelve literal output
records are unchanged. For arbitrary slow enable phase p, its T0 event windows
are 45441–45568 and 58241–58368, and T1's first event is 32513–32640. For arbitrary
fast phase q, T2's first event is 4081–4096. All old reads remain separated from
those event windows; the seven live-target scenarios, audible DSP and cadence
controls are preserved without weakening their distinctions.

### Executed regression evidence

Before production edits, the new API tests ran against the unchanged baseline:
7 passed / 3 failed (including all six old tests passing). The new guest then
compiled and failed numerically: actual complete records were
`[0,0,1,2,0,1,1,2,4,0,0,1,0,2,0,0,1,1,1,3]`, required
`[1,0,1,1,0,1,1,2,4,0,0,1,0,2,1,0,1,1,1,3]`.

After correction, **all 11 focused tests pass**. For reproducible confirmation,
an isolated source snapshot under `.cache/issue-85/experiments` compiled the
final tests against the exact `9bc710d` APU source: **7 passed / 4 failed**.
The pre-roll test got 0 instead of 1 at T0 cycle 512 (phase 32); the disabled-wrap
test got 0 instead of 1 at cycle 1536. The independence test got one newly
enabled T1 event instead of two; the guest failed its first six-record prefix.
Restricting only the test matrix timer selection to T1 and T2 independently
produced the same behavioral failures: pre-roll at 512/64 and re-enable at
1536/192, respectively. No production or harness behavior was changed in these
baseline snapshots.

A second isolated experiment removed only the rising-enable scaler reset from
that baseline, leaving disabled advancement frozen. The disabled-wrap test
still failed, producing 0 instead of 1 at cycle 1536. This demonstrates rejection
of frozen-phase resume as well as restart-on-enable. These are executed assertion
failures, not compilation failures or output used to choose expectations.
Local raw logs are in `.cache/issue-85`; durable evidence is summarized here.

### Mandatory checks

Host: macOS arm64, native Rust 1.94.0; rustup stable Rust 1.99.0 for the SPC
assembler and Wasm using the README `RUSTC` override; Node 24.12.0 and Python
3.13.13.

| Required command | Actual result |
| --- | --- |
| `cargo test --locked -p snes-core --test spc_timers` | Passed: 11 tests, zero failed/ignored |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed, including final record-prefix assertions |
| `cargo test --workspace --locked` | Passed: 435 tests, zero failed/ignored |
| `python3 tools/make-fixture.py .cache/smoke.bin` and `cmp tests/roms/smoke.bin .cache/smoke.bin` | Passed: byte-identical |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Passed: all three pinned assemblers built from source |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Passed: CPU full/basic and SPC, 600 frames each; terminal markers 0649/0452/0557 |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Passed: 11 grayscale bands, each 16 scanlines; final shade persists through line 223 |
| `python3 tools/build-cleared-games.py` twice | Passed: two fresh builds reproduce both expected hashes |
| `npm --prefix web ci` | Passed: zero audit vulnerabilities |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Passed |
| `npm --prefix web exec -- playwright install chromium` | Passed |
| `RUSTC="$(rustup which rustc)" SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'` | Passed: all 37 selected tests, zero failures/skips, 3.1 minutes |

The workspace retains all audible DSP regressions and the exact 32 kHz cadence
assertion. The browser selection retains Castle/Memory 18100-frame gameplay
replays, Castle's five-map/credits route and freeze rejection, Memory music-note
duration/finite-effect checks, gesture-activated Web Audio, and queue/drift
bounds. Only existing optional unaudited `301 seconds` tests are excluded by
the required filter; no required browser assertion was removed or relaxed.

Both fresh cleared-game builds reproduce Castle SHA-256
`197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`
and Memory SHA-256
`6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
Smoke remains
`6e5518b3959883cc22e914621fc5b5d9f5c0fe4eb96ee61000769afdbb68d6e5`.
Source/tool pins, cleared-game sources and browser regression assertions are
unchanged. The initial tool build's Codeberg download was terminated after
several minutes at roughly 10 KB/s; a bounded single-transfer retry timed out
(curl exit 28). The server supports byte ranges; bounded parallel transfers
assembled the same archive, which matched the unchanged SHA-256
`979b0fbe19830ffb49d2f187a5512044b7514914f1d289686c340ffd1d6a6e63`.
The documented build was then rerun successfully, including the pinned SPC
assembler with Rust 1.99. No source, pin or dependency change was needed.

All required gates ultimately passed; none remain failed, skipped or unperformed.
The intentional baseline/frozen-phase assertion failures and resolved download
timeout above are the only failed attempts. `git diff --cached --check` and
local Markdown target checks also passed. The final diff contains only the
ordinary timer correction, original tests, README and validation/provenance docs.

Source-built fixture hashes remain:

| Fixture | SHA-256 |
| --- | --- |
| CPU full | `1727e7c46ebb5df55711598264ba19005986bd34e915621383966adf674b3cfa` |
| CPU basic | `514d58950ae1ca940d6673c2a028b258b2ab17293bbc7cbd7cb54dded32abc5b` |
| SPC | `279d6a92d051ff8a98e190ce73e40482ff2cc981812509c7bca35a047484507e` |
| PPU/HDMA | `aef339e5d4a7319db31de577b253d977b45f5c45f09adfaa99a302ca300a957a` |

## Inventory and remaining work

All 24 scopes and their outstanding requirements in the
[Issue #83 inventory](issue-83-validation.md#inventory-and-remaining-work)
remain in force: `cpu`, `bus`, `scheduler`, `dma`, `hdma`, `controllers`,
`ppu-bg`, `ppu-offset`, `ppu-hires`, `ppu-mode7`, `ppu-objects`, `ppu-color`,
`ppu-ports`, `ppu-display`, `spc`, `ipl`, `dsp-voices`, `dsp-noise`, `dsp-pitch`,
`dsp-envelope`, `dsp-echo` and `dsp-timing` remain partial; `ppu-raster` and
`pal` remain unimplemented. No scope is promoted. Ordinary prescaler phase is
corrected here, but physical instruction/access timing, communication races,
SLEEP/STOP distinctions, register readback, reset retention, comprehensive SPC
instruction semantics and IPL instruction execution remain partial. Existing
BRR/filter/interpolation/envelope, live-header/directory/key timing, PMON/NON,
echo-RAM interaction and physical audio qualifications remain unchanged.
Selected timer tests do not establish complete hardware accuracy, commercial
compatibility or Issue #3 completion. No optional or post-merge check is required.
