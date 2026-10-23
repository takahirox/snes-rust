# Issue #47: CGRAM data-port ownership

For [Issue #47](https://github.com/takahirox/snes-rust/issues/47), evaluation
baseline `bd0eecab0adf064c3db4f70289bedd781eed1996`. Original code/data are MIT;
no commercial ROM, firmware or external emulator implementation is used.

## Specification reconciliation (recorded before production changes)

Retrieved both authorities directly on 2026-10-24; ignored complete raw copies
are in `.cache/references/`. These match the Issue #45 reference hashes.

| Authority | Actual version / SHA-256 | Sections (one-based raw lines) |
| --- | --- | --- |
| [Fullsnes](https://problemkaputt.de/fullsnes.htm) | no$sns v1.5 / `3853b65a7331bf12eea431ff494a2948a1c78ef22f7e819db4b139fd5a79f6c6` | CGRAM Access, 633–653; INIDISP, 973–989; Timing H/V Events, 26986–27022; Timing PPU Memory Accesses / PPU Palette Load, 27083–27119 |
| [Anomie's Register Doc](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt) | revision 1157, 2007-07-12 / `19dfdf3f1c206655b8ede11bcd6dc053e36634f876819e40bb39cfb88a237dfd` | flag legend / read buses, 18–44; INIDISP, 48–76; OAM low-table write/read analogy, 119–140 and 695–703; CGADD/CGDATA, 441–459; CGDATAREAD, 727–736 |

Fullsnes explicitly gives one first/second selector for both data ports, reset
by CGADD, increment after every read/write, staged even writes, paired odd
commits, and seven stored high bits with the eighth read bit from PPU2. Anomie
refers both CGRAM data ports to low-table OAM behavior. That analogy corroborates
shared read/write progression and reads not replacing the pending write latch;
it is not evidence for OAM's high-table substitute or its VBlank reload in CGRAM.
Neither source specifies clearing the data latch on CGADD. Retention across
readdressing is retained model policy, with mixed operations tested explicitly.

There is a real timing conflict: Fullsnes's general video-memory and INIDISP
summaries say forced/VBlank only; its specific PPU Palette Load paragraph admits
some HBlank access but expressly leaves slots and fetch load unknown. Anomie's
CGADD/CGDATA flags grant HBlank, while CGDATAREAD marks HBlank `?`. Thus the
summaries cannot justify copying VRAM's HBlank ban. The low-OAM analogy indicates
that rendering may change the accessed address, but neither authority supplies
a CGRAM rendering address or an exact active/blanking arbitration window.
Rendering-owned access during stable display is the bounded correction derived
from those access qualifications and the addressing analogy. Exact corruption
values are not a hardware oracle. Read HBlank availability and scanline-0
ownership are unresolved by these sources and need explicit model conventions.

### Operation/state table fixed before implementation

`C` is the nine-bit software byte cursor; its low bit is the shared selector.
`L` is the pending low write byte. `D` is the effective even memory destination.

| Operation/state | Supported semantics / derivation | Explicit approximation or unresolved part |
| --- | --- | --- |
| `$2121 = n` | Set `C=2*n`, first/low phase for both data ports. No memory commit. | Keep `L`; no documented latch reset. Address assignment itself is accepted in every state; no invented suppression. |
| First `$2122 = x` (`C` even) | Capture `L=x`; no memory commit; advance `C` by one byte. | Retain this latch/phase action also under rendering ownership; exact contested-access retention is unverified. |
| Second `$2122 = x` (`C` odd) | Commit `L` and `x & $7F` as a pair; increment `C` modulo 512, including `$1FF→$000`. | Accessible destination `D=C-1`; rendering substitute below. |
| `$213B`, low phase | Read low byte at effective destination, advance shared `C`; do not replace `L`. | Restricted address/value and latch retention use policy below. |
| `$213B`, high phase | Read seven high bits, OR retained PPU2 bit 7, advance shared `C` with wrap; do not replace `L`. PPU2 bus receives returned byte; CPU/PPU1 buses do not supply bit 7. | Same restricted-access policy; no fixed read value. |
| Forced blank (any H/V) | Software address and ordinary sequencing. | Immediate INIDISP effects retained; physical latency unresolved in Fullsnes. |
| Ordinary VBlank (`V>=225` in fixed NTSC model) | Software address and ordinary sequencing. | Availability at H=0 retained scheduler convention; no CGRAM automatic reload. |
| Stable visible display, forced blank clear | Access is subject to rendering ownership rather than unconditionally using software address. Do not suppress every write or substitute CPU open bus. | Without a pixel-fetch pipeline use **color 0**, keeping byte phase; both bytes of a commit go there. This is a deterministic backdrop-address substitute, not a claimed physical address. |
| HBlank | Preserve legal software-addressed palette writes per Anomie's CGDATA flags. | Use entire existing HBlank interval `H>=1096` for both data ports, including reads; Fullsnes warns the physical window is uncertain. |
| Scanline 0 | Outside ordinary VBlank; no CGRAM-specific fetch rule established. | Apply same active/HBlank split as visible lines: rendering substitute before 1096, software after. No assertion that OAM's line-0 rule establishes CGRAM's. |
| Recovery / crossings | Software cursor continues byte progression; forced blank, VBlank or HBlank selects it again. | Preserve software `C` and `L` across ownership transitions; rendering does not overwrite `C`. Physical retention/arbitration unresolved. |

The horizontal split includes the left border in the rendering approximation.
It deliberately claims no precise hardware start/end dot. Tests before/at/after
1096 and line/frame edges check this convention, not measured access strobes.
CPU accesses occur at completion; DMA sources sample at +4, destinations commit
at +8. Coincident events precede access. These existing scheduler conventions
remain unchanged. PAL, overscan/interlace, raster effects and pixel fetches are
outside scope. No inventory category is promoted by this bounded correction.

## Implementation and original numerical coverage

A single effective-byte-address function in the shared PPU path selects the
software cursor or the color-0 substitute. Only the paired write destination
and read source change. CGADD, low-byte staging, shared progression, latch
retention, seven-bit storage, PPU2 read-bus propagation and modulo-512 wrap stay
in their existing paths. CPU, DMA and HDMA need no separate port decoder or
scheduler changes. The helper is evaluated at high-write commitment or read
sampling, so a pair crossing ownership changes uses the second write's state.

`src/ppu_cgram_tests.rs` uses literal, independently specified numeric vectors:

- Seed CPU argument `$03`, PPU1 `$51`, PPU2 `$E2`, palette `$92,$34` at color 4.
  Write-low `$65` leaves memory intact and selects high; read-high returns `$B4`,
  retaining `$65`. Read-low of color 5 returns `$56`, clearing PPU2 bit 7; a
  high-write `$FE` commits `$65,$7E` there. Independently reseeding PPU1 `$F1`,
  PPU2 `$02`, CPU `$80` makes high read return `$7F`, never `$FF` from CPU/PPU1.
- Stage `$D3`, readdress to `$FF` (selector reset, latch retained by policy),
  read-low `$AB`, then high-write `$FF`: stored bytes `$D3,$7F`, cursor wraps to
  zero. Write pair `$21,$87` stores `$21,$07` at color 0. Readback of color `$FF`
  returns `$D3,$FF` and also wraps. Reads never refresh the pending write latch.
- Distinct color-0 `$A1,$32` and color-4 `$12,$34` sentinels distinguish both
  read phases, staged first writes and paired second writes in forced blank,
  VBlank, active display, HBlank and scanline 0. Restricted numeric results
  assert the documented substitute, never an unknown physical corruption byte.

`src/machine/ppu_cgram_tests.rs` advances the natural scheduler solely with timed
CPU accesses/idle; it never assigns raster state or invokes transfer helpers.
It covers stable states and recovery, both phases of CGADD/CGDATA/CGDATAREAD
before/at/after these explicitly modeled boundaries (offsets −2/0/+2 clocks):
V=100/H=1096, V=101/H=0, V=225/H=0, next frame V=0/H=0. The VBlank-entry control
also confirms access remains software-addressed from preceding HBlank; no
invented CGRAM reload is introduced. A pair spanning active→HBlank or
HBlank→active commits both bytes at the high write's destination.

Repeated-port forward/reverse DMA checks three bytes, distinguishing a complete
pair from a staged third byte, software cursor/latch effects, payloads, eight-clock
bytes, A1T `$0400→$0403`, DAS zero, retained bank and pending/enable clearing.
One-byte odd-phase transfers sample/commit before/at/after all four boundaries;
reverse source values survive a subsequent ownership transition before A-bus
commitment. Direct and indirect HDMA in both directions run two rows naturally
from next frame's scanline-0 initialization, then terminate with a zero header.
Assertions cover legal HBlank payloads, paired commits/seven-bit masking, no
color-0 corruption, shared byte phase and latch, table `$0303→$0306` (direct)
or `$0303→$0305` (indirect), indirect pointer `$0402` then terminating zero,
unchanged A1T/banks, line counter `$81→$00`, inactive channel and retained `$420C`.
A third event proves termination persists. Forced-blank controls agree.
Expected values never call the new address helper.

### Executed original LoROM guest

`tests/ppu_cgram_access.rs` constructs original MIT instructions and palette data
in a 32 KiB in-memory LoROM. Executed CPU instructions initialize every sentinel,
program/acknowledge H/V IRQs and WAI, poll HVBJOY, perform all port accesses and
publish eighteen bytes at `$1800..$1811`, completion `$18F0=$A5`, then STP.
The host supplies the cartridge and observes results/deadlines only: no raster,
phase, latch, palette or expected result is injected. It completes in **34,660
instructions, one frame wrap**, below 150,000 instructions and four wraps.
Source/data provenance is recorded in `tests/roms/PROVENANCE.md`.

The visible access occurs after a masked V=100/H=64 IRQ. The HBlank pair follows
V=120/H=278. Ordinary VBlank and scanline 0 use entering/leaving HVBJOY polling.
These provide stable windows under existing IRQ/bus conventions, not physical
boundary measurements. The unit scheduler vectors separately test exact modeled
edges. Guest results distinguish supported blanked/HBlank controls from
restricted-address and retention approximation checks.

## Baseline counterexamples

New vectors were run before changing production behavior, against this node's
upstream checkout (production matches evaluation baseline above):

| Command | Baseline result |
| --- | --- |
| `cargo test --locked -p snes-core cgram_visible_ownership -- --nocapture` | Failed, exit 101: visible low read actual decimal 18 (`$12`), policy expected decimal 161 (`$A1`) |
| `cargo test --locked -p snes-core --test ppu_cgram_access` | Failed, exit 101: full guest result differs in visible read and recovered palette destinations |

Literal guest before/after results, in hex:

| WRAM | Meaning | Baseline | Corrected |
| --- | --- | --- | --- |
| `$1800..01` | Forced-blank color 4 read | `12 34` | `12 34` |
| `$1802..03` | Stable visible read with software color 4 | `12 34` | `A1 B2` (substitute policy) |
| `$1804..05` | Forced-blank color 4 after visible pair `$65,$FE` | `65 7E` | `12 34` |
| `$1806..07` | Forced-blank color 0 after that pair | `A1 B2` | `65 7E` (substitute policy) |
| `$1808..09` | Color 5 after HBlank pair `$DE,$AD` | `DE AD` | `DE AD` |
| `$180A..0B` | Ordinary VBlank color 4 | `65 7E` | `12 34` |
| `$180C..0D` | Scanline-0 read, software color 4 | `65 7E` | `65 7E` (policy; earlier writes make this observation equal) |
| `$180E..0F` | Forced-blank recovery at progressed color 5 | `DE AD` | `DE AD` |
| `$1810..11` | Pending `$D3`, readdress/read-low/write-high `$FF` | `D3 FF` | `D3 FF` |

The decisive write counterexample is the preserved software-selected color 4;
color 0 and its restricted returned bytes establish deterministic emulator
policy only. High read bits `$B2/$AD/$FF` reflect PPU2 retention, not stored
CGRAM bit 15. Both formerly failing commands now pass. The existing baseline
suite was not separately rerun before production changes; only the two new
counterexamples were run there. Existing assertions and game fixtures remain
unchanged.

## Mandatory validation

Run on 2026-10-24 (local date). Cargo/Clippy use installed Homebrew Rust 1.94;
SPC assembler and Wasm builds select installed rustup Rust 1.99 as documented
in README. Full ignored logs are `.cache/issue-47-*.log`; source-built reports
are `.cache/hardware-tests/results.json`, `.cache/ppu-test/results.json` and
`.cache/cleared-games/results.json`. Browser attachments stay ignored under
`web/test-results/`. No source pins, hashes, replay inputs, durations or existing
assertions are modified.

| Actual command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --workspace --locked` | Passed, 243 tests; includes existing CPU, SPC, DSP, CGRAM, VRAM, OAM/reload, counter-latch, rendering, DMA and HDMA regressions |
| `cargo test --locked -p snes-core --lib cgram` | Passed; 8 tests at initial focused run, subsequent pair-crossing test also passed separately and in workspace |
| `cargo test --locked -p snes-core --test ppu_cgram_access` | Passed, original executed guest |
| `python3 tools/make-fixture.py .cache/smoke.bin` | Passed, SHA-256 `73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb` |
| `cmp tests/roms/smoke.bin .cache/smoke.bin` | Passed, byte-identical |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Passed, all three pinned assemblers including SPC |
| `RUSTC="$(rustup which rustc)" python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Passed, all 3 source-built suites: full CPU terminal `$00:81A2` / last `$0649`, basic CPU `$00:81A2` / `$0452`, SPC `$00:8060` / `$0557` |
| `RUSTC="$(rustup which rustc)" python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Passed, eleven grayscale bands; final shade through line 223 |
| `python3 tools/build-cleared-games.py` (twice, sequential fresh builds) | Passed, both pinned game hashes on both runs |
| `npm --prefix web ci` | Passed |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Passed, production Wasm/static assets |
| `npm --prefix web exec -- playwright install chromium` | Passed |
| `SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'` | Passed, all 18 browser checks in 2.6 minutes using documented alternate port |
| `git diff --check` and `git diff --cached --check` | Passed, including new files |

Both fresh builds retained Castle SHA-256
`197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`
and Memory SHA-256
`6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
The licensed eleven-band PPU fixture retains source
`ac6ef8006809c0ff5dabc8ff137a4623967697fe` and ROM SHA-256
`aef339e5d4a7319db31de577b253d977b45f5c45f09adfaa99a302ca300a957a`.
These selected passes do not establish every CGRAM access boundary.

Browser evidence records 18,100 frames (301.172 emulated seconds) and all 20
windows per game. Castle's separate 11,097-frame route traversed five maps,
recorded 20 checkpoints and finished credits. Memory won/restarted 18 times;
its 600-frame audio check recovered 40 notes and an independent finite card
effect. Gesture-activated Web Audio observed 424 music buffers and nine effect
buffers, with the effect ending while music continued and pause/reset silence.
Queue/drift/stall/repeat playback checks recorded zero invalid samples/schedules
and at most eight active sources. UI, save recovery and subpath Pages smoke
checks also passed. The grep excludes only optional external games; cleared-game
five-minute replays, the full route and all other mandatory checks remained.
No mandatory gate failed on the final implementation. The two intentional new
baseline counterexample failures above are the only failures in this run.

## Remaining uncertainty and inventory

`ppu-ports` remains partial. Exact rendering-owned corruption addresses/bytes,
unresolved contested-access and readdressing phase/latch retention, physical
access/blanking latency, scanline-0 CGRAM fetch behavior, exact HBlank arbitration
and dot-level pixel fetches remain outstanding. Entire-HBlank access and the
color-0 substitute are deterministic policy. OAM corruption and coincident reload
strobes, VRAM invalid-fetch bits, physical reset retention and external counter
inputs retain their earlier limits. PAL, overscan/interlace and unrelated timing
changes are outside this Issue. No commercial compatibility claim follows.

All 24 inventory categories keep the status and outstanding scope in the
[baseline inventory](issue-27-validation.md#remaining-uncertainty-and-inventory):
`cpu`, `bus`, `scheduler`, `dma`, `hdma`, `controllers`, `ppu-bg`, `ppu-offset`,
`ppu-mode7`, `ppu-objects`, `ppu-color`, `ppu-ports`, `ppu-display`, `spc`, `ipl`,
`dsp-noise`, `dsp-pitch`, `dsp-envelope`, `dsp-echo`, `dsp-timing` remain partial;
`dsp-voices` remains `implemented_unverified`; `ppu-hires`, `ppu-raster`, `pal`
remain unimplemented. No inventory category or historical Issue #3 requirement
is completed. No optional external diagnostic/game was acquired or run. No
post-merge verification is required.
