# Issue #43: OAM port address ownership

For [Issue #43](https://github.com/takahirox/snes-rust/issues/43), evaluation
baseline `f1e3a668339cf52eead622d00ec915ea7952b721`. Original changes and tests
are MIT licensed. No commercial ROM or firmware is acquired.

The automatic reload timing in this historical record is superseded by
[Issue #45](issue-45-validation.md): VBlank availability begins at H=0, but
OAM reload occurs at H=10 dots (40 master clocks). Other addressing, latch,
rendering-policy and INIDISP qualifications remain applicable. Validation counts
and results below describe the original Issue #43 revision.

## Specification record (written before production changes)

Retrieved 2026-10-24, with complete ignored raw copies in `.cache/references/`.
Line numbers refer to those raw text/HTML files, not browser-extracted text.

| Reference | Version and SHA-256 | Exact sections |
| --- | --- | --- |
| [Anomie's Register Doc](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt) | Revision 1157, 2007-07-12; `19dfdf3f1c206655b8ede11bcd6dc053e36634f876819e40bb39cfb88a237dfd` | Flag legend 18–44; INIDISP 48–63; OAMADDL/H 89–115; OAMDATA 118–138; OAMDATAREAD 693–703; SPRITES/OAM 1368–1401; priority rotation 1470–1495; scanline zero 1642–1650 |
| [Fullsnes](https://problemkaputt.de/fullsnes.htm#snesmemoryoamaccessspriteattributes) | no$sns v1.5; `3853b65a7331bf12eea431ff494a2948a1c78ef22f7e819db4b139fd5a79f6c6` | SNES Memory OAM Access (Sprite Attributes), 503–543; PPU Control/INIDISP 973–989; SNES Timing H/V Events 26986–27022; SNES Timing PPU Memory Accesses/PPU OAM load 27083–27135 |

Fullsnes distinguishes a nine-bit programmed word **reload** value from a
ten-bit internal byte address. Either address-register write reloads the whole
internal address and selects even phase. Every data-port read/write advances
one byte, wrapping at 1024; bit 9 selects the high table with only five physical
index bits. Anomie's register and SPRITES/OAM descriptions agree.

Fullsnes's EVEN write rule captures the shared low-byte latch even in the high
table; its ANY high-table write rule also commits that byte immediately. The
old implementation omitted this high-even latch effect. Low-odd writes commit
the saved latch and current byte; reads never capture the write latch. These
rules correct the incomplete Issue #5 description without changing its ordinary
low-pair/high-mirror expectations. Neither document specifies clearing the data
latch on address reload or blanking; preserving it is our explicit policy.

Rendering destroys the internal address. Anomie calls the invalidation
deterministic but unknown, including its timing, and specifically allows OAM
writes during HBlank with unexpected destinations (mid-line writes are probable).
Its RDOAM entry qualifies invalidation of reads as probable. Fullsnes INIDISP
limits ordinary software access to forced blank/VBlank; its OAM timing section
explicitly labels unused-cycle/HBlank possibilities as guesses. Thus HBlank
is not a grant of **software-addressed** access, and these sources do not justify
discarding writes or inventing an open-bus return. Scanline zero also performs
rendering/preparation despite not being an output line. Exact access strobes,
rendering destinations and electrical read values remain unresolved.

### Operation/state table

F = forced blank (any line/H phase); V = VBlank outside F (model 225–261);
A = active lines 1–224; H = their HBlank; Z = scanline 0 including HBlank.
Rules for A/H/Z establish address ownership; numeric substitute addresses below
are emulator policy, not measured hardware values.

| Operation | F | V | A | H | Z | Support / qualification |
| --- | --- | --- | --- | --- | --- | --- |
| $2102/$2103 write | Update named reload bits; copy whole reload ×2 to internal address, even phase | Same | Same, subsequently rendering-owned | Same | Same | Fullsnes OAMADDL/H; Anomie OAMADDL/H flags `++?-` leave H uncertain. Register write accepted in all states as policy; rendering subsequently overrides the memory address. |
| $2104 low-even | Capture latch; no memory commit; +1 | Same | Effective address owned by rendering; capture if effective even; +1 | Same as A | Same as A | Fullsnes OAMDATA EVEN rule; Anomie OAMDATA and SPRITES/OAM. No blanket write discard. |
| $2104 low-odd | Commit latch/current pair; +1 | Same | Rendering-owned effective address; pair commit only if effective low-odd; +1 | Same as A | Same as A | Same sections; actual restricted low-table selection unspecified. |
| $2104 high table | Mirror into $200–$21F; direct commit; even also captures shared latch; +1 | Same | Rendering-owned effective address, same byte/latch operation | Same as A | Same as A | Fullsnes OAMDATA EVEN and ANY rules, mirror description. |
| $2138 read | Current internal byte, then +1; latch unchanged | Same | Rendering-owned address/read, +1; no specified electrical byte | Same as A | Same as A | Fullsnes RDOAM; Anomie RDOAM says invalidation probably applies. Numeric active returns are policy. |
| Enter forced blank | Stop rendering ownership; preserve current internal address, phase and latch; reprogram either address register to recover | Same | Same | Same | Same | Anomie OAMDATA warns that mid-frame force blank needs address reset; latch retention is policy. |
| Automatic first-VBlank-line reload at H=10 dots | If F stays set, no reload | If F clear, reload programmed word ×2, even phase | Rendering ownership ends at H=0; reload follows at H=40 master clocks | Same | N/A | Both OAMADDL/H sections, Anomie SPRITES/OAM; detailed coordinate from Fullsnes H/V Events, corrected in Issue #45. Latch retention is policy. |
| INIDISP write with old F set on first VBlank line | Reload, even phase, preserve latch, including F→F | N/A | N/A | N/A | N/A | Selected Anomie INIDISP rule; Fullsnes only explicitly specifies F→normal here. See conflict below. |
| Exit F elsewhere / exit VBlank | No automatic programmed-address restore; rendering owns address when outside VBlank; latch retained | Later VBlank F exit preserves address | Rendering owns address | Same as A | Same as A | Fullsnes OAMADDL/H; Anomie OAMDATA warning; invalidation details approximated. |

### Disagreements and bounded approximation

Anomie SPRITES/OAM describes reload on any F 1→0 transition, while its INIDISP
entry singles out writes with old F set on the first VBlank line. Fullsnes
OAMADDL/H limits the F-deactivation reload to that first line. We select the
more specific register descriptions: automatic normal VBlank entry reload, plus
Anomie's old-F INIDISP-write rule on line 225. F→F on that line follows Anomie's
explicit register wording; it is not independently confirmed by Fullsnes.
We do not restore the programmed address on an active-display F release.

Without an object-fetch pipeline, our deterministic rendering substitute is
high-table byte `$200 | phase`, where phase is the internal address bit 0.
Rendering entry/scanline starts and restricted data accesses replace the other
address bits; each data access then increments normally. Writes still execute,
reads return that OAM byte, and both update the existing bus paths. The resulting
byte/address after a restricted access is preserved when forced blank begins;
only a documented reload event or either address write restores software control.
This deliberately small approximation touches two high bytes rather than guessing
object-fetch addresses or treating unavailable software access as discarded writes.
Actual hardware may touch other bytes, use different phases or return other bits.

A separate advancing priority cursor preserves Anomie's SPRITES priority rule:
rendering invalidation does not itself change priority rotation. Port accesses
advance both cursors; address reloads reset both. Data-latch preservation across
all reloads is policy. This is not a complete object-fetch/priority-oddity model.

Timing remains the existing fixed 1364-clock lines, VBlank at 225, frame wrap at
262, immediate INIDISP effects, CPU access completion sampling, DMA source +4 /
destination +8. Fullsnes INIDISP expressly leaves latency uncertain. Boundary
assertions below test these scheduler conventions, not physical strobe accuracy.
In particular, Fullsnes's detailed H/V Events lists OAMADD reload at H=10 dots
on V=225 whereas its register entry and Anomie describe VBlank beginning.
Issue #43 originally chose the existing line-225 event (H=0); Issue #45
supersedes that approximation with the detailed H=10 event. The original tests
below did not establish the 40-clock placement or physical forced-blank latency.

## Implementation and independent vectors

`Ppu` retains the programmed reload bits in OAMADDL/H and now separates its
rendering-owned memory cursor from the advancing priority-rotation cursor.
Address writes and automatic reloads reset both to the programmed even byte;
port accesses advance both, while rendering invalidation changes only the memory
cursor. The scheduler calls the OAM line event at every natural line transition,
including VBlank entry and frame wrap. Issue #45 separates automatic reload into
a distinct H=40 event; the line event now only updates rendering ownership.
CPU, DMA and HDMA continue through the same PPU read/write paths; there is no special transfer bypass.

- `src/ppu_oam_tests.rs`: six independent PPU tests cover forced/VBlank low-even
  capture, low-odd pair commit, interleaved reads and writes, high-even shared
  latch capture, high-odd direct commit, $100/$110 word mirrors, ten-bit wrap,
  both address-register reloads in all five states, active/HBlank/scanline-zero
  substitute effects, forced-blank preservation/recovery, normal/forced VBlank
  entry and qualified INIDISP reloads. Memory, internal/rotation addresses and
  latch are asserted against literal expected values, without production helpers
  as an expected-value oracle. A two-color overlapping-sprite test checks
  rendering invalidation preserves rotation, accessible reads advance rotation,
  VBlank reload restores it, and disabling rotation still selects sprite 0.
- `src/machine/ppu_oam_tests.rs`: six natural-scheduler tests initialize OAM,
  DMA registers and HDMA tables solely with ordinary timed writes. Positioning
  uses ordinary reads/idle cycles, retaining refresh stalls; no raster, pending
  event, PPU, DMA or result state is injected. Eight stable CPU controls cover
  active display, HBlank, scanline zero, forced blank and VBlank, followed by
  forced-blank recovery. Forty-eight CPU boundary cases cover both address
  writes, OAMDATA and RDOAM at -2/0/+2 clocks around VBlank and frame wrap with
  forced/unforced controls. Normal VBlank phase reload, suppression under forced
  blank, old-forced INIDISP behavior and frame-wrap ownership are also checked.
- Ten forward/reverse DMA controls transfer two bytes through OAMDATA/RDOAM
  in active/HBlank/scanline-zero and permitted states, asserting port memory,
  latch/cursors, count, A1T/bank, enable clearing and 16+2×8+6 clock accounting.
  Twelve boundary DMA cases access at -2/0/+2: reverse transfers sample before
  committing, forward transfers apply the destination effect at +8, and any
  intervening VBlank reload precedes the final observing CPU access. Sampled
  bytes cannot be retroactively changed by a later reset.
- Eight HDMA cases cross direct/indirect, forward/reverse and forced/unforced
  controls. Next-frame natural initialization and HBlank service on scanlines
  0 and 1 transfer two bytes per row. Assertions cover OAM/WRAM bytes,
  latch/cursors, unchanged A1T, A2A progression, indirect DAS/bank destination,
  live NTRL `$81→0`, terminating header/pointer reads, preserved `$420C`, and
  continued inactivity on the following line. The two forced rows advance
  normally; rendering rows repeatedly use the documented high-table substitute.

All restricted numeric memory addresses/returned bytes, preserved low sentinels
and downstream latch effects are **approximation-policy assertions**. Reference
support establishes rendering ownership and the ordinary register operations,
not those particular corruption destinations or electrical bytes. The dedicated
high-even latch test runs entirely under forced blank and directly exercises the
Fullsnes operation table without relying on rendering policy.

`tests/ppu_oam_access.rs` is an original MIT integration target constructing a
32 KiB LoROM in memory. Its CPU initializes four distinct OAM words through
executed writes; polls HVBJOY and uses masked H/V IRQ/WAI to reach stable active
display and HBlank; reads/writes ports; recovers under forced blank via address
writes; observes automatic VBlank reload and scanline zero after frame wrap;
and publishes 20 WRAM result bytes plus `$18F0=$A5` before STP. The host only
supplies the ROM and observes results, with a 150,000-instruction deadline and
fewer than four frame wraps. Initialized forced-blank controls are separated
from policy assertions. [Provenance](../tests/roms/PROVENANCE.md#original-oam-access-cpu-guest-approved)
records the original code/data and redistribution decision.

## Baseline counterexamples

The untouched evaluation source was extracted with
`git archive f1e3a668339cf52eead622d00ec915ea7952b721` into the ignored
`.cache/oam-baseline/`; this worktree's implementation was never replaced.

1. Copied the original CPU guest to that tree and ran
   `cargo test --locked --manifest-path .cache/oam-baseline/Cargo.toml -p snes-core --test ppu_oam_access`.
   It failed (exit 101): the first active read (result index 2) returned the
   software-selected `$12`, whereas the policy expects high-table `$A1`. The
   later forced-blank recovery observation (index 5) returned `$56` instead of
   the preserved `$12`, because baseline active writes changed the low table.
   This counterexample asserts the chosen
   bounded rendering policy; it does not measure a hardware corruption byte.
2. Appended only the independent `high_even_write_captures_latch_for_later_low_odd_commit`
   test to baseline `ppu.rs` and ran
   `cargo test --locked --manifest-path .cache/oam-baseline/Cargo.toml -p snes-core --lib high_even_write_captures_latch`.
   It failed (exit 101): low bytes were `$11,$87`, expected `$65,$87`, because
   high-even `$65` failed to replace the old `$11` latch. This is a source-backed
   accessible-period correction, independent of rendering-address policy.

Both regressions pass with the implementation. Initial local checks also exposed
and corrected a boundary-test expectation: VBlank reload after a transfer access
but before the final CPU observer resets the address even for a forward transfer.
Clippy flagged two default-then-field test initializers; those were rewritten.
Neither failure required relaxing a production rule or an existing assertion.

## Mandatory validation

Run on 2026-10-24 using README's toolchain setup. Cargo/Clippy use the installed
Homebrew Rust 1.94; the isolated SPC assembler and Wasm build use
`RUSTC="$(rustup which rustc)"` (rustup stable 1.99). Raw logs and generated
artifacts are ignored under `.cache/` and `web/test-results/`; committed files
contain the reproducible commands and relevant results, not third-party binaries.

| Actual command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed after correcting the two test initializers |
| `cargo test --workspace --locked` | Passed, 229 tests, including all existing DMA/HDMA/VRAM/counter and composition regressions |
| `cargo test --locked -p snes-core --lib oam` | Passed, 13 tests (12 new, one existing) |
| `cargo test --locked -p snes-core --test ppu_oam_access` | Passed, one original CPU guest |
| `python3 tools/make-fixture.py .cache/smoke.bin` and `cmp tests/roms/smoke.bin .cache/smoke.bin` | Passed; fixture SHA-256 `73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb` |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Passed; all three pinned assemblers built |
| `RUSTC="$(rustup which rustc)" python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Passed: cputest-full terminal `$00:81A2`, last `$0649`; basic `$00:81A2`, last `$0452`; spctest `$00:8060`, last `$0557` |
| `RUSTC="$(rustup which rustc)" python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Passed, all 11 grayscale bands and final shade through line 223 |
| `python3 tools/build-cleared-games.py` (twice) | Both passed with pinned hashes below |
| `npm --prefix web ci` | Passed |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Passed, production Wasm/static assets |
| `(cd web && npx playwright install chromium)` | Passed |
| `SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'` | Passed, all 18 browser checks in 2.5 minutes |

Both fresh cleared-game builds retained Castle SHA-256
`197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`
and Memory SHA-256
`6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
Fixture/game source pins, ROM hashes, notices, licensing decisions and all existing
browser assertions/replay inputs remain unchanged. The grep excludes only the
optional externally supplied games; it retains both cleared five-minute replays,
Castle's complete route, Memory's gameplay/audio checks and the UI assertions.

Browser evidence under `web/test-results/` confirms both cleared games ran
18,100 frames (301.172 emulated seconds). Castle retained all 20 replay windows
and 40 jumps/landings; its independent 11,097-frame route traversed maps 0→1→2→3→4
and finished credits. Memory won and restarted 18 times; every gameplay/audio
window passed. Its separate 600-frame score/effect check, real Web Audio gesture
check (424 music buffers, nine effect buffers), pause/reset silence and queue/drift
checks passed. The audio queue check reported zero invalid samples/schedules.
The remaining browser UI, SRAM recovery and Pages checks all passed unchanged.

## Remaining uncertainty and inventory

The rendering address substitute, electrical bytes, phase evolution during
rendering, and data-latch retention across reloads are bounded model policy.
The two references disagree about INIDISP reset qualification; the selected rule
is explicit above; the former H=0 reload convention is superseded by Issue #45.
Exact OAM corruption, physical memory strobes, forced-blank latency and object-fetch timing remain unresolved.
CGRAM access/corruption, VRAM invalid-fetch bits, physical blanking/access
boundaries, external latch inputs, long-dot/field conversion and warm-reset
retention remain unresolved. This does not establish full OAM accuracy, full
hardware accuracy or commercial compatibility.

All 24 categories retain their supplied outstanding scopes and qualifications
from the [baseline inventory](issue-27-validation.md#remaining-uncertainty-and-inventory):
`cpu`, `bus`, `scheduler`, `dma`, `hdma`, `controllers`, `ppu-bg`, `ppu-offset`,
`ppu-mode7`, `ppu-objects`, `ppu-color`, `ppu-ports`, `ppu-display`, `spc`, `ipl`,
`dsp-noise`, `dsp-pitch`, `dsp-envelope`, `dsp-echo`, `dsp-timing` remain partial;
`dsp-voices` remains `implemented_unverified`; `ppu-hires`, `ppu-raster`, `pal`
remain unimplemented. No whole category or historical Issue #3 requirement is
completed. No optional or post-merge verification is required.
