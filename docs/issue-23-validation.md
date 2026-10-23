# Issue #23: accessible DMA/HDMA status and result reads

Validated on 2026-10-23 for [Issue #23](https://github.com/takahirox/snes-rust/issues/23),
against baseline `e77cad3e5185959bb1c264d23833223215984923`.
This supersedes the `$4210–$421F` read contract and contradictory flag-preservation
assertions in [Issue #21's historical evidence](issue-21-validation.md).

## References and corrected access rule

Both references were fetched and inspected for this correction; no emulator
implementation was consulted. Precise supporting sections are:

- [Fullsnes](https://problemkaputt.de/fullsnes.htm), **SNES Unpredictable Things →
  Open Bus → Open Bus for DMA**, explicitly makes `$4210–$421F` accessible and
  identifies `$2100–$21FF`, `$4000–$41FF`, and `$4300–$437F` as open-bus areas.
  **SNES Memory Map → System Area** supplies the system-bank mirrors.
  **SNES PPU Interrupts → 4210h RDNMI / 4211h TIMEUP / 4212h HVBJOY** describes
  read acknowledgement, the separate queued CPU NMI event, and status bits.
  Its **Open Bus → Unused bits** table identifies floating masks `$70/$7F/$3E`
  for those three registers. Tests mask status to `$80/$80/$C1`.
- [Anomie's Register Doc, revision 1157](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt),
  **4210 RDNMI**, **4211 TIMEUP**, **4212 HVBJOY**, **4213 RDIO**,
  **4214–4217 RDDIV/RDMPY**, and **4218–421F JOY1–4** describe acknowledgement,
  driven bits, arithmetic and controller results. **DMA AND HDMA → DMA**, the
  AAddress-access notes, excludes B-bus registers and `$4300–$437F/$420B/$420C`.
  **DMA AND HDMA → HDMA**, table/pointer steps, describes fetched zero
  termination and two-byte indirect addresses. **43x0 DMAPx**, direction bit,
  applies to both direct and indirect HDMA.

In `$00–$3F/$80–$BF`, every A-bus read at `$4210–$421F` now calls the existing
register handler. This includes general DMA, direct/indirect HDMA payloads,
descriptors and indirect pointers. RDNMI and TIMEUP acknowledge their flags;
RDNMI acknowledgement preserves an already queued NMI. A descriptor terminates
according to its fetched value, rather than merely its register address.

Writes retain the previous exclusion map. CPU and B-bus decoding, outside-system-bank
cartridge/WRAM decoding, transfer scheduling and address/count progression are
unchanged. Existing handlers are reused without controller or RDIO expansion.
`$421A–$421F` remain zero in the current controller model; testing their routing
is not verification of additional controller ports or data lines.

## Explicit limits

Excluded-read zero bytes remain an approximation, not documented electrical
open-bus values. Floating-bit retention and unmapped-address values in boundary
controls test the current emulator model. No exact electrical value is inferred.
Existing read/write restrictions elsewhere in `$4200–$420F` are retained and
remain unresolved where unsupported; this correction does not certify them.
In particular, Anomie's TIMEUP entry discusses writes ambiguously; this issue
preserves existing ineffective writes rather than expanding write semantics.

The four-clock read/write phases, aggregate 16-clock DMA setup, HDMA fetch
placement and eight-clock fetch slots remain emulator conventions. Tests retain
those timings to detect unintended changes; they do not establish startup,
alignment, IRQ/NMI boundary races or HDMA initialization/overhead accuracy.

## Original vectors and guest

[Unit vectors](../crates/snes-core/src/machine/dma_tests.rs) use independent
literal expectations, without calling the production access predicate:

- All sixteen offsets run through real general DMA in `$00/$3F/$80/$BF`, with
  `$40/$C0` ROM and `$7E/$7F` WRAM controls. All sixteen also run as direct and
  indirect HDMA payloads, both directions, in the four system-bank mirrors.
  Read-only writes preserve register state while legal CGRAM B-bus reads advance.
- Fixed-source RDNMI/TIMEUP DMA to `$2180` gives bit 7 `$80,$00`, acknowledges
  only the addressed flag and preserves the unrelated flag and queued NMI.
  A `Machine` vector executes that queued NMI after DMA acknowledgement.
  Existing deferred-NMI, VBlank access-boundary and HDMA-preemption tests remain.
- Stable HVBJOY active/blank/busy/completed states use mask `$C1`; floating bits
  are not asserted. Completed `$ABCD / 19` yields quotient `$090A`, remainder
  `$000F`; `$FF * $AB` yields `$AA55`. Automatic first-controller input completes
  to `$A590`. RDIO `$A6` tests the existing model, without claiming external pins.
- Fixed, incrementing and decrementing runs check both `$420F/$4210` and
  `$421F/$4220`, final addresses, banks, counts, WRAM-port advancement and retained
  timing. Existing excluded-endpoint, bank-wrap, B-bus side-effect and CPU-access
  controls remain; only contradictory status-register exclusions were replaced.
- Accessible RDNMI descriptors remain nonzero even with bit 7 clear (version 2);
  TIMEUP tests nonzero and zero descriptors. Zero RDIO terminates too. Tests
  assert line counters, active masks, payloads, acknowledgement and table advancement.
- Complete indirect tables feed status-register payloads then terminate on the
  following zero descriptor. Register-backed indirect-pointer pairs use
  TIMEUP/HVBJOY and RDDIVL/H, assert the fetched pointers through nonzero WRAM
  payloads, and check both pointer progressions. Sequential RDNMI/TIMEUP fetches
  independently check acknowledgement in both byte slots; `$420F` remains excluded,
  so no accessible descriptor immediately before RDNMI is invented.

The new original [65C816 guest](../crates/snes-core/tests/dma_access.rs) creates
its own VBlank NMI flag, vertical timer IRQ, completed arithmetic and automatic
controller result. Guest instructions initialize registers, poll HVBJOY, trigger
DMA, mask status and copy numeric results into CPU-visible WRAM. The host only
supplies the ROM and `$A590` controller input. The guest must reach its STP marker
within 100,000 instructions and fewer than two frames. The two original exclusion
guests retain their 10,000-instruction/two-frame bounds and assertions.
[MIT provenance](../tests/roms/PROVENANCE.md#original-dma-a-bus-65c816-guests-approved)
covers all three source-built LoROMs; no additional binary, firmware or commercial
ROM was acquired or committed.

## Baseline failure evidence

An isolated `git archive e77cad3e5185959bb1c264d23833223215984923` was extracted
under `.cache/issue-23-baseline`. Only the two updated test files were copied
into that archive. All other tracked files under `crates/` were byte-compared
with baseline and were unchanged, including production `machine.rs` (SHA-256
`0a1392d8608261e7b12f5d7c2cfcbd7d2e015560c6e74f3c564b1b2a72720129`).
The test module already exists at baseline; no production/module edit was needed.

Both commands returned exit 101 there:

```sh
cargo test --locked -p snes-core --lib machine::dma_tests
cargo test --locked -p snes-core --test dma_access
```

Ten positive-read unit regressions failed; ten controls passed. The new guest
failed; both original guests passed. In the correction worktree all 20 unit tests
and all three guests pass. Logs are under `.cache/issue-23/`.

| Case | Baseline actual | Corrected expectation/result |
| --- | --- | --- |
| Repeated status DMA | bit 7 `$00,$00` | `$80,$00`, flag acknowledged |
| Completed product DMA | `$00,$00` | `$55,$AA` |
| Accessible descriptor | active mask `$00` | active mask `$01` |
| Register-backed pointer and payload | MOSAIC `$00` | `$66` |
| Guest `$1800–$1811` | eighteen zero bytes | `80 00 80 00 80 A6 0A 09 0F 00 90 A5 00 00 00 00 00 00` |

## Commands and actual results

Workspace checks used Homebrew Rust 1.94.0; the SPC tool and Wasm build used
`RUSTC="$(rustup which rustc)"` (Rust 1.99.0), as documented in README for this
mixed macOS installation. Node was 24.12.0 and Python 3.13.13.

| Command | Actual result |
| --- | --- |
| `cargo test --locked -p snes-core --lib machine::dma_tests` | Passed: 20 |
| `cargo test --locked -p snes-core --test dma_access` | Passed: 3 |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed after fixing a precedence warning in a new test expression |
| `cargo test --workspace --locked` | Passed: 133 tests |
| `cargo test -p snes-core --test dsp_noise --locked` | Passed: 4 |
| `cargo test -p snes-core --test dsp_pitch --locked` | Passed: 3 |
| `cargo test -p snes-core --test ppu_offset --locked` | Passed: 12 |
| `python3 tools/make-fixture.py .cache/issue-23/smoke.bin` and `cmp tests/roms/smoke.bin .cache/issue-23/smoke.bin` | Passed: byte-identical |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Passed: pinned cc65, bass, SPC assembler |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Passed: full/basic CPU and SPC, 600 frames each |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Passed: 11 grayscale bands, 16 lines each, final shade through line 223 |
| `python3 tools/build-cleared-games.py` (twice) | Passed: both game hashes matched on both builds |
| `npm --prefix web ci` | Passed |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Passed |
| `(cd web && npx playwright install chromium)` | Passed |
| `SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'` | Passed: 18 tests in 2.5 minutes |

The browser gate retained both 18,100-frame cleared-game replays, Castle's
five-map route and completed credits, Memory's rendered music/effects and live
Web Audio, freeze negative controls, diagnostic controls, SRAM recovery and Pages
subpath smoke. No runs, assertions or fixtures were shortened or weakened. No
mandatory pre-merge check was skipped or left unperformed. Optional user-supplied
original-game replays and physical hardware checks were not run.

Hardware results are in `.cache/hardware-tests/results.json` and
`.cache/ppu-test/results.json`; browser evidence goes to `web/test-results`.
Generated artifacts remain ignored. Source pins, licensing decisions and fixture
alterations remain as recorded in [provenance](../tests/roms/PROVENANCE.md).
Smoke, CPU/SPC, HDMA and cleared-game hashes match the values in
[Issue #21's artifact table](issue-21-validation.md#commands-and-results).

## Remaining scope

`dma` and `hdma` remain partial. Exact open-bus/electrical behavior, WRAM–`$2180`
conflicts, DMA startup/alignment, HDMA initialization/first-frame/overhead,
same-channel contention and the full mode/channel/device-interaction matrix
remain separate work. This does not expand controllers or RDIO external pins,
redesign scheduling or complete Issue #3.

All inventory scopes and statuses remain unchanged: `cpu`, `bus`, `scheduler`,
`dma`, `hdma`, `controllers`, `ppu-bg`, `ppu-offset`, `ppu-mode7`, `ppu-objects`,
`ppu-color`, `ppu-ports`, `ppu-display`, `spc`, `ipl`, `dsp-noise`, `dsp-pitch`,
`dsp-envelope`, `dsp-echo`, `dsp-timing` remain partial; `dsp-voices` remains
implemented_unverified; `ppu-hires`, `ppu-raster`, `pal` remain unimplemented.
These suite passes establish neither full hardware accuracy nor commercial
compatibility. No allowance reset, purchase or model/provider switch was used.
No post-merge verification is required.
