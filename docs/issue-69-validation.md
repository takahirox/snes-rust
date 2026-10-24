# Issue #69: clocked automatic two-pad polling

## Reference record and event contract (read before implementation, 2026-10-24)

[Fullsnes](https://problemkaputt.de/fullsnes.htm), **SNES Controllers I/O Ports —
Automatic Reading**, JOY1–4 mapping/button table and AUTO JOYPAD READ, states
“ends 4224 master cycles later” and “Specifically, it begins at H=74.5 on the first
frame”. Subsequent starts are separated by multiples of 256 master clocks,
within H=32.5..95.5 on the first VBlank line. Its Manual Reading section assigns
one shared OUT0 and independent read clocks. A manually held OUT0 prevents
serial advancement. Results during busy are unreliable; released pads return zero.

[Anomie's Register Doc revision 1157](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt),
**4016/4017** (read/sample before advancing, shared latch, routing), **4200
NMITIMEN** (enable and approximate V=$E3 update), **4212 HVBJOY** (busy during
read, approximately three lines), **4218–421F JOY1–4** (updates during busy),
**CONTROLLERS / Generic / Open Port / Joypads**, says automatic reading will
“effectively write 1 then 0” and “then read 16 times from both $4016 and $4017”.
The approximate V=$E3 statement does not specify an atomic publication instant;
it cannot override the explicit during-busy update statement or Fullsnes's
more precise duration. The `$42c/d` spelling in 4016/17 is a typo: both register
maps assign port 1 Data2 to $421C/D. Standard pad Data2 is unconnected, reading
zero; B,Y,Select,Start,Up,Down,Left,Right,A,X,L,R,0,0,0,0 are followed by ones.

Neither required reference establishes exact latch width, individual sample
edges, clear timing, enable sampling, or electrical shift propagation. As
supplemental primary research, [Ilari's 2013-05-15 report](https://tasvideos.org/Forum/Topics/14077)
describes video observations of intermediate shifts, but explicitly labels its
fine timing proposal a guess. Its proposed 256-clock latch and 128-clock
release-to-first-sample gap motivate the deterministic convention below; they
are **not measured hardware requirements**. No test ROM/video from that thread
was downloaded. No physical trace was available. This Issue implements the
logical shared-state protocol; the subcycle phases remain unverified.

All times below are master clocks, T = automatic busy start. S = source
statement, D = deduction, C = emulator convention where evidence is incomplete.
The ordinary raster here is 262 lines × 1364 clocks, VBlank at V=225/H=0.

| Event | Time / operation | Evidence / qualification |
| --- | --- | --- |
| VBlank begins | V=225/H=0; no immediate busy | S: Fullsnes starts later; existing VBlank status is scheduler policy |
| Start opportunity | First frame H=298 clocks (74.5 dots); subsequent first 256-clock lattice point at/after H=130, through H=382 | S: Fullsnes range/first frame/lattice; D: unique lattice point in that interval using this fixed raster; reset-phase alignment is C |
| Enable sampling | Inspect $4200.0 once at opportunity; if zero, no latch, result clear, or busy | S: enabled cycle only; exact sampling point is C |
| Start/latch assertion | T: busy=1, automatic latch=1, clear all four result words | C: clear and latch coincide with busy; old results retained until T |
| Latch release | T+256: automatic latch=0, capture both live inputs when effective shared latch falls | C: width; D: shared parallel-load protocol; manual OUT0 is ORed with automatic latch |
| Sample/clock n=1..16 | T+128+256n, i.e. +384,+640,+896,+1152,+1408,+1664,+1920,+2176,+2432,+2688,+2944,+3200,+3456,+3712,+3968,+4224 | S: sixteen reads, independently clocked ports; C: placement, logical sample and returning clock combined at one event |
| Result update per sample | JOY1/2 = (old << 1) OR corresponding Data1; JOY3/4 shift in zero | S: routing and during-busy updates; D: final B in bit 15; C: exact intermediate shift visibility, no torn electrical bits |
| Completion | T+4224: sixteenth sample is visible, busy=0; no separate atomic publication | S: duration/busy completion; C: final sample coincides with completion |
| Serial position afterward | Sixteen reads consumed on each port, unless held/relatched/manually clocked; standard pads next return one | D: same serial state and protocol |
| Writes before start | $4200.0 write completed before opportunity selects that cycle, including one clock before in raw-boundary tests | C: precise sampling; enabled throughout remains supported contract |
| Writes after completion | Disable retains results and skips next opportunity; re-enable before next opportunity allows next cycle; no immediate restart | D: once-per-frame enabled polling; exact write/edge ordering is C |

Input changes during latch assertion remain live until release; changes after
release cannot replace captured bits. Manual reads consume only that port's
next bit. A noncoincident manual high/low pulse after release relatches both.
Manual OUT0 high across automatic release holds both at live B until software
lowers it. These are logical protocol deductions; Anomie explicitly leaves
physical during-poll access effects unknown. Automatic operations do not read
CPU open bus or execute CPU bus accesses.

Boundary policy: `tick(n)` services all events through its endpoint in time
order, even across a whole cycle. A following raw access sees the new phase.
Timed CPU accesses retain their existing bus sampling convention. This does
not establish arbitrary coincident CPU/device edge priority or setup/hold.
Mid-cycle $4200 cancellation/restart is excluded: the deterministic model finishes
an already-started cycle, but this is not an asserted hardware contract.

This corrects the old immediate VBlank busy/snapshot/atomic-publication
convention, including the NMI test and DMA guest's VBlank-then-busy-clear wait.
The guest must observe busy becoming set before waiting for its clear.

## Scope and remaining limits

Both standard pads use shared automatic/manual latch and serial state; no new
input API or frontend multiplayer is added. PAL, overscan/interlace, multitaps,
other peripherals, electrical pulse/propagation behavior, physical reset
retention, coincident edges and mid-cycle enable semantics remain outside scope.
Reset retains the existing emulator policy: clear volatile state and both inputs,
preserve SRAM. Controllers and scheduler remain partial. All 24 inventory
categories keep their prior status: 21 partial, dsp-voices implemented_unverified,
ppu-raster and pal unimplemented, none verified. Passing limited gates establishes
neither complete hardware accuracy nor commercial compatibility.

## Implementation and validation results

`SystemBus::tick` now stops at each automatic event, including when one call
crosses the complete interval; it continues through the remaining clocks.
An explicit pending-start opportunity and active phase replace `auto_until`
and the first-pad `auto_buttons` snapshot. Busy comes from the active phase.
Automatic reads use the same `joypad_latch` / `joypad_read` operations as manual
accesses; both port positions are consumed, and all four result words shift
in place. Device events advance in `bus_tick` during DMA, without CPU execution.

The 12 original unit tests in
[`joypad_auto_tests.rs`](../crates/snes-core/src/machine/joypad_auto_tests.rs)
cover every button on either pad, complementary/swapped/released/fully pressed
inputs, ignored low bits, all eight result bytes and four representative system
banks. Literal partial words check before/at/after every start, release,
sample and completion event, including clearing previously nonzero results.
Input changes during latch assertion/after release, unequal manual clocks,
manual relatch, manual OUT0 held high and post-completion positions are checked.
Other cases cover consecutive frames, the start lattice's frame-phase wrap,
disabled/just-before-start enables/disables and post-completion transitions,
reset during latch/sampling with SRAM retained, fine/irregular/whole-cycle tick
partitions, and natural DMA progression with the CPU PC unchanged.

Two original CPU guests in
[`joypad_auto.rs`](../crates/snes-core/tests/joypad_auto.rs) use ordinary CPU
scheduling; the second observes two cycles and checks swapped inputs.
The host supplies only ROM/buttons and observes results, with instruction/frame
deadlines. Build and license details are in
[PROVENANCE](../tests/roms/PROVENANCE.md#issue-69-original-automatic-two-pad-guests-and-polling-corrections).

### Baseline failure and corrected historical fixtures

On the unchanged source at the selected checkpoint (whose emulation files match
`80bb25417a08e871d42fef3fb699d23797c8a364`), running the new
`automatic_second_pad_counterexample` failed with bytes
`[90,A5,00,00,00,00,00,00]` instead of `[90,A5,60,5A,00,00,00,00]`.
This was additionally reproduced against an exact `git archive` of that baseline,
copying only the new integration test into the archived tree and running:

```sh
cargo test --locked --manifest-path .cache/issue-69/baseline/Cargo.toml \
  -p snes-core --test joypad_auto automatic_second_pad_counterexample
```

That run failed (0 passed, 1 failed) with the same missing second-pad bytes.
The final implementation passes the same counterexample and both CPU guests.

The existing NMI test's immediate busy assertion is replaced with a no-busy
assertion at VBlank and busy after 298 clocks; its NMI enable/read-clear and
completion assertions remain. DMA status fixtures now describe active scheduled
phases instead of assigning `auto_until`; their status/result and exclusion
assertions remain.

The first workspace run exposed the unsafe VBlank-then-busy-clear wait in the
pseudo-hires guest; both hires/pseudo-hires interactive builders were corrected.
The following run exposed the same assumption in the smoke diagnostic NMI
handler. Its source now waits for busy assertion and completion. The original
smoke source reproduced the checked-in original bytes before correction:
SHA-256 `73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb`.
The corrected Python-built source and committed binary reproduce byte for byte:
SHA-256 `6e5518b3959883cc22e914621fc5b5d9f5c0fe4eb96ee61000769afdbb68d6e5`.
The diagnostic's input, deterministic video, DMA, audio and frame-count assertions
are unchanged. This is an evidence-backed fixture correction, not a weakened
input expectation. Four HDMA fixtures had implicitly used adjacent smoke program
bytes as terminating indirect-pointer high bytes; those bytes ($A9/$22) are now
explicit test data, preserving their original numeric assertions. Static
hires/pseudo-hires outputs are unchanged; all nine fixture outputs rebuilt twice
and compared byte for byte. Current interactive hashes are recorded in provenance.

### Executed mandatory checks (2026-10-24)

All final checks below passed. Earlier failures and their corrections are
recorded above; there are no unresolved mandatory failures or unperformed
mandatory checks. Native Cargo checks used Homebrew Rust 1.94.0. The tool-build
and web-build commands selected installed rustup Rust 1.99.0 as documented in
README. The initial web build with Homebrew rustc failed because that compiler
lacks the Wasm target; selecting rustup succeeded without changing dependencies.

| Check | Final result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Pass |
| `cargo test --workspace --locked` | 372 passed, 0 failed, 0 ignored |
| `cargo test --locked -p snes-core --lib machine::joypad_auto_tests` | 12 passed |
| `cargo test --locked -p snes-core --test joypad_auto` | 2 passed |
| `cargo test --locked -p snes-core --test joypad_serial` | 8 passed |
| `cargo test --locked -p snes-core --lib machine::dma_tests` | 48 passed, including prohibited DMA/HDMA controller reads |
| `cargo test --locked -p snes-core --test dma_access` | 6 passed |
| `python3 tools/make-fixture.py .cache/smoke.bin` and `cmp tests/roms/smoke.bin .cache/smoke.bin` | Pass, both original before correction and rebuilt corrected fixture |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Pinned ca65/ld65, bass-untech and spcasm built |
| `tools/hardware-tests.py` with README absolute ca65/ld65/spcasm paths | CPU full/basic and SPC suites passed |
| `tools/ppu-test.py` with README absolute bass path | 11 grayscale bands × 16 lines, final shade through line 223 passed |
| `python3 tools/build-cleared-games.py` twice | Both builds passed with unchanged Castle/Memory hashes |
| `npm --prefix web ci` | Pass; 4 packages, 0 vulnerabilities |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Pass |
| `(cd web && npx playwright install chromium)` | Pass |
| `SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'` | 37 passed, 2.8 minutes |

Hardware gates used the unchanged source/tool pins and asset replacements in
[PROVENANCE](../tests/roms/PROVENANCE.md). Numeric terminal evidence:

| Suite | Last test / PC | Built ROM SHA-256 |
| --- | --- | --- |
| CPU full | $0649 / $0081A2 | `1727e7c46ebb5df55711598264ba19005986bd34e915621383966adf674b3cfa` |
| CPU basic | $0452 / $0081A2 | `514d58950ae1ca940d6673c2a028b258b2ab17293bbc7cbd7cb54dded32abc5b` |
| SPC | $0557 / $008060 | `279d6a92d051ff8a98e190ce73e40482ff2cc981812509c7bca35a047484507e` |
| HDMA bands | 11 bands / line 223 | `aef339e5d4a7319db31de577b253d977b45f5c45f09adfaa99a302ca300a957a` |

Mandatory browser checks retain both five-minute cleared Castle/Memory replays,
Castle's all-map/credits route, gameplay/audio assertions and the mandatory
cleared Castle frozen-player negative control. Both optional original-release
frozen-player controls remain unchanged and are excluded by the documented
`301 seconds` filter along with those optional replays. The 37 passing checks
also retain actual Wasm/canvas, input, reset, SRAM, error and Pages artifact checks. Cleared-game SHA-256 values from both builds:
Castle `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`,
Memory `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
No game source, replay, hash, assertion or negative control was relaxed.
Local logs are under `.cache/issue-69`; hardware JSON is under
`.cache/hardware-tests` / `.cache/ppu-test`, and browser attachments are under
`web/test-results` (generated/ignored artifacts; this committed record retains
the relevant results).

Optional physical traces and incompletely audited user-supplied commercial or
original-release games were not run or acquired. No new ROM acquisition was
needed beyond the repository's approved hash-pinned source builds. No post-merge
verification is required. The electrical/fine-phase and excluded scope limitations
above remain despite passing all these checks.
