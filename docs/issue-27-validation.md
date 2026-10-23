# Issue #27: WRAM-sourced address-register writes

Validated on 2026-10-23 for [Issue #27](https://github.com/takahirox/snes-rust/issues/27).
The assigned checkpoint was `4d0c9ab7070ad5a92d6df4eab16de807ecaf7f12`;
its production code matches the evaluation baseline
`396d4f75eaa47452f819b161b1eb4ae830c142b4`.

## Specification and HDMA applicability

Both linked references were fetched and inspected before changing production code.
No emulator implementation supplied expectations.

[Anomie's Register Doc, revision 1157](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt):

- **2181–2183 WMADDL/WMADDM/WMADDH**, lines 754–767:
  “DMA from WRAM to these registers has no effect.” Non-WRAM DMA remains legal.
- **43x0 DMAPx**, lines 1067–1095: direction, direct/indirect payload selection,
  eight mode patterns. **43x1 BBADx**, lines 1105–1110: eight-bit port wrapping.
- **DMA AND HDMA → DMA**, lines 2154–2172: the simultaneous-bus restriction
  names all four WRAM ports; subsequent steps “Adjust AAddress.” and
  “Decrement Count.” retain bank and count progression.
- **DMA AND HDMA → HDMA**, lines 2182–2196 and 2224–2244: shared port/modes,
  separate descriptor/payload processing; payload reads “and increment.” precede
  port writes and “Decrement $43xA.” The conflict notes “apply here as well.”

HDMA address-register suppression is a derivation: the register-specific
no-effect rule above concerns simultaneous WRAM A-source/B-port accesses;
HDMA uses the same direction, port and byte modes. Its payload operations
therefore have the same conflict, regardless of direct/indirect addressing.
The HDMA note explicitly mentions `$2180`, not separately `$2181–$2183`;
the general WRAM-to-WRAM prohibition alone would not establish this extension.
Descriptor/pointer fetches have no simultaneous address-register B write.

[Fullsnes](https://problemkaputt.de/fullsnes.htm), **SNES Memory Work RAM Access →
2181h–2183h**, confirms the low/middle/high-bit fields of the 17-bit pointer;
legal writes replace only their addressed bits, with data bit 0 selecting bit 16.
Its **DMA Notes** corroborates the WRAM bus conflict, without independently
specifying address-register suppression. **Overall Memory Map / System Area**
and **Work RAM Access** establish full WRAM banks `$7E/$7F`, first-8-KiB system-bank
mirrors and non-WRAM `$2000`. **DMA and HDMA Channel 0..7 Registers → DMAPx,
BBADx, A1Tx, DASx, A2Ax, NTRLx, HDMA Table Formats** corroborates shared port
selection, distinct indirect payload bank/pointer, table progression, repeats and
zero termination. These sources do not establish this emulator's retained timing.

## Implementation and independent vectors

`SystemBus::transfer` extends the existing forward restriction from `$2180` to
`$2180–$2183`, after resolving the wrapping effective B address. The current
A-bus payload address is classified for each byte. Source reads and both
four-clock phases remain; DMA/HDMA callers still progress normally. CPU handlers,
reverse transfers, descriptor/pointer reads and the Issue #23 status-read contract
are unchanged.

Original MIT tests in `machine/dma_tests.rs` add:

- Separate one-byte low/middle/high-clear/high-set reproductions at pointer
  `$12345` (high-set starts at `$02345`), with source-read observations, complete
  pointer, whole-WRAM/sentinel preservation, final A-address/bank/count, pending
  mask and 24 retained model clocks. Payloads `$5A/$FE/$FF` would change the
  selected bits. All full-bank/mirror/fixed-source cases consume three slots.
- Literal full-bank endpoints and `$2000` within both WRAM banks; `$0000/$1FFF`
  in mirror banks `$00/$3F/$80/$BF`; non-WRAM `$2000` routing controls; ROM in
  `$40/$C0`, mapped `$00/$80` ROM and `$70` SRAM; CPU partial-field controls.
  Unmapped reads use observed fixture data to compare routing against ordinary
  writes, without specifying an electrical value.
- Increment/decrement mirror entry/exit, 16-bit wrap without bank carry,
  all eight B patterns with legal ROM controls, mode 4 at `$2180–$2183`, mixed
  `$217F–$2182`, and BBAD wrap from `$21FF` to `$2100–$2102`. Literal mode-3/7
  controls account for the legal WMDATA carry from `$123FE` to `$12400`.
- Complete direct/indirect tables for each address register: two repeated rows,
  a two-line non-repeated interval, then zero termination. Every row checks
  all pointer bits, unchanged WRAM, table/payload pointers, line/transfer state,
  active/enabled masks, A-bank/start and model clocks. WRAM-table/ROM-payload,
  ROM-table/WRAM-payload and legal direct ROM controls distinguish payloads from
  descriptors and indirect-pointer bytes. High-bit conflicts start with both
  bit-16 values; legal high-bit rows clear/set/clear.

The original CPU guest in `tests/dma_access.rs` initializes sources and distinct
correct/redirected-address sentinels with absolute/long instructions. Four DMA
cases cover all three registers and both high-bit directions. Two `$2180` reads
observe preserved pointer position; CPU-written WRAM records source, A-address,
bank, count and both redirected sentinels. Separate legal CPU and ROM controls
select four numeric sentinels. A success marker and STP must occur within 10,000
instructions and fewer than two frames. Host code supplies only the cartridge,
steps the machine and asserts independent numeric results. Its code/data/build
provenance is [MIT](../tests/roms/PROVENANCE.md); no new binary is committed.

Existing `$2180`, zero-count/65,536-slot, A-bus exclusions, status acknowledgement,
queued/deferred NMI, access-boundary, refresh and HDMA-preemption checks remain.
No fixture pin, game assertion, deadline, licensing decision or inventory status
was relaxed.

## Baseline and final focused evidence

Before changing `machine.rs`, the new tests produced:

- `cargo test --locked -p snes-core --lib machine::dma_tests`: **30 passed,
  8 failed**. Low/middle/high-clear reproductions actually produced `$1235A`,
  `$15A45`, `$02345`; high-set produced `$12345` instead of `$02345`. Fixed-source,
  progression, effective-port and HDMA-table tests also failed. Legal controls
  and all 29 existing unit tests passed.
- `cargo test --locked -p snes-core --test dma_access`: **4 passed, 1 failed**.
  Case 0 read `$C9,$D8` from the redirected address instead of `$A6,$37`.
  All four existing guests passed.

After implementation, a legal mode-3/7 control exposed a mistaken test-vector
expectation that had omitted WMDATA's carry into bit 8. That expectation was
corrected from `$123FE` to `$124FE`, following the documented mode pattern.
Mapped progression controls were also corrected to retain their initial clear
bit 16. These are test-vector corrections, not additional production changes.
Final focused results: **38 unit tests and five CPU guests passed**.

## Mandatory validation

Commands ran from the assigned worktree. Rust gates used default Homebrew Rust
1.94.0. Wasm and the optional SPC assembler build selected rustup Rust 1.99.0
with `RUSTC="$(rustup which rustc)"`, as documented in README. Source pins and compiler requirements were preserved.

| Command | Actual result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Pass |
| `cargo test --workspace --locked` | Pass: 153 tests, zero failed/ignored; doctests pass |
| `cargo test --locked -p snes-core --lib machine::dma_tests` | Pass: 38 tests |
| `cargo test --locked -p snes-core --test dma_access` | Pass: five CPU guests |
| `python3 tools/make-fixture.py .cache/smoke.bin` | Pass |
| `cmp tests/roms/smoke.bin .cache/smoke.bin` | Identical; SHA-256 below |
| `python3 tools/build-test-tools.py --spc` | Pass: all three pinned assemblers built |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Pass: full CPU `$0649`, basic CPU `$0452`, SPC `$0557`; each 600 frames and expected terminal PC |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Pass: 11 grayscale bands and final shade through line 223 |
| `python3 tools/build-cleared-games.py` (twice) | Pass: both fresh builds reproduce both pinned ROM hashes |
| `npm --prefix web ci` | Pass: zero reported vulnerabilities |
| `npm --prefix web run build` | Pass |
| `(cd web && npx playwright install chromium)` | Pass |
| `SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'` | Pass: all 18 tests, 2.5 minutes |

The pinned SPC source download was slow but completed and passed its hash check.
A bounded duplicate download probe was stopped after the original build completed;
its partial archive was never used. The combined CPU/SPC command above passed;
a preceding CPU-only invocation also passed. All final mandatory gates passed; none were skipped or left pending. Port 18085 was selected using README's documented override.

Browser evidence confirms 18,100 frames and 20 checked gameplay windows per game.
Castle completed all five maps and credits at frame 11,097. Memory won and restarted
18 times. Music, independent finite effects, gesture-activated Web Audio,
queue/drift bounds, freeze rejection, SRAM failure controls and the Pages subpath
smoke test passed. Runs, assertions and negative controls were preserved.

Reproduced SHA-256 values:

- Original smoke: `73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb`.
- Castle: `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`.
- Memory: `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.

Generated command logs remain in ignored `.cache/issue-27/`. Hardware, PPU and
browser structured evidence stays in `.cache/hardware-tests/results.json`,
`.cache/ppu-test/results.json`, `.cache/cleared-games/` and `web/test-results/`.

## Remaining uncertainty and inventory

DMA and HDMA remain partial. Retained clocks assert emulator regressions, not
hardware startup/alignment, HDMA initialization/overhead/first-frame accuracy.
Reverse conflicts, electrical/open-bus values, unresolved `$4200–$420F`
exclusions, same-channel contention and the complete interaction matrix remain
outside this change. No physical-hardware measurement was performed. No commercial
ROM or firmware was acquired. Bounded checks establish neither full hardware
accuracy nor commercial compatibility.

All 24 inventory categories retain their scopes/statuses: `cpu`, `bus`, `scheduler`,
`dma`, `hdma`, `controllers`, `ppu-bg`, `ppu-offset`, `ppu-mode7`, `ppu-objects`,
`ppu-color`, `ppu-ports`, `ppu-display`, `spc`, `ipl`, `dsp-noise`, `dsp-pitch`,
`dsp-envelope`, `dsp-echo`, `dsp-timing` are partial; `dsp-voices` is
implemented_unverified; `ppu-hires`, `ppu-raster`, `pal` are unimplemented.
There are no optional or required post-merge checks for this Issue.
