# Issue #29: reverse WMDATA/WRAM conflicts

Validated on 2026-10-23 for [Issue #29](https://github.com/takahirox/snes-rust/issues/29).
Assigned checkpoint: `81748360759c68e98fa69b17711eaf69f3738a8a`; its production
code matches evaluation baseline `8f3e56850b0a9f6584a3086749e6a1647ae93a70`.
This delivers the supported side-effect correction, rather than a prerequisite-only
investigation. The electrical byte remains explicitly unverified.

## Retrieved specification evidence

Fetched the following exact URLs with `curl -fL --max-time 45`, then used
`shasum -a 256` and inspected the requested sections before implementation.
Line numbers below refer to the raw fetched files, including HTML markup, not
browser-extracted text. Copies remain local in `.cache/issue-29/`; third-party
reference documents are not redistributed here. Neither another emulator's code
nor existing assertions supplied a hardware oracle.

| Reference | Version | Retrieved SHA-256 |
| --- | --- | --- |
| [Anomie's Register Doc](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt) | Revision 1157; 2007-07-12 16:39:41 -0400 | `19dfdf3f1c206655b8ede11bcd6dc053e36634f876819e40bb39cfb88a237dfd` |
| [Fullsnes](https://problemkaputt.de/fullsnes.htm) | Page identifies itself as extracted from no$sns v1.5; retrieved 2026-10-23 | `3853b65a7331bf12eea431ff494a2948a1c78ef22f7e819db4b139fd5a79f6c6` |

Anomie's **2180 WMDATA**, raw lines 828–838, explicitly distinguishes forward
suppression from reverse destination writes. For reverse it says “the value
written is (initially) the Open Bus value.” The following sentence states:
“In either case, the address in $2181-3 is not incremented.” Thus an ordinary
WMDATA read/increment is incorrect, and suppressing the A-bus write would also
be incorrect. The qualifier about the initial value does not supply a stable
numeric byte, later-byte sequence, decay model, or a guarantee that a particular
bit differs from either sentinel.

Other inspected passages, paraphrased:

- **2181–2183**, raw lines 841–858: 17-bit address, full `$7E/$7F` WRAM and
  first-8-KiB system-bank mirrors, inaccessible address-register reads, and
  ineffective WRAM-sourced address-register writes. Reverse reads of these
  three address registers remain outside this change.
- **43x0 DMAPx**, lines 1190–1223: direction applies to HDMA too. Indirect
  reverse HDMA reads descriptors normally and writes to the specified A address;
  direct reverse HDMA writes payload positions in the table. All eight byte
  patterns are listed. Fixed/decrement settings affect DMA only.
- **43x1 BBADx**, lines 1233–1239: effective B address wraps within `$21xx`.
  **43x2–43x7**, lines 1250–1302, separates the constant A bank, 16-bit DMA
  address/count, and indirect payload bank/address.
- **DMA AND HDMA → DMA**, lines 2394–2415: simultaneous WRAM A/B access fails
  at the B port. The subsequent address/count steps continue, keep the bank
  constant, and treat initial count zero as 65,536 slots.
- **HDMA**, lines 2425–2492: shared modes/port, separate table fetches and payload
  processing, address/line progression, repeat state and zero termination.
  The payload conflict note at lines 2477–2478 applies the DMA WRAM/$2180
  restrictions to HDMA. Combining this with the explicit direction rule above
  yields the direct/indirect reverse contract; the pseudocode's forward-only
  spelling must not override the direction bit.

Fullsnes **Work RAM Access → DMA Notes**, raw HTML lines 449–478, independently
excludes WRAM-to-WRAM transfers in both directions because the WRAM chip cannot
process the two addresses together. **Overall Memory Map / System Area**, lines
312–346, confirms the full banks and mirror boundaries and identifies `$2000`
as unused rather than WRAM. **DMA and HDMA Channel 0..7 Registers**, lines
725–859, confirms direction/addressing flags, byte patterns, banks, counts,
table formats and row progression. It does not independently settle which
reverse A write occurs or whether the WMDATA pointer advances.

Fullsnes **Open Bus for DMA**, lines 26749–26783, discusses unused A-bus reads,
startup and retention, then qualifies retention observations according to an
attached XBOO cable/cartridge. It supplies no numeric oracle for this simultaneous
reverse B-port conflict. In particular, its initial high-impedance discussion
must not be generalized into a guaranteed `$FF` conflict byte.

## Direction-specific contract matrix

The following is scoped to each payload byte whose current A address selects
WRAM and whose effective wrapping B address is `$2180`.

| Transfer | A-bus WRAM write | WMDATA pointer | Byte guarantee / evidence level |
| --- | --- | --- | --- |
| General reverse DMA | Occurs; explicit in Anomie 2180 | All 17 bits retained; explicit in 2180 | Initially open bus, qualitative only; exact byte/sequence unverified |
| Direct reverse HDMA | Occurs at current table payload position; derived from 43x0 direction and HDMA conflict applicability | Retained; same derivation from 2180 | Open-bus conflict classification derived; no numeric electrical guarantee |
| Indirect reverse HDMA | Occurs at current indirect payload bank/address; same derivation | Retained; same derivation | Same unresolved numeric byte/sequence |

Descriptor and indirect-pointer reads do not have a simultaneous `$2180` B read.
A WRAM-resident table therefore does not itself establish a conflict. Conversely,
a ROM table does not make a WRAM payload destination legal. Direct reverse HDMA
can overwrite payload slots, but tests keep descriptors and future pointers
outside those slots. An A write may also alias the WMDATA-addressed WRAM byte;
retaining the pointer does not promise that byte's preservation in that case.

This corrects the earlier unresolved status recorded by Issues #25/#27 and
README, using the reverse-specific sentences, not by extending forward write
suppression. Their forward contracts and historical evidence remain intact.

## Implementation and original tests

`SystemBus::transfer` classifies the current A address once for both directions,
after computing the effective eight-bit B port. A reverse conflict bypasses
`read_raw($2180)`, retaining the pointer, then performs the existing A write.
Both four-clock phases, DMA address/count progression, HDMA payload/table/line
progression, and device advancement remain. Descriptor/pointer fetch paths are
unchanged. CPU accesses and legal reverse port reads keep ordinary decoding.

The deterministic approximation returns the previous `SystemBus::open` byte on
a conflicting read and stores it back to `open` after the A write. It deliberately
does not simulate decay, bus drivers or startup electrical state. HDMA descriptor
reads currently leave this field unchanged; that retained convention is not new
hardware evidence. Two separately named `approximation` tests verify DMA and
both HDMA addressing modes really write the chosen byte only at payload
destinations. A mixed APU/WMDATA DMA vector verifies previous-byte reuse. Those
literal byte expectations must not be cited as hardware observations.

Hardware-side-effect vectors never assert a numeric conflict destination byte.
They cover the original one-byte reproducer; both full banks and their endpoints;
`$0000/$1FFF` in mirror-bank boundaries `$00/$3F/$80/$BF`; adjacent `$2000`
controls; fixed/incrementing/decrementing destinations; mirror entry/exit;
16-bit wrapping without bank carry; zero-count/65,536-slot progression;
all eight B patterns; mixed `$217F/$2180`; BBAD wrapping; complete direct/indirect
HDMA tables with repeated/non-repeated rows and zero termination; line/transfer
state, enabled/active masks and table/payload pointers. Indirect HDMA also crosses
mirror and 16-bit boundaries and demonstrates that DMA step flags do not apply.
The wrapping read control asserts progression and no WMDATA access; existing
forward BBAD wrapping checks the observable PPU write destination.

Legal controls include reverse WMDATA-to-SRAM with 17-bit wrap, reverse ROM
write protection with real WMDATA advancement, and ordinary readable B ports
writing WRAM. Existing forward ROM/SRAM, CPU WMDATA, NMI/IRQ acknowledgement,
access-boundary, refresh, device advancement and HDMA-preemption tests remain.
No reverse `$2181–$2183` reads are introduced by the mode-4 vector.

The sixth original CPU guest in `tests/dma_access.rs` initializes all memory and
registers through instructions, exercises reverse conflicts in both full banks
and a mirror, observes pointer retention through subsequent CPU reads, and
records numeric A-address/bank/count and readback results in WRAM. A `$1FFFF`
case also checks legal CPU wrap. Separate reverse ROM and CGRAM-to-WRAM controls
check legal port advancement and writes. STP plus a success marker must occur
within 10,000 instructions and fewer than two frames. No unspecified conflict
byte is asserted. [Provenance](../tests/roms/PROVENANCE.md) records the original
MIT instruction/data/header inputs and in-memory build; no binary is added.

## Baseline and corrected results

With the final tests installed, temporarily replaced only `machine.rs` with
`git show HEAD:crates/snes-core/src/machine.rs`, ran both focused commands, and
restored the fixed bytes in a Python `finally` block. Production baseline bytes
were unchanged; no emulator reference implementation was used.

- Unit command: **38 passed, 10 failed**. All ten new tests failed; all existing
  tests passed. The exact reproduction actually printed destination `$A6`,
  pointer `$11001`, confirming the planning prediction as an emulator observation.
  It failed the hardware-derived `$11000` pointer assertion. The selected
  approximation is tested separately; the reproduction has no byte oracle.
- CPU guest command: **five passed, one failed**. Case 0 recorded
  `[1,1,$7E,0,0,$37,0]` instead of `[1,1,$7E,0,0,$A6,$37]` because DMA had consumed
  the first port byte. All existing guests passed.
- After correction: **48 focused unit tests and six CPU guests passed**.

During initial test development, a source-preservation assertion incorrectly
included an A destination aliasing the WMDATA pointer at `$1FFFF`; it was narrowed
because the documented A write may change that byte. An HDMA SRAM control was
moved from `$70:8200` (ROM) to `$70:0200` (LoROM SRAM), with matching pointer
expectations. These were test-fixture corrections, not hardware-rule changes.

## Mandatory validation

Commands ran from the assigned worktree. Rust workspace gates used Homebrew
Rust 1.94.0. Wasm and the pinned SPC assembler use rustup Rust 1.99.0 through
README's `RUSTC="$(rustup which rustc)"` selection. No pins, ROM/game assertions,
licensing decisions, deadlines, or frame counts were weakened.

| Command | Actual result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Pass |
| `cargo test --workspace --locked` | Pass: 164 tests, zero failed/ignored; doctests pass |
| `cargo test --locked -p snes-core --lib machine::dma_tests` | Pass: 48 tests |
| `cargo test --locked -p snes-core --test dma_access` | Pass: six original CPU guests |
| `python3 tools/make-fixture.py .cache/smoke.bin` and `cmp tests/roms/smoke.bin .cache/smoke.bin` | Pass: identical |
| `python3 tools/build-test-tools.py --spc` | Pass: all three pinned assemblers built |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Pass: full CPU `$0649`, basic CPU `$0452`, SPC `$0557`; each 600 frames and expected terminal PC |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Pass: 11 grayscale bands; final shade through line 223 |
| `python3 tools/build-cleared-games.py` (twice) | Pass: both fresh builds reproduced both expected hashes |
| `npm --prefix web ci` | Pass; zero reported vulnerabilities |
| `npm --prefix web run build` | Pass |
| `(cd web && npx playwright install chromium)` | Pass |
| `SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'` | Pass: all 18 tests, 2.5 minutes |

The first tool-build attempt rejected the SPC archive on a hash mismatch; it
was not used. A separate bounded 60-second download timed out partway through
the slow transfer. Resuming that download completed the archive, which matched
the unchanged pinned SHA-256
`979b0fbe19830ffb49d2f187a5512044b7514914f1d289686c340ffd1d6a6e63`.
Only then was it copied to the tool cache and built. The final combined CPU/SPC
command above passed; a preceding CPU-only invocation also passed. All mandatory
gates passed; none remain pending or skipped.

Browser results retain 18,100 frames and 20 gameplay windows per game. Castle's
route traversed all five maps and completed the credits at frame 11,097; Memory
won/restarted 18 times. Independent music/finite-effect, gesture-activated audio,
queue/drift, freeze-rejection, SRAM-failure and Pages subpath checks passed.
The documented alternate port 18085 was selected before running tests.

Reproduced hashes:

- Smoke: `73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb`.
- Castle: `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`.
- Memory: `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.

Generated logs and fixture results remain under ignored `.cache/issue-29/`,
`.cache/hardware-tests/`, `.cache/ppu-test/`, `.cache/cleared-games/` and
`web/test-results/`.

## Remaining uncertainty and inventory

DMA and HDMA remain partial. Exact electrical conflict values, open-bus
retention/decay and startup are unverified. DMA startup/alignment, HDMA
initialization/overhead/first-frame timing, same-channel contention, unrelated
A-bus exclusions, reverse address-register reads and the broader interaction
matrix remain outstanding. Retained timing constants are emulator conventions,
not new hardware accuracy evidence. No commercial ROM or firmware was acquired.

All 24 categories, scopes and statuses from the
[Issue #27 baseline inventory](issue-27-validation.md#remaining-uncertainty-and-inventory)
remain unchanged: `cpu`, `bus`, `scheduler`, `dma`, `hdma`, `controllers`, `ppu-bg`,
`ppu-offset`, `ppu-mode7`, `ppu-objects`, `ppu-color`, `ppu-ports`, `ppu-display`,
`spc`, `ipl`, `dsp-noise`, `dsp-pitch`, `dsp-envelope`, `dsp-echo`, `dsp-timing` are
partial; `dsp-voices` is implemented_unverified; `ppu-hires`, `ppu-raster`, `pal`
are unimplemented. Limited passes establish neither full hardware accuracy nor
commercial compatibility. No optional checks or required post-merge checks.
