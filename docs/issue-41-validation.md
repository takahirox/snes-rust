# Issue #41: VRAM read-buffer access restrictions

For [Issue #41](https://github.com/takahirox/snes-rust/issues/41), evaluation
baseline `c72edab2e9e49e22110a86b713d6bcf80c241f26`. Original code, vectors and
guest data are MIT licensed. No additional ROM or firmware is acquired.

## Evidence and contract (recorded before implementation)

Retrieved 2026-10-24; ignored complete raw copies are in `.cache/references/`.
Line numbers below refer to those raw files, including HTML markup.

| Reference | Version / SHA-256 | Exact supporting sections |
| --- | --- | --- |
| [Anomie's Register Doc](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt) | Revision 1157, 2007-07-12; `19dfdf3f1c206655b8ede11bcd6dc053e36634f876819e40bb39cfb88a237dfd` | Flag legend 18–44; INIDISP 48–61; VMAIN 305–329; VMADDL/H 332–339; VMDATAL/HREAD 706–724; BACKGROUNDS scanline-zero discussion 1642–1650 |
| [Fullsnes](https://problemkaputt.de/fullsnes.htm) | no$sns v1.5; `3853b65a7331bf12eea431ff494a2948a1c78ef22f7e819db4b139fd5a79f6c6` | SNES Memory VRAM Access (Tile and BG Map), VMAIN/VMADD/RDVRAM 548–624; PPU overview access restriction 927–935; PPU Control INIDISP 973–989; Timing H/V Events 26986–27022; Timing PPU Memory Accesses, PPU VRAM Load 27083–27102 |

Anomie specifies buffered returns and refill **before** increment, with the
VMAIN-selected port controlling both. Fullsnes explicitly adds prefetch after
either address-byte write and orders old-buffer return, old-address refill,
then increment. Its duplicated-first-word description qualifies Anomie's
"dummy read" explanation: the first value is already valid after an accessible
address write, but repeats on the next selected-port read. No initial garbage
read is required. The old Issue #5 attribution must be read with this correction.

Fullsnes restricts video-memory access to forced blank or VBlank. Its INIDISP
entry expressly includes VRAM, and its VRAM read description makes refill a
VRAM access, separately from returning the buffered byte. Anomie's `++?-` flags
confirm forced/VBlank availability but leave HBlank uncertain; they do not
establish HBlank permission. Use Fullsnes's explicit restriction, also during
HBlank. Its timing section describes near-continuous VRAM load and explicitly
leaves possible unused cycles unknown. This change makes no dot-arbitration
claim. Scanline 0 ends VBlank and prefetches objects; it is restricted despite
not being an output picture line (Anomie also describes rendering that line).

**Neither reference specifies the 16 buffer bits obtained by an unavailable
fetch.** They do not establish zero, retention, a renderer-addressed word, or
CPU/PPU open bus. The implementation suppresses the unavailable memory fetch
and retains the previous buffer as a deterministic approximation. Retention is
an emulator policy, **not verified hardware semantics**. This removes the
contradicted software-addressed read without inventing an electrical constant.
Tests of retained bits are policy regressions; the restriction, return ordering
and address progression have the source support above. Exact invalid-fetch
bits remain unresolved. CPU open bus and PPU1's read bus continue to receive
the returned old-buffer byte through their existing paths; neither supplies
the buffer approximation.

Let A be live VMADD, B the old read buffer, W(A) the remapped VRAM word (bit 15
mirrors), S the VMAIN step 1/32/128/128, and byte(B,p) select the port's byte.
A' replaces just the written byte of the live word address. F(A) is W(A) when
accessible, otherwise B under the explicit retention approximation.

| Operation | Returned byte, next buffer, next VMADD |
| --- | --- |
| Write $2116 | (none, F(A'), A'), replacing low byte then attempting prefetch |
| Write $2117 | (none, F(A'), A'), replacing high byte then attempting prefetch |
| Non-selected $2139/$213A read | (byte(B,p), B, A), no fetch or increment |
| Selected $2139/$213A read | (byte(B,p), F(A), (A+S) mod 65536), refill before increment |
| $2118/$2119 write | No read-buffer change, even when incrementing; existing write gate retained |

| Display state | Software-addressed fetch |
| --- | --- |
| Forced blank, any line or H phase | Available |
| Ordinary VBlank (model lines 225–261), including HBlank | Available |
| Active display lines 1–224 | Unavailable |
| HBlank of lines 0–224 | Unavailable |
| Scanline 0, outside forced blank | Unavailable |

The model keeps immediate INIDISP effects, fixed 1364-clock lines, VBlank at
225 and wrap at 262, CPU sampling at access completion, reverse DMA sampling
at +4 and destination commitment at +8. Fullsnes's tentative forced-blank
latency discussion does not resolve physical transition timing. Boundary tests
below assert the retained scheduler convention, not physical strobe accuracy.

## Implementation and original vectors

Both address-write prefetch and selected-port refill call the same guarded PPU
fetch operation. CPU and B-bus DMA/HDMA already route through this register path;
no scheduler or routing changes are needed. The returned byte is captured before
the fetch attempt; VMADD advances afterward regardless of fetch availability.
Address-byte writes replace the named byte of **live** VMADD, as the lower/upper
address-register descriptions imply. Previously they reconstructed both bytes
from stale write shadows, losing increments in the other byte. This correction
is confined to the same address/prefetch operation.

- `src/ppu_vram_tests.rs`: 256 combinations of eight display states, four remaps,
  four VMAIN step encodings and both increment-port selections. An independent
  bit-permutation oracle supplies expected physical words from distinct byte
  patterns, without production helpers. Both address writes, repeated selected
  and non-selected reads, wrap from `$FFFF` for every step, recovery under forced
  blank, and separate PPU1/CPU bus bytes are checked. Another vector checks
  write-without-refill, low-byte carry and replacement of each live address byte.
- `src/machine/ppu_vram_tests.rs`: all VRAM/register/table initialization uses
  ordinary timed bus writes. Positioning uses six/eight-clock reads and idle
  cycles, with real refresh stalls; no raster, buffer or pending-event state is
  injected. Ninety-six CPU cases finish two clocks before, exactly at, and two
  clocks after each `224→225` / `261→0` boundary: both address ports, both read
  ports, both selections and forced/unforced controls. Four stable active/HBlank
  cases test blanking changes and VBlank recovery.
- Twelve reverse-DMA boundary cases sample each selected port at boundary
  `-2/0/+2`. DMA is armed by `$420B`; the next ordinary access services it.
  The `-2` cases sample in the old state and commit in the new state four clocks
  later. They assert the returned old byte, next buffer/address, destination,
  count, bank and source-register progression. Four additional mode-1 controls
  alternate `$2139/$213A` during active/HBlank, VBlank and forced blank.
- Eight reverse-HDMA controls cross direct/indirect, forced/unforced and both
  VMAIN selections. Natural next-frame initialization and HBlank events on
  scanlines 0 and 1 perform the two-byte payloads. In forced blank, low-selected
  mode returns `$12,$78` then `$56,$BC`; high-selected returns `$12,$34` then
  `$56,$78`. Restricted controls return `$12,$34` twice under the buffer policy.
  Assertions include VMADD/buffer, destination bytes, unchanged A1T, A2A/DAS,
  live line count, row-end zero termination and preserved `$420C`. Existing
  forward VRAM-write boundary tests remain intact.

The clock assertions test the model conventions stated above. They do not
establish physical access strobes, blanking latency or full DMA alignment.

## Original CPU guests and provenance

`tests/ppu_vram_access.rs` constructs two original 32 KiB LoROMs in Rust, one per
increment-port selection. Executed 65C816 instructions initialize VRAM words
`$3412/$7856/$BC9A/$F0DE` through `$2116–$2119`, prefetch and independently
seed the buffer, poll `$4212` to enter/leave VBlank and wake WAI using masked
H/V IRQs at line 100/dot 64 and line 120/dot 280. Results distinguish stable
active display, HBlank, forced-blank recovery, duplicated first valid reads,
VBlank recovery, and restricted scanline-zero increments followed by recovery.
The guest publishes 20 literal numeric results in `$1800–$1813` and a completion
marker `$18F0=$A5` before STP. Each must finish within 150,000 instructions and
fewer than four frame wraps. The host only supplies the ROM and observes results,
terminal state and deadlines. It patches no CPU, raster, PPU, buffer or guest
results. [PROVENANCE.md](../tests/roms/PROVENANCE.md#original-vram-read-access-cpu-guests-approved)
records original source/data/build licensing. No external assembler is needed.

## Baseline counterexamples

Temporarily compiled the baseline PPU from `c72edab` with only the new test module
and a read-only test accessor added. All seven new unit tests fail; the existing
VRAM write/buffering test passes. The CPU guest test also fails. Restored the
corrected production file in a `finally` block before final gates. Ignored
comparison logs: `.cache/issue-41-baseline-unit.log` and
`.cache/issue-41-baseline-guest.log`.

For example, initialize B=`$3412`, VMADD=1 under forced blank, release blanking,
and reverse-DMA a selected read in active display. Both versions return the
old byte and advance to VMADD=2, but baseline loads B=`$7856` from word 1.
The correction suppresses that unavailable software-addressed fetch and retains
B=`$3412` under the explicit approximation. A mode-1 four-byte transfer exposes
baseline `$12,$34,$56,$78` versus corrected `$12,$34,$12,$34`. The restriction
is source-supported; the corrected retained bit pattern is a policy regression,
not independent hardware measurement. Separate accessible-period expectations
are derived from the reference's old-buffer/refill/increment ordering.

A second baseline failure replaces low VMADD after `$12FF→$1300`: the old
shadow reconstruction produces `$1227`, while live-byte replacement produces
`$1327`. Tests also replace the high byte after a selected-port increment.

## Mandatory validation results

All commands below passed on 2026-10-24. Default host Cargo/rustc is Homebrew
1.94.0; the separate SPC assembler and web build selected rustup rustc 1.99.0
with the README's `RUSTC="$(rustup which rustc)"` prefix. Ignored logs use
`.cache/issue-41-*.log`; hardware fixture evidence is also in
`.cache/hardware-tests/results.json` and `.cache/ppu-test/results.json`.

| Actual command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed, no warnings |
| `cargo test --workspace --locked` | 216 tests passed, including 134 core unit tests and the new integration test with two CPU guests |
| `cargo test --locked -p snes-core --lib vram` | Eight tests passed (seven new and existing write/buffering control) |
| `cargo test --locked -p snes-core --test ppu_vram_access` | Passed, both guest variants reached completion within bounds |
| `python3 tools/make-fixture.py .cache/smoke.bin` | Passed; SHA-256 `73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb` |
| `cmp tests/roms/smoke.bin .cache/smoke.bin` | Identical |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | All three pinned assemblers built |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Full/basic CPU and SPC suites passed; final markers `$0649/$0452/$0557`, terminal PCs `$0081A2/$0081A2/$008060` |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | All eleven independently expected grayscale bands passed |
| `python3 tools/build-cleared-games.py` (twice) | Both fresh builds reproduced both pinned ROM hashes |
| `npm --prefix web ci` | Passed |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Passed, production Wasm/static assets |
| `(cd web && npx playwright install chromium)` | Passed |
| `SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'` | 18 passed in 2.6 minutes; port 8080 was occupied and 18085 was checked free |
| `git diff --check` and `git diff --cached --check` | Passed, including new files after staging |

Browser checks include **both** cleared games' 18,100-frame five-minute runs,
Castle's full controller route across all maps through credits, Memory
win/restart, gameplay/audio assertions, independent music/effects, Web Audio
stall/repeat controls, SRAM failure recovery and Pages subpath smoke. The
`301 seconds` filter excludes only the optional unaudited-ROM tests, not these
mandatory cleared-game runs. Browser JSON, WAV and screenshots remain ignored
under `web/test-results`. No checks or assertions were weakened.

Unchanged built artifact SHA-256 values:

- Castle: `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`.
- Memory: `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
- Licensed HDMA: `aef339e5d4a7319db31de577b253d977b45f5c45f09adfaa99a302ca300a957a`.

The final format/lint/workspace/guest gates ran after restoring corrected
production and adding the final HDMA vectors. Hardware and browser production
builds used the same corrected production bytes; later changes only expanded
test coverage and documentation. Fixture/game manifests, source pins, licenses,
ROMs, replay inputs and existing gameplay/audio assertions remain unchanged.

## Remaining uncertainty and inventory

Invalid-fetch electrical buffer bits remain unspecified by these references;
retention is an explicit approximation. Physical blanking-transition latency,
access strobes, dot-level VRAM arbitration, OAM/CGRAM restrictions/corruption,
raster rendering, long dots, interlace/overscan, PAL and DMA startup/alignment
remain outside this issue. This is neither full hardware accuracy nor commercial
compatibility evidence.

All 24 categories retain their statuses, outstanding scopes and qualifications
from the [baseline inventory](issue-27-validation.md#remaining-uncertainty-and-inventory):
`cpu`, `bus`, `scheduler`, `dma`, `hdma`, `controllers`, `ppu-bg`, `ppu-offset`,
`ppu-mode7`, `ppu-objects`, `ppu-color`, `ppu-ports`, `ppu-display`, `spc`, `ipl`,
`dsp-noise`, `dsp-pitch`, `dsp-envelope`, `dsp-echo`, `dsp-timing` remain partial;
`dsp-voices` remains implemented_unverified; `ppu-hires`, `ppu-raster`, `pal`
remain unimplemented. This bounded correction does not complete Issue #3 or
any inventory category. No optional or post-merge verification is required.
