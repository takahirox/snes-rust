# Issue #39: WRIO-controlled PPU counter latching

Implemented for [Issue #39](https://github.com/takahirox/snes-rust/issues/39),
from checkpoint `d9cd7a28a86a0eee41b1ebce42203f85e12d27a9`.
Evaluation baseline: `9dedbee9a1baaa7d3d80bfd54a77046cfc2b9b03`.
All new instructions, data and tests are original MIT work.

## Register evidence and qualifications

Retrieved the complete reference entries before implementation on 2026-10-24.
Ignored raw copies and logs are under `.cache/issue-39/`; no third-party
reference text or emulator implementation is included in the change.

| Reference | Version / raw SHA-256 | Supporting sections (raw file line numbers) |
| --- | --- | --- |
| [Anomie's Register Doc](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt) | Revision 1157, 2007-07-12; `19dfdf3f1c206655b8ede11bcd6dc053e36634f876819e40bb39cfb88a237dfd` | 2137 SLHV (685–690); 213C/213D OPHCT/OPVCT (739–751); 213F STAT78 (786–804); 4201 WRIO (929–942) |
| [Fullsnes](https://problemkaputt.de/fullsnes.htm) | no$sns v1.5; `3853b65a7331bf12eea431ff494a2948a1c78ef22f7e819db4b139fd5a79f6c6` | I/O Map WRIO initial value (139); PPU Timers and Status, SLHV/OPHCT/OPVCT (1641–1666), STAT78 (1689–1698); WRIO/RDIO (3615–3638) |

Anomie specifies high-gated SLHV with CPU open-bus return, falling-edge WRIO
capture, independent nine-bit counter selectors, and selector reset only by
STAT78. Capture sets the flag; STAT78 acknowledges only while WRIO bit 7 is
high. Its WRIO entry describes all bits high at power-up, but tentatively
suggests retention across reset. Its informal clear-and-set wording explicitly
places capture on the transition to zero. These rules supersede earlier tests'
unconditional software capture and acknowledgement assumptions.

Fullsnes confirms initial WRIO `$FF`, falling-edge capture, gating, flag setting,
and independent selectors unaffected by capture. Its STAT78 summary omits the
gate condition. Use Anomie's more specific acknowledgement rule. With the scoped
CPU-only input, entering low captures and sets the flag, and low reads cannot
clear it; repeated low reads therefore report bit 6 set. This is a derived
reachable-state consequence, without evidence for new physical lightgun inputs.

The requested `Machine::reset` behavior retains this project's full volatile
reset convention: reconstruct the bus with WRIO `$FF`, clear held values/flag
and selectors, preserve SRAM. This is **not** a source-confirmed physical warm
reset claim; Anomie's tentative retention note remains unresolved. Power-up
initialization directly sets the high level without generating an edge. General
RDIO external inputs and physical latch-pin behavior remain outside scope.

## Implementation and numerical coverage

The PPU owns the latch-input level and one capture operation. WRIO writes update
that level through the existing system-bank decoder. SLHV and STAT78 consult
it in the shared PPU read path, also used by B-bus DMA/HDMA. A rising edge changes
only the level. Captures preserve both read selectors and the CPU open-bus
return. Other CPU I/O initialization remains unchanged.

`machine/ppu_counter_tests.rs` uses ordinary timed CPU bus operations and idle
cycles to reach positions; it never injects raster, held counters or latch state.

- 64 vectors cover four mirrors (`$00/$3F/$80/$BF`), every combination of
  `$00/$7F/$80/$FF`, all four bit-7 transitions and unrelated-bit changes.
  Captures at dots 26, 50 and 51 distinguish software, first falling edge and
  subsequent writes.
- A six-clock SLHV read or WRIO write starting at `(V=255,H=1360 clocks)`
  completes at `(256,2)` and captures `(0,256)`. A later edge captures `(300,256)`;
  disabled SLHV retains that value across a scanline. Repeated low status reads,
  release, two high status reads and re-enabled SLHV check acknowledgement and
  held values. High counter bytes mask floating bits to bit 0.
- Non-system banks `$40/$7D/$7E/$7F/$C0/$FF` cannot drive WRIO or consume PPU
  selectors/flag. Reverse DMA STAT78 reads check low retention and high
  acknowledgement, selector reset, and destination/count progression.
- `Machine::new` and `reset` check `$FF` RDIO, no initial capture, enabled SLHV,
  cleared unrelated volatile state and preservation of nonempty SRAM.
- The PPU selector test captures `(300,260)`, `(10,17)` and `(320,258)` between
  low/high reads; it tests both ninth bits and independent selector phases.
  Repeated STAT78 reads reset both selectors at either gate level.
- Existing direct/indirect reverse-HDMA assertions retain dot 279 at service+4,
  destination commitment at +8 and line-counter progression. Added low controls
  retain dot 26. The zero-RDIO descriptor now explicitly writes WRIO zero and
  retains all prior termination, acknowledgement, pointer and timing assertions.

These clock values use the existing 1364-clock line, four-clock dot conversion,
refresh holds and CPU access-completion convention. They are model regressions,
not independently measured hardware latch-strobe timing. HDMA +4/+8 phases
remain the [Issue #35 conventions](issue-35-validation.md).

## CPU-executed guest

`tests/ppu_counter_latch.rs` builds one original 32 KiB LoROM in memory.
The guest initializes WRIO, synchronizes by polling HVBJOY and executing WAI
with masked IRQ at V=256/H=256, then publishes results in WRAM `$1800..$1821`.
The host supplies only the ROM and observes results and deadlines; it injects no
CPU, bus, raster or guest-memory state. STP plus `$18F0=$A5` is required within
100,000 instructions and fewer than three frame wraps.

After [Issue #99](issue-99-validation.md), assertion is `256*4+14=1038`
clocks and six-clock WAI polling wakes at H=1040. SLHV completes 30 clocks
later, so A is `(267,256)`. Subsequent 30-clock register loads, 32-clock WRAM
stores and a 46-clock immediate-load/register-store pair place the falling edge at
H=1272, so B is `(318,256)`. Both selectors remain high across that edge.
Disabled SLHV and repeated low writes preserve B. Low STAT78 returns `$40`
twice; release preserves B and the first high read returns `$40`, then zero.
Re-enabled SLHV completes at `(V=258,H=324 clocks)`, producing C `(81,258)`.
Its acknowledgement sequence is again `$40,$00`. Floating bits are masked by
guest instructions, while SLHV's deterministic CPU open byte is `$21`.
See [provenance](../tests/roms/PROVENANCE.md#original-wrio-counter-latch-cpu-guest-approved).

## Baseline failures and corrected results

Temporarily substituted production `machine.rs` and `ppu.rs` from the exact
evaluation baseline, adding only the new unit-module declaration. The updated
HDMA/descriptor tests and guest remained present. All three commands exited 101:

| Baseline command | Observed failure |
| --- | --- |
| `cargo test --locked -p snes-core --lib machine::ppu_counter_tests` | Five failures: reset RDIO `$00` versus `$FF`; crossing falling-edge capture `(0,0)` versus `(0,256)`; reverse-DMA status zero versus `$40` |
| `cargo test --locked -p snes-core --lib machine::hdma_phase_tests::payload_samples_at_service_plus_four_and_commits_at_plus_eight` | Enabled cases pass, first disabled case reads dot-279 low byte `$17` versus retained dot-26 `$1A` |
| `cargo test --locked -p snes-core --test ppu_counter_latch` | B incorrectly remains `(264,256)`; disabled SLHV replaces it with `(90,257)`; low/released acknowledgement sequence incorrectly becomes `$40,$00,$00,$00` |

Corrected production files were restored in a `finally` block before final gates.
The initial guest development run printed numerical results before its expected
vector was completed; that intentional development failure is separate from the
retained baseline experiment above.

## Mandatory validation

Validation commands and results are recorded below. Logs and generated artifacts
remain ignored under `.cache/issue-39/` and `web/test-results/`. Rust workspace
checks use Homebrew Rust 1.94.0. Per README, Wasm and the auxiliary SPC assembler
select rustup Rust 1.99.0 through `RUSTC="$(rustup which rustc)"`.

All mandatory gates passed; no check remains pending or skipped.

| Command | Actual result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Pass |
| `cargo test --workspace --locked` | Pass: 208 tests, none failed or ignored; doctests pass |
| `cargo test --locked -p snes-core --test ppu_counter_latch` | Pass: original CPU guest |
| `cargo test --locked -p snes-core --lib machine::ppu_counter_tests` | Pass: five tests; also included in final workspace run |
| `python3 tools/make-fixture.py .cache/smoke.bin` | Pass |
| `cmp tests/roms/smoke.bin .cache/smoke.bin` | Pass: identical |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Pass: three pinned assemblers built |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Pass: full CPU `$0649`, basic CPU `$0452`, SPC `$0557`; expected terminal PCs; 600 frames each |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Pass: eleven grayscale bands, final shade through line 223 |
| `python3 tools/build-cleared-games.py` (twice) | Pass: both fresh builds reproduce both expected hashes |
| `npm --prefix web ci` | Pass: zero reported vulnerabilities |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Pass |
| `(cd web && npx playwright install chromium)` | Pass |
| `SNES_TEST_PORT=18089 npm --prefix web test -- --grep-invert '301 seconds'` | Pass: all 18 checks, 2.6 minutes |
| `git diff --check` and staged whitespace check | Pass |

Both game replays retain 18,100 frames (301.17 emulated seconds), twenty
asserted gameplay windows and freeze controls. Castle's separate route completes
all five maps and credits; Memory's controller replay wins/restarts. Existing
music/effect separation, Web Audio, queue/drift, pause/reset, SRAM failure
recovery and Pages subpath assertions all pass. Port 18089 was selected before
starting the suite. No browser test or input was modified.

Reproduced SHA-256 values:

- Smoke: `73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb`.
- Castle: `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`.
- Memory: `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
- Licensed HDMA: `aef339e5d4a7319db31de577b253d977b45f5c45f09adfaa99a302ca300a957a`.

The final fmt, clippy, workspace and focused guest gates ran after restoring
corrected production from the baseline comparison. Hardware/browser production
builds used the same corrected production bytes; subsequent changes only added
unit coverage and documentation.

## Remaining limits

Physical controller/lightgun inputs, external RDIO pins, exact long-dot/field
counter conversion, physical warm-reset retention, electrical open-bus values,
the exact invalid-fetch VRAM buffer bits, OAM/CGRAM corruption and access-boundary
accuracy remain unverified. [Issue #41](issue-41-validation.md) subsequently adds
logical VRAM read-fetch restrictions and labels its buffer approximation.
Raster rendering, interlace/overscan, PAL and DMA
startup/alignment are separate work. Limited passes establish neither full
hardware accuracy nor commercial compatibility. No fixtures, game hashes,
notices, replay inputs or gameplay/audio assertions changed.

All inventory statuses remain unchanged, including partial `ppu-ports`, `bus`
and `scheduler`. This bounded register correction does not complete Issue #3.
No optional or required post-merge verification is specified.
