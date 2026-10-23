# Issue #21: DMA/HDMA A-bus I/O exclusions (historical)

Validated on 2026-10-23 against evaluation baseline
`03cc363011cab05992f0c2165d1c43cfef4b1764` for
[Issue #21](https://github.com/takahirox/snes-rust/issues/21).

**Superseded in part by [Issue #23 validation](issue-23-validation.md).**
The `$4210–$421F` read exclusion and assertions preserving pending NMI/IRQ below
were incorrect. Those reads now reach the existing register handlers in every
DMA/HDMA path, including descriptor/pointer fetches, and acknowledge flags.
The sections below record the original Issue #21 implementation and its checks,
not the current read contract. Current tests replace those contradictory vectors;
write restrictions, unrelated exclusions and their regression coverage remain.
Zero excluded-read values and existing `$4200–$420F` restrictions remain emulator
approximations, not hardware certification.

## Contract and reference limitations

The implemented contract is the Issue's explicit address table: in banks
`$00–$3F/$80–$BF`, A-bus offsets `$2100–$21FF`, `$4000–$41FF`,
`$4200–$421F` and `$4300–$437F` read as zero and discard writes. The
ordinary register handlers are never invoked. B-bus accesses still execute;
CPU accesses and cartridge/WRAM decoding outside those banks retain their behavior.
The same rules apply to HDMA descriptors and indirect-address bytes. A blocked
descriptor is a zero terminator, after advancing its table pointer.

The named hardware references were inspected, and **do not fully agree with
the requested contract**:

- [Fullsnes](https://problemkaputt.de/fullsnes.htm), **SNES Memory Map →
  System Area**, identifies the I/O bank mirrors. **SNES DMA and HDMA Channel
  0..7 Registers** describes separate A/B endpoints, constant banks, address
  progression, indirect pointers and zero-descriptor termination.
- Fullsnes **Unpredictable Things → Open Bus for DMA** lists `$2100–$21FF`,
  `$4000–$41FF` and `$4300–$437F` as open-bus areas, but explicitly describes
  `$4210–$421F` as accessible and discusses nonzero electrical bus values.
- [Anomie's Register Doc](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt),
  revision 1157, **DMA AND HDMA**, describes A-bus exclusion of B-bus ports
  and `$4300–$437F/$420B/$420C`, and ineffective writes, but describes
  the reads as open bus. **43x0 DMAPx** describes direction in both HDMA modes.

Thus the full CPU-register exclusion and explicit zero payload are requirements
of Issue #21, not independently established hardware facts from those documents.
These tests verify that contract; they do not resolve the documentation conflict
or establish exact electrical behavior. No emulator source was consulted.

The existing four-clock read phase, four-clock write phase, 16-clock aggregate
general-DMA setup, HDMA table-fetch placement and eight-clock table-fetch slots
are retained emulator conventions. Eight clocks per payload byte is documented,
but these tests do not establish hardware startup/alignment or fetch-phase timing.

## Independent expected values and coverage

Original [unit vectors](../crates/snes-core/src/machine/dma_tests.rs) use literal
allowed/blocked offset and bank tables, without consulting production decoding
helpers. They execute real transfers and inspect the legal B-bus output:

- 20 offsets × 14 banks cover each range's endpoints and interiors, all adjacent
  allowed gaps, and both sides of the mirrored-bank boundaries. `$41FF/$4200`
  are adjacent prohibited ranges, not an allowed gap. Nonzero ROM `$6B`, SRAM
  `$6D`, WRAM `$7C` and ordinary unmapped/open-bus `$5A` controls make incorrect
  zeroing visible. Reverse controls retain cartridge ROM/SRAM and WRAM decoding.
- Fixed `$4016`, incrementing `$20FF/$21FF`, decrementing `$2200/$2100` and
  `$FFFF/$0000` wrapping have literal byte vectors and final A-address/count/bank
  assertions. Legal unmapped reads retain the preceding bus byte, so leaving
  a blocked region can still yield zero; entering from `$5A` exposes the transition.
  Reverse tests check fixed/boundary/wrap progression with legal CGRAM reads.
  Short runs start at H=0, away from refresh, and require `16 + 8*N` clocks;
  isolated payloads require eight clocks even when blocked.
- `$4016` retains serial `$9180`; `$4210/$4211` retain pending NMI/IRQ;
  `$2180` retains pointer `$1000` and data `$5A,$12`. Real CPU bus reads then
  shift/acknowledge/increment them. Reverse writes preserve PPU/WRAM/controller,
  CPU control/math/DMA-enable and channel registers, with legal CPU write controls.
- Forward blocked reads still write zero to CGRAM/MOSAIC. Reverse blocked writes
  still consume successive CGRAM bytes `$35,$12`, exposing `$67` on the next read.
  Legal B-bus WRAM writes advance their pointer and retain numeric byte vectors.
  The side-effect tests avoid the separate WRAM-to-B-bus-`$2180` conflict.
- Direct and indirect HDMA payloads exercise both directions and protected read
  side effects. Complete tables also exercise pointer loading, payload advancement
  and subsequent termination. Blocked descriptors terminate without touching
  the underlying ports; table pointers still advance by one. A descriptor at
  `$20FF` or `$42FF` is legal, its two blocked pointer bytes produce `$0000`,
  then a legal payload advances the indirect pointer to `$0001`. A table at
  `$80FFFE` wraps its pointer fetch through `$800000`, independently of bank carry.

Two original [65C816 guests](../crates/snes-core/tests/dma_access.rs) generate
32 KiB LoROMs entirely from checked-in Rust source and execute initialization,
transfers, register readback and numeric WRAM results through `Machine`. Each
must reach its STP success marker within 10,000 instructions and fewer than two
frames. Host code supplies only the ROM and controller input. See
[MIT provenance](../tests/roms/PROVENANCE.md#original-dma-a-bus-65c816-guests-approved).

The forward guest reproduces `$00:2180 → $2122`, fixed address, count two:
CGRAM receives `$0000` and subsequent CPU WRAM-port reads return `$5A,$12`.
The reverse guest reads CGRAM while attempting to overwrite `$4201`, `$437F`
and `$2180`; CPU-visible registers and WRAM remain unchanged while the B-bus
pointer advances. CPU-written result vectors are independent literals.

## Baseline failure evidence

An isolated archive under `.cache/issue-21-baseline` was made with
`git archive 03cc363011cab05992f0c2165d1c43cfef4b1764 | tar -x -C ...`.
The two new test files were copied unchanged into it; only the test-module
declaration was appended to baseline `machine.rs`. Production logic and all
existing tests remained exactly at baseline. No new expectation was weakened.

Executed from that archive:

```sh
cargo test --locked -p snes-core --lib machine::dma_tests
cargo test --locked -p snes-core --test dma_access
```

Both returned exit 101: eight unit regressions failed, the outside-I/O-bank
reverse control passed, and both guests failed. The same nine unit tests and
two guests pass in the implementation worktree.

| Regression | Baseline actual | Required and fixed result |
| --- | --- | --- |
| Forward `$2180` into CGRAM | `$5A,$12` | `$00,$00` |
| Reverse blocked PPU MOSAIC write | register `$12` | register `$00` |
| HDMA descriptor at `$00:2180` | active mask `$01` | terminated mask `$00` |
| HDMA indirect pointer after blocked fetches/payload | `$8182` | `$0001` |
| HDMA table-loaded prohibited payload | WRAM pointer `$1001` | `$1000` |
| Forward guest `$1800–$1809` | `00 00 5A 12 80 21 00 00 00 00` | `5A 12 00 00 80 21 00 00 00 01` |
| Reverse guest `$1800–$1804` | `12 67 23 00 00` | `A6 67 A6 5A 12` |

## Commands and results

Commands ran in the assigned worktree. The mixed macOS toolchain used Homebrew
Rust 1.94 for workspace checks and `RUSTC="$(rustup which rustc)"` (Rust 1.99)
for the optional SPC assembler and Wasm build, as documented in README.
Node was 24.12.0 and Python 3.13.13. No allowance reset, purchase or provider/model
switch was used.

| Command | Actual result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --workspace --locked` | Passed: 121 tests, including nine new unit tests and two new guests |
| `python3 tools/make-fixture.py .cache/smoke.bin` | Passed; original hash unchanged |
| `cmp tests/roms/smoke.bin .cache/smoke.bin` | Passed, byte-identical |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Passed; pinned cc65, bass and SPC assembler built |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Passed: full/basic 65C816 and SPC, 600 frames each |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Passed: 11 grayscale bands, 16 lines each, final shade through line 223 |
| `python3 tools/build-cleared-games.py` (twice) | Both passed; both games matched their pinned hashes on both builds |
| `npm --prefix web ci` | Passed |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Passed |
| `(cd web && npx playwright install chromium)` | Passed |
| `SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'` | Passed: 18 tests in 2.5 minutes |

The browser gate retained both 18,100-frame cleared-game replays, Castle's complete
five-map/credits route, Memory's music/effect and live Web Audio checks, freeze
negative controls, diagnostic controls and SRAM recovery, and the Pages subpath
smoke. There are no skipped or unperformed mandatory execution gates.

All pre-existing assertions, replay lengths, fixtures, negative controls and
licensed-source alteration policies are retained. Hardware evidence remains in
`.cache/hardware-tests/results.json` and `.cache/ppu-test/results.json`; browser
evidence is generated under `web/test-results`. Build/download artifacts remain
ignored. The hashes verified here are:

| Artifact | SHA-256 |
| --- | --- |
| Full CPU | `1727e7c46ebb5df55711598264ba19005986bd34e915621383966adf674b3cfa` |
| Basic CPU | `514d58950ae1ca940d6673c2a028b258b2ab17293bbc7cbd7cb54dded32abc5b` |
| SPC | `279d6a92d051ff8a98e190ce73e40482ff2cc981812509c7bca35a047484507e` |
| HDMA | `aef339e5d4a7319db31de577b253d977b45f5c45f09adfaa99a302ca300a957a` |
| Smoke | `73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb` |
| Cleared Castle | `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869` |
| Cleared Memory | `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1` |

## Remaining scope

`dma` and `hdma` remain partial. WRAM-to-B-bus-`$2180` conflicts, startup/
alignment accuracy, HDMA initialization/overhead and first-frame behavior,
same-channel contention and exact electrical/open-bus behavior remain separate
work. The DMA/HDMA mode-interaction matrix is not completed. All other supplied
inventory categories retain their previous scopes, statuses and limitations,
including the existing CPU/bus, PPU and DSP limitations. These checks establish
neither full hardware accuracy nor commercial compatibility. No commercial ROM
or firmware was acquired. Optional user-supplied original-game replays and
physical hardware checks were not run. No post-merge verification is required.
