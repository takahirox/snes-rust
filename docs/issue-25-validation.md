# Issue #25: forward WRAM/WMDATA conflict suppression

Validated on 2026-10-23 for [Issue #25](https://github.com/takahirox/snes-rust/issues/25).
The unchanged production baseline is `f7ec60a6e7645fab737b09f1002a3f16be3caf8a`
(the Issue's evaluation baseline is `af6b37d81a024a8d5c2d9255863321b751f5470e`).

## Specification prerequisite

Both references were fetched and inspected before implementation. No emulator
implementation supplied the expectations. Exact short extracts and their locations:

[Anomie's Register Doc, revision 1157](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt):

- **2180 WMDATA**, lines 748–751: “WRAM will not be written.” and
  “the address in $2181-3 is not incremented.” These explicitly support discarding
  the forward write and retaining the port address.
- **DMA AND HDMA → DMA**, steps 2–3, lines 2166–2172: “Adjust AAddress.” and
  “Decrement Count.” These follow the conflict note in step 1; a conflict does
  not terminate the algorithm. The bank stays constant; initial zero means 65,536.
- **DMA AND HDMA → HDMA**, step 2a, lines 2224–2229: “and increment.”;
  its RAM/$2180 note says the DMA restrictions “apply here as well.” Step 3,
  line 2230: “Decrement $43xA.” Thus payload and line accounting continue.
- **43x0 DMAPx**, lines 1067–1095, defines direction, direct versus indirect
  payload selection and mode offsets. **2181–2183**, lines 759–761, supplies
  the full-bank/mirror map. **43x1**, lines 1105–1110, specifies eight-bit BBAD
  wrapping. **43x5–43xA** distinguishes indirect payload, table and line state.

[Fullsnes](https://problemkaputt.de/fullsnes.htm), **SNES Memory Work RAM Access →
DMA Notes**: “WRAM-to-WRAM DMA isn't possible”. Its **Overall Memory Map / System
Area** and **Work RAM Access** confirm full banks `$7E/$7F` and first-8-KiB mirrors
in `$00–$3F/$80–$BF`. **DMA and HDMA Channel Registers → DMAPx / BBADx / A1Tx /
DASx / A2Ax / NTRLx / HDMA Table Formats** cross-checks mode selection, bank
selection, byte counts, incrementing HDMA payload/table addresses, repeats and
zero termination. This corroborates the conflict and progression; the explicit
no-port-increment statement above comes from Anomie.

Anomie's **2181–2183** also describes ineffective WRAM-sourced DMA writes to the
address registers. This bounded change implements only `$2180`, as requested;
no new hardware assertion is made about those other registers. Mixed-register
vectors use ordinary `$217F` (APU port 3), avoiding a contradictory `$2181` control.
Anomie mentions an initial reverse open-bus value, which does not establish a
stable reverse electrical value. Reverse conflicts remain unresolved here.

## Implementation and original vectors

The shared `SystemBus::transfer` path resolves the effective B address with its
existing wrapping mode offset, then suppresses only forward WRAM-to-`$2180`
writes. It still reads A and consumes both existing four-clock phases. DMA and
HDMA callers retain byte/address/count and row accounting. Descriptor and indirect
pointer fetches use their existing separate A-bus path.

The original MIT vectors in `machine/dma_tests.rs` use literal addresses, states
and expectations, without invoking the production conflict predicate. They cover
both complete WRAM banks, all four mirror-bank boundaries at `$0000/$1FFF`,
non-WRAM `$2000` controls (port side effects only), `$40/$C0` ROM controls,
ROM/SRAM payloads, legal reverse WMDATA-to-SRAM, CPU reads/writes with 17-bit wrap,
fixed/incrementing/decrementing sources, mirror entry/exit, bank-end wrapping,
mode-1 `$217F/$2180`, and eight-bit BBAD wrapping to `$2100`. The zero-count case
consumes 65,536 slots and retains 540,144 model clocks including refresh holds.

Complete direct/indirect HDMA tables exercise two repeated rows, a non-repeated
two-line entry, mixed legal/conflicting payloads and termination. Assertions check
table and indirect pointers, line counters, active masks, sentinels and clocks.
A WRAM table with ROM payload remains legal; a ROM table with WRAM payload
conflicts, independently testing payload-bank selection and legal table fetches.
The original CPU-executed guest in `tests/dma_access.rs` initializes source,
sentinels and DMA registers with instructions, checks conflict progression and
port position, then performs a legal ROM transfer and records numeric results in
WRAM. It has 10,000-instruction / two-frame deadlines; provenance is in
`tests/roms/PROVENANCE.md`.

Only the forward wrap vectors' formerly successful WRAM copies are corrected;
their ROM copies, final addresses/banks/counts and timing remain checked. Existing
A-bus exclusion, status acknowledgement, queued/deferred NMI, access-boundary and
HDMA-preemption regressions remain intact.

## Baseline failure evidence

With all new tests present and production `machine.rs` unchanged:

- `cargo test --locked -p snes-core --lib machine::dma_tests`: 21 passed,
  8 failed. Seven new conflict regressions and the corrected wrap regression
  failed; the exact one-byte reproduction produced `$5A/$11001` instead of
  `$A6/$11000`. Legal/CPU controls passed.
- `cargo test --locked -p snes-core --test dma_access`: 3 passed, 1 failed.
  The new guest's CPU result at `$1805` was `$37` instead of `$A6`, exposing
  the unwanted port increment; the three existing guests passed.

## Validation results

Commands were run from the assigned worktree. On this macOS host, Homebrew's
Rust 1.94 was selected by default; the pinned SPC assembler needs Rust 1.99.
Following README's compiler-selection guidance, these final gates used
`RUSTC="$(rustup which rustc)"` (Rust 1.99.0); workspace doctests also used
`RUSTDOC="$(rustup which rustdoc)"`. The first assembler attempt failed on the
older compiler's unsupported syntax, and the first workspace run failed only at
doctests because the old `rustdoc` tried loading 1.99 artifacts. Both were rerun
successfully with the matching compiler tools. No source pins were changed.

| Command | Actual result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Pass |
| `cargo test --workspace --locked` | Pass: 143 tests; no failed/ignored tests; doctests pass |
| `cargo test --locked -p snes-core --lib machine::dma_tests` | Pass: 29 tests |
| `cargo test --locked -p snes-core --test dma_access` | Pass: 4 original CPU guests |
| `python3 tools/make-fixture.py .cache/smoke.bin` | Pass |
| `cmp tests/roms/smoke.bin .cache/smoke.bin` | Identical: SHA-256 `73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb` |
| `python3 tools/build-test-tools.py --spc` | Pass: all three pinned assemblers built |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Pass: full CPU `$0649`, basic CPU `$0452`, SPC `$0557`, each 600 frames with expected terminal PC |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Pass: 11 grayscale bands and final shade through line 223 |
| `python3 tools/build-cleared-games.py` (twice) | Pass: both fresh builds reproduced both expected ROM hashes |
| `npm --prefix web ci` | Pass; zero reported vulnerabilities |
| `npm --prefix web run build` | Pass: production Wasm/static artifact built |
| `(cd web && npx playwright install chromium)` | Pass |
| `SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'` | Pass: all 18 tests, 2.5 minutes |

The initial browser command on port 8080 failed before tests because the port was
occupied; the documented alternate port 18085 was used without changing the suite.
Browser and hardware assertions, source pins, licensing and provenance decisions
were preserved. Reproduced cleared-game SHA-256 values:

- Castle: `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`.
- Memory: `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.

Browser evidence confirms both Castle and Memory ran 18,100 frames, each with
20 checked gameplay windows. Castle's full route completed at frame 11,097,
traversing all five maps and finishing credits at scroll 287. Memory won and
restarted 18 times; music, independent finite effects, gesture-activated audio,
queue/drift checks, the gameplay-freeze negative control, SRAM-failure controls
and the Pages subpath smoke test all passed. No frame count, route, deadline,
audio assertion or negative control was shortened or weakened.

Generated local evidence is in `.cache/hardware-tests/results.json`,
`.cache/ppu-test/results.json`, `.cache/cleared-games/` and `web/test-results/`;
these generated artifacts remain ignored.

## Remaining scope

DMA and HDMA remain partial. The four-clock phases, aggregate 16-clock DMA setup,
HDMA fetch placement and refresh timing are retained emulator conventions, not
new evidence of hardware startup/alignment, initialization, overhead or first-frame
accuracy. Reverse conflicts, electrical/open-bus values, same-channel contention,
unrelated I/O restrictions and the complete mode-interaction matrix remain work.
No commercial ROM or firmware was acquired.

All 24 inventory states remain unchanged: `cpu`, `bus`, `scheduler`, `dma`, `hdma`,
`controllers`, `ppu-bg`, `ppu-offset`, `ppu-mode7`, `ppu-objects`, `ppu-color`,
`ppu-ports`, `ppu-display`, `spc`, `ipl`, `dsp-noise`, `dsp-pitch`, `dsp-envelope`,
`dsp-echo`, `dsp-timing` are partial; `dsp-voices` is implemented_unverified;
`ppu-hires`, `ppu-raster`, `pal` are unimplemented. Limited passes establish neither
complete hardware accuracy nor commercial compatibility. There are no optional
checks or required post-merge checks for this Issue.
