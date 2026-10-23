# Issue #37: live CPU writes to HDMA NTRL

Implemented for [Issue #37](https://github.com/takahirox/snes-rust/issues/37),
starting at checkpoint `1eefd03a7c6c52e53780fc2398d524e910a8ec88` with production
baseline `e92e25707f7b892772f628c21711e2f80fd11b98`. All new tests, instructions
and data are original MIT work. No emulator implementation was consulted.

## Reference evidence and live-write derivation

Retrieved both references on 2026-10-23. Ignored raw copies and command logs are
under `.cache/issue-37/`; the references are not redistributed. Hashes match the
Issue #33 reference versions:

| Reference | Version | SHA-256 |
| --- | --- | --- |
| [Anomie's Register Doc](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt) | Revision 1157, 2007-07-12 | `19dfdf3f1c206655b8ede11bcd6dc053e36634f876819e40bb39cfb88a237dfd` |
| [Fullsnes](https://problemkaputt.de/fullsnes.htm#snesdmaandhdmachannel07registers) | Extracted from no$sns v1.5 | `3853b65a7331bf12eea431ff494a2948a1c78ef22f7e819db4b139fd5a79f6c6` |

Anomie **43xa / NLTRx** (raw lines 1322–1348) explicitly describes writable
repeat and line-count bits, mid-transfer writes changing both for the next
scanline, and this caveat: “Remember though that 'repeat' won't take effect
until after the next transfer period.” It also explicitly describes decrement
before testing repeat/count, `$80` as 128 nonrepeating lines and `$00` as 128
repeating lines when it does not denote termination.

Anomie **DMA AND HDMA → HDMA** (2422–2513), particularly the scanline steps
(2475–2498), distinguishes an internal DoTransfer flag from NTRL. Payload
steps 1–2 consult that flag first; step 3 says “Decrement $43xA.” Step 4 derives
the next DoTransfer from repeat. Step 5 reloads metadata on low-seven-bit
exhaustion, handles zero-descriptor termination, and prepares the next transfer.
Initialization separately fetches metadata and prepares DoTransfer (2451–2463).
The source's nearby uncertainty concerns arbitrary mid-frame enabling; this
change does not claim to resolve that question.

Fullsnes **DMA and HDMA Channel 0..7 Registers → NTRLx** (raw HTML 809–819)
corroborates an R/W register with bit 7 repeat and bits 6–0 count, decremented
per scanline. Its **HDMA Table Formats** (836–858) describes fetched zero
termination, `$01..$80` one-transfer rows and `$81..$FF` repeat rows. Fullsnes
alone does not explicitly specify the live-write transfer-flag latency; that
rule comes from Anomie's register text and separate algorithm state.

Derived consequences for an already naturally initialized, enabled,
unterminated channel, with CPU writes between events:

1. Replace **all eight NTRL bits**. Preserve the already prepared transfer/hold
   decision. A register write is not the metadata-fetch operation that sets
   DoTransfer to true. The register caveat and scanline step order jointly
   support this distinction; the implementation is not the oracle.
2. At the next eligible event, run or skip the payload using the old decision.
   Decrement the written byte modulo 256, even on a hold, then derive repeat
   and low-seven-bit exhaustion from that decremented byte.
3. A live `$00` becomes `$FF` on that event. It neither terminates nor reloads
   immediately. It exhausts after 128 events, with the initial transfer/hold
   preserved and transfers on the following 127 events. A live `$80` becomes
   `$7F` and holds on the following 127 events; its first event may itself be
   a hold. These are arithmetic/ordering deductions from the cited rules,
   not independent physical-device observations.
4. A live `$01` or `$81` exhausts after one event and fetches the next row.
   Only a **fetched** zero terminates internally. Programmed `$420C` stays set.
   CPU NTRL writes alone do not schedule an inactive channel or undo termination.
   Initialization still replaces earlier NTRL writes with fetched metadata.

This corrects unsupported extrapolations from older descriptor tests: a CPU
write does **not** guarantee a first payload, `$80` does not force one transfer
when already holding, and a live zero does not mean a fetched terminator.
The existing initialization/row-end vectors only exercised fetched descriptors;
their first-transfer expectations remain valid in that scope. The Issue #33
record now explicitly states that scope; no established descriptor expectations
were weakened to make live-write tests pass.

## Implementation and original scheduler regressions

The CPU register-write decoder synchronizes `hdma_line[channel]` with the byte
already published in `dma[channel][10]` for NTRL only. It does not alter the
prepared transfer flag, active/pending masks, HDMAEN, other channels or any
source registers. Existing initialization and row reload still replace both
counter copies using the shared metadata path. Keeping this change local also
preserves existing payload/reload phase ordering and A-bus access behavior.

Four tests in `machine/hdma_ntrl_tests.rs` configure tables, registers and live
writes exclusively through ordinary CPU bus reads/writes. Clock advancement
uses natural initialization and scanline events; no private line, transfer,
active or pending state is injected. Internal mask/HDMAEN observations are
limited to assertions because HDMAEN is write-only to the CPU.

- A focused `$82 → $81`, CPU-write `$02` regression checks next count `$01`,
  payload `$22`, no premature reload, a subsequent hold/reload to `$83`, payloads
  `$33/$44/$55` and final zero termination. Direct and indirect variants assert
  numeric table/payload pointers and preserve A1T/A1B/DASB.
- A 32-case matrix (two addressing modes, transferring/holding states, eight
  written bytes) checks immediate readback, the next two events, repeat-bit
  changes, nonzero row reload, payload bytes, counters and pointers.
- A second 32-case matrix runs **every** boundary to exhaustion and fetched-zero
  termination. Zero-valued payloads plus CPU-written `$EE` destination sentinels
  distinguish actual transfers from holds. It checks each counter, final table
  and indirect pointer, preserved programmed enable and no post-termination
  payload. Live `$00` and `$80` each run through all 128 events in both states.
- Isolation checks keep an initialized sibling transferring `$77/$88` with
  counter `$82/$81` and table `$0602/$0603` while changing channel 0. An inactive
  channel's NTRL is writable without scheduling work. A naturally terminated
  channel accepts another counter write but performs no payload, decrement or
  pointer change. Neither control attempts undocumented mid-frame reactivation.

The first-event counter/transfer derivation is:

| CPU byte | Counter after next event (next descriptor is `$83`) | Decision for following event |
| --- | --- | --- |
| `$00` | `$FF` | Transfer |
| `$01` | `$83` (reload) | Transfer |
| `$02` | `$01` | Hold |
| `$7F` | `$7E` | Hold |
| `$80` | `$7F` | Hold |
| `$81` | `$83` (reload) | Transfer |
| `$82` | `$81` | Transfer |
| `$FF` | `$FE` | Transfer |

In every row, the **next** event itself retains the pre-write decision. For
zero-filled termination vectors the number of events is respectively
128, 1, 2, 127, 128, 1, 2 and 127. Total payloads equal the preserved initial
transfer (zero or one) plus remaining repeating events. Direct table progression
starts at `$0102` after the initial payload; it advances per payload and once
for the terminating descriptor. Indirect tables stay at `$0103` until termination,
then reach `$0105` with pointer `$0000` through the retained last-channel rule.

## Baseline failure and CPU-executed guests

Before changing production code, ran:

```sh
cargo test --locked -p snes-core --lib live_02_uses_written_counter_instead_of_stale_81
```

It failed on baseline production: next NTRL was decimal **131 (`$83`)**, expected
**1 (`$01`)**. Immediate CPU readback of `$02` had passed. The stale `$81`
private counter decremented to `$80` and reloaded the next descriptor. The direct
variant failed first, before reaching the indirect iteration. With the fix,
the same focused test passes in both modes, and all four new scheduler tests pass.

Two guests in `tests/hdma_ntrl.rs` execute original 65C816 instructions to set up
HDMA during VBlank, poll `$4212`, write NTRL, record readback, inspect counters,
table/indirect addresses and CGRAM, and stop. Host code only supplies the
cartridge and observes results. Both are bounded by 100,000 instructions and
fewer than three frame wraps; completion requires a WRAM marker and STP.
[Provenance](../tests/roms/PROVENANCE.md#original-hdma-live-ntrl-cpu-guests-approved)
records original MIT instruction/data ownership. No commercial ROM or firmware
was acquired.

The guests' eight observations are independently derived below. Between events
0/1 write `$02`; 1/2 write `$82`; 3/4 write `$80`; 4/5 write `$00`; 5/6 write
`$01`; after fetched termination write `$82` and sentinel color `$1777`.

| Event | NTRL | CGRAM word | Direct table | Indirect table | Indirect payload pointer |
| --- | --- | --- | --- | --- | --- |
| 0 | `$81` | `$1211` | `$1003` | `$1003` | `$1202` |
| 1 | `$01` | `$2322` | `$1005` | `$1003` | `$1204` |
| 2 (held) | `$81` | `$2322` | `$1005` | `$1003` | `$1204` |
| 3 (reload) | `$83` | `$3433` | `$1008` | `$1006` | `$1300` |
| 4 | `$7F` | `$1555` | `$100A` | `$1006` | `$1302` |
| 5 (held) | `$FF` | `$1555` | `$100A` | `$1006` | `$1302` |
| 6 (terminate) | `$00` | `$1666` | `$100D` | `$1008` | `$5600` |
| 7 (terminated) | `$82` | `$1777` | `$100D` | `$1008` | `$5600` |

Immediate written readbacks are `$02/$82/$80/$00/$01/$82`; pre-initialization
readback is `$FF`, replaced by fetched `$82` before event 0. Direct DAS stays
`$5AA6`; both source A1T/A1B values stay `$001000`. Numeric host assertions
check all these bytes, not only a completion flag.

## Validation commands and results

Final mandatory gate results are recorded below. Rust workspace commands use
Homebrew Rust 1.94.0. Wasm and the auxiliary SPC assembler select rustup Rust
1.99.0 with README's `RUSTC="$(rustup which rustc)"` convention. Generated logs,
reference copies, fixtures and browser evidence remain ignored.

| Command | Actual result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Pass |
| `cargo test --workspace --locked` | Pass: 201 tests, zero failed/ignored; doctests pass |
| `cargo test --locked -p snes-core --lib hdma_ntrl_tests` | Pass: four new tests, including both 32-case matrices |
| `cargo test --locked -p snes-core --test hdma_ntrl` | Pass: both original CPU guests |
| `python3 tools/make-fixture.py .cache/smoke.bin` | Pass |
| `cmp tests/roms/smoke.bin .cache/smoke.bin` | Pass: identical, SHA-256 `73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb` |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Pass: all three pinned assemblers built |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Pass: full CPU `$0649`, basic CPU `$0452`, SPC `$0557`; expected terminal PCs, 600 frames each |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Pass: eleven grayscale bands, final shade through line 223 |
| `python3 tools/build-cleared-games.py` (twice) | Pass: both fresh builds reproduce both expected hashes |
| `npm --prefix web ci` | Pass: zero reported vulnerabilities |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Pass |
| `(cd web && npx playwright install chromium)` | Pass |
| `SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'` | Pass: all 18 checks, 2.5 minutes |
| `git diff --check` and staged whitespace check | Pass |

The workspace run preserves initialization, row-end ordering, terminating
indirect-read, HDMA-phase and DMA-access regressions, including all existing CPU
guests. No test was removed or disabled. During development one new inactive
control initially expected an uninitialized table pointer of zero; the actual
power-on value is `$FFFF`. That test setup expectation was corrected before the
final passing gates. It was unrelated to the baseline stale-counter failure.

Both game builds reproduce Castle
`197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`
and Memory
`6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
The browser suite retains both 18,100-frame gameplay checks, Castle's all-map
route/credits replay, Memory win/restart, independent music/finite effects,
gesture-activated Web Audio, stalls/repeat playback, pause/reset, save recovery
and Pages subpath loading. Existing replay inputs, fixture pins, licensed-source
assertions, notices and inventory statuses were unchanged. The alternate port
was selected using the README's documented `SNES_TEST_PORT` override.

## Review follow-up: CI Clippy gate

Reproduced the review finding with CI's Rust 1.99.0 (`b940084d7`) and Clippy
0.1.99: the mandatory workspace Clippy command failed with
`clippy::chunks_exact_to_as_chunks` in `hdma_phase_tests.rs`. The earlier
Homebrew Rust 1.94 result above did not exercise that lint. Replaced the
four-byte chunk assertion with direct equality of all 1,024 rendered bytes
against 255. Because the checked length is divisible by four, this preserves
the original pixel coverage and expected values.

Selected the installed CI toolchain explicitly; `rustup run 1.99.0 cargo clippy`
alone still selected Homebrew Clippy 0.1.94 in this environment:

```sh
export PATH="$(dirname "$(rustup which --toolchain 1.99.0 cargo)"):$PATH"
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
```

All three commands pass with Rust 1.99.0. The workspace run passes all 201 tests,
including the changed direct/indirect rendering regression, and doctests.
Hardware fixtures and browser checks were not rerun for this assertion-only
fix; their earlier results remain recorded above.

## Retained uncertainty and scope

The reference-backed rules and arithmetic above establish the tested live-write
behavior. They do not establish physical-device sampling during an in-progress
HDMA event, exact startup/alignment/full overhead, arbitrary mid-frame enabling,
same-channel DMA contention, electrical open-bus values or the complete transfer
mode/direction interaction matrix. The tests retain Issue #35's separate HBlank
and HDMA service phases (1096 and 1112 master clocks), initialization at clock 24,
and bus-boundary service conventions. Broad guest polling windows and scheduler
deadlines test this model, not exact hardware first-byte alignment.

HDMA remains partial, as does general DMA. All 24 inventory categories and their
existing statuses remain intact. This work does not complete Issue #3 or claim
complete hardware accuracy or commercial compatibility. No optional validation
or post-merge verification is required by Issue #37. Nothing was pushed, opened
as a PR, merged or posted to GitHub.
