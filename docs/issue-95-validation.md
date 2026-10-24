# Issue #95: NTSC non-interlaced beam geometry

## Specification reconciliation (recorded before implementation)

Inspected on 2026-10-24, before changing runtime code:

- [Fullsnes](https://problemkaputt.de/fullsnes.htm), self-identified as extracted
  from no$sns v1.5; downloaded HTML SHA-256
  `3853b65a7331bf12eea431ff494a2948a1c78ef22f7e819db4b139fd5a79f6c6`.
  Sections: **SNES Timing H/V Counters**, **SNES Timing H/V Events** (including
  **PPU H-Counter-Latch Quantities**), and **213Fh - STAT78**.
- [SNESdev Timing, revision 1467](https://snes.nesdev.org/w/index.php?title=Timing&oldid=1467),
  retrieved using its MediaWiki parse API (pageid 50, revid 1467).
  Sections: **Video / Scanline / Short and Long Scanlines**, **Vertical Blank**,
  **Frame**, and **Audio**. These are reference documents, not device traces.

Source statements: both references specify a four-clock reduction on NTSC
non-interlaced V=240, field 1, independent of picture height. Fullsnes STAT78
identifies bit 7 as the current field. Its latch-quantity section identifies
six-clock counter values 323 and 327 and four-clock values around them; the
H/V events distinguish PPU dots from CPU timer units. H counts end at 339.
SNESdev identifies 262 scanlines and an independent audio clock.

Disagreements resolved: both scanline summaries describe four five-clock dots;
Fullsnes's more specific software counter observations instead describe two
six-clock values. This Issue models the **latched counter**, using the latter,
not an external dot-clock waveform. Fullsnes's terse horizontal summary says
"of Interlace" for the short line; its explicit long/short section, detailed
H/V events, and SNESdev agree that interlace must be **off**. Use that condition.
The frame tables are explicitly approximate and do not override the short line.
Fullsnes summarizes field toggling at V=0; its detailed list specifies
`H=1, V=0` for the FIELD flag transition. The summary supplies no conflicting
H coordinate. On review, the original H=0 implementation was corrected to this
specific H=1 source statement. Thus the Issue's "at field rollover" requirement
is refined to H=1 for the reported flag; raster rollover remains H=0.
Re-retrieving Fullsnes on 2026-10-24 produced the same HTML hash above.
Electrical latch edges remain outside scope.

Arithmetic deductions, independent of implementation: 340*4+2*2=1364;
short=340*4=1360; 262*1364=357368; shortened field=357364;
two fields=714732 clocks. Ordinary H=323 occupies [1292,1298), H=327
[1310,1316), H=339 [1360,1364). Short-line counterparts are [1292,1296),
[1308,1312), [1356,1360). These establish the Issue's numeric geometry contract.
Since dot 0 lasts four master clocks, the H=1 FIELD transition means STAT78
retains the preceding bit at V=0/master clocks [0,4) and changes at clock 4.

Emulator conventions: new/reset machines start field 0; although Fullsnes lists
H=0,V=0,F=0 after reset, no physical reset evidence was examined. Warm-reset
retention is outside scope. Field 0 is ordinary, field 1 contains the short
line, including forced blank. Initial V=0 does not synthesize a field transition
at H=1: reset field 0 lasts through the first raster, preserving the Issue's
explicit startup convention. Whole-line rendering, HBlank=1096,
HDMA eligibility=1112, refresh start=538/hold=40 remain existing conventions.
Fullsnes lists IRQ assertion after comparison (HTIME+3.5; V-only H=2.5), and
NMI flag after VBlank begins. Beam conversion does not establish those delays:
`HTIME*4`, H=0 V comparisons, NMI at V=225/H=0 and instruction-boundary
sampling remained limitations of #95. [Issue #97](issue-97-validation.md)
supersedes V-only timing; [Issue #99](issue-99-validation.md) supersedes the
nonzero same-line H/HV approximation. Zero/late cases and NMI delays remain
unresolved. Interlace, overscan, PAL and fetch/arbitration accuracy are not
implemented by these changes.

## Implementation and validation

Runtime scheduler advancement, access/refresh splitting and PPU capture use the
same active-line geometry. Parity lives in the PPU and changes at V=0/H=1
after each field rollover; its V coordinate is synchronized at each line
boundary even in a whole-field tick. APU advancement remains by the actual
elapsed master clocks, with no raster-phase reset. The late PPU conversion is
separate from CPU timer units (now `HTIME*4+14` in #99's bounded domain).

Eight original unit regressions and one original source-built CPU guest are part
of normal workspace execution:

- Every clock from 1288 through rollover on V=239/240/241, both parities, four
  naturally advanced fields, with display enabled and forced blank. Literal
  interval tables establish both long dots, H=339 and rollover; every one of
  the 1048 line totals and four field/cumulative totals is asserted per run.
- Timed six-clock SLHV/WRIO accesses at every clock around both long dots and
  rollover; held values, low gate, repeated low, rising release, independent
  H/V selectors, STAT78 acknowledgement/reset, CPU and PPU2 open bus, NTSC and
  version bits. Existing latch/mirror/DMA controls also pass unchanged.
- Timed six-clock STAT78 reads ending at each clock from two before rollover
  through six after it, across four naturally advanced fields. Both parities
  retain the preceding bit at V=0/master H=0..3 and change at H=4. One-clock
  sampling independently covers all four clocks and reset retains field 0.
- One-clock, irregular and whole-interval advancement across four fields,
  comparing raster/interrupt/HDMA/controller/audio state, then continued audio
  to check retained fractional phase. Literal audio counts use elapsed clocks.
- Timed 6/8/12-clock CPU accesses, DMA payload halves and a complete 100-byte
  DMA crossing the short line. Two complete fields consume 693772 access
  clocks plus exactly 524*40 refresh clocks, in whole and irregular chunks.
  Per-line H=0 IRQ and four VBlank NMIs are acknowledged once, retaining the
  existing approximate event offsets. Older V-only/HV rollover tests now use
  independent alternating field lengths rather than fixed-frame multiplication.
- The CPU guest self-checks four field/counter records before writing success
  and STPing, with bounded host deadlines and no injected timing/result state.
  Authorship, license, byte construction and cycle arithmetic are in
  [PROVENANCE](../tests/roms/PROVENANCE.md#original-ntsc-non-interlaced-timing-guest-issue-95).

Controller opportunities are still on absolute clock 254 modulo 256. VBlank
starts at field_start+306900; round field_start+307030 up to that lattice.
Field starts are 0,357368,714732,1072100,...; relative polling H positions are
298,306,318,326,338,346,358,366,378,130,142,150,162. Thus the phase wraps in
field 9, not field 11. The first aligned polling interval is still 357376;
it is not a field duration. The 4224-clock duration and all existing serial,
interference, busy, completion and reset controls are retained.

Browser pacing uses the 714732-clock pair's average; the independent audio
oracles count `fields*357368 - floor(fields/2)*4`. Reported gameplay seconds
also use alternating field lengths. No drift tolerance, ROM source pin/hash,
input/replay, frame count, route, progression assertion, audio/music assertion
or negative control was changed.

## Exact-baseline behavioral counterexamples

Exported baseline `028ec9f4e519a865e9f3e5f8e28728a7eb4e6224` with
`git archive ... | tar -x -C .cache/issue95/baseline`. Copied only the two new
test files and appended their unit module declaration to baseline machine.rs;
no baseline production behavior was edited. Commands:

```sh
cargo test --manifest-path .cache/issue95/baseline/Cargo.toml --locked -p snes-core --lib ntsc_timing_tests
cargo test --manifest-path .cache/issue95/baseline/Cargo.toml --locked -p snes-core --test ntsc_timing
```

Both compiled and exited 101 on behavioral assertions: all seven unit tests and
the guest failed. Specific independent counterexamples:

| Case | Baseline | Required |
| --- | --- | --- |
| Ordinary V=239, master H=1296 | Latched H=324 | H=323 |
| Timed WRIO on ordinary V=240 at H=1296 | H=324 | H=323 |
| Six-clock read starting field 1 V=240/H=1358 | V=241/H=0 | V=241/H=4 |
| Full DMA at cumulative clock 686922 | V=241/H=830 | V=241/H=834 |
| Four fields plus remainder at clock 1430001 | V=0/H=529 | V=0/H=537 |
| IRQ at shortened-line rollover | No flag yet | One acknowledged flag |
| Guest success marker | $00 (self-check STP) | $A5 |

## Executed gates (2026-10-24)

Commands below run from the repository root. Local detailed logs are in ignored
`.cache/issue95`; this record preserves the durable results.

| Command | Actual result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Pass |
| `cargo test --workspace --locked` | Pass: 471 tests, including 201 core unit tests and the new CPU guest; no failures/ignored tests |
| `cargo test --locked -p snes-core --lib ntsc_timing_tests` | Pass: all seven new unit regressions |
| `python3 tools/make-fixture.py .cache/issue95/smoke.bin` and `cmp tests/roms/smoke.bin .cache/issue95/smoke.bin` | Pass; SHA-256 `6e5518b3959883cc22e914621fc5b5d9f5c0fe4eb96ee61000769afdbb68d6e5` |
| `python3 tools/build-test-tools.py --spc` | Initial Homebrew Rust 1.94 invocation failed on the SPC assembler's experimental syntax; cc65 and bass built |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Pass using the README compiler override; pinned cc65, bass and spcasm builds |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Pass: full/basic CPU suites (terminal $0081A2, last cases $0649/$0452) and SPC suite (terminal $008060, last case $0557) |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Pass: 11 grayscale bands of 16 lines, final shade through line 223 |
| `python3 tools/build-cleared-games.py` twice | Both pass; identical pinned ROM hashes on both builds |
| `npm --prefix web ci` and `npx --prefix web playwright install chromium` | Pass |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Pass: fresh release Wasm and web artifact; repeated after final browser timing edits |
| `npm --prefix web test -- --grep-invert '301 seconds'` | Default port 8080 occupied; no tests ran on that invocation |
| `SNES_TEST_PORT=18095 npm --prefix web test -- --grep-invert '301 seconds'` | Pass: 37 tests in the initial run (3.0m), and 37 after final shared replay/progression timing corrections (2.8m); mandatory cleared gameplay, progression, audio, queue/drift, reset and persistence checks retained |

The final corrected-cadence browser replay records 18100 frames / 301.1706794
emulated seconds for each game. Castle's maximum cumulative audio drift is
1.42055 samples, Memory's 1.41970 samples (existing limit 128 samples). Memory
completes 18 wins. Castle's unchanged 11097-frame route completes all five maps
and credits, final state 3, scroll 287 and hash `97663f45`. These are observed
results under existing assertions, not replacement golden gameplay values.

Pinned source-built fixture details and licenses remain in the existing
provenance records. Castle ROM SHA-256 remains
`197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`;
Memory remains `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
Hardware-test source pin remains `5ecdf555da920f0bd7b157542141965a8120186d`;
PPU-test source pin remains `ac6ef8006809c0ff5dabc8ff137a4623967697fe`.

## Review correction: FIELD reporting edge (2026-10-24)

The original H=0 parity assertion is superseded by the source reconciliation
above. Before changing runtime behavior, the new
`timed_stat78_reads_across_field_reporting_edge_in_both_parities` regression
compiled against the reviewed implementation and failed behaviorally: its
six-clock read ending at clock 357368 returned `$83` instead of `$03`.
The corrected scheduler changes FIELD when advancement crosses master clock 4
of V=0 after rollover. It handles accesses spanning rollover and whole-line or
whole-field ticks without adding an independent reported-parity state. V=0 has
ordinary line geometry with either bit, so field totals and V=240 geometry are
unchanged. Initial/reset parity remains the documented emulator convention.

Commands were rerun in the review worktree; detailed logs are under ignored
`.cache/issue95-review`:

| Command | Actual result |
| --- | --- |
| `cargo test --locked -p snes-core --lib ntsc_timing_tests` | Pass: eight regressions, including all 36 timed STAT78 edge samples |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Pass |
| `cargo test --workspace --locked` | Pass: 472 tests, zero failures/ignored tests; original timing guest and existing controller, interrupt, DMA/HDMA, latch, PPU-port and APU regressions retained |
| `python3 tools/make-fixture.py .cache/issue95-review/smoke.bin` and `cmp tests/roms/smoke.bin .cache/issue95-review/smoke.bin` | Pass: unchanged SHA-256 recorded above |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Pass: all three pinned assemblers rebuilt |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Pass: CPU full/basic and SPC suites |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Pass: all 11 grayscale bands |
| `python3 tools/build-cleared-games.py` twice | Both pass: Castle/Memory hashes identical to the pinned values above |
| `npm --prefix web ci` and `npx --prefix web playwright install chromium` | Pass |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Pass: fresh Wasm/web build |
| `SNES_TEST_PORT=18095 npm --prefix web test -- --grep-invert '301 seconds'` | Pass: 37 tests in 2.9m, retaining cleared gameplay/progression, audio, queue/drift, reset and persistence checks |

## Remaining limits and inventory

No physical-device traces were available or required. These bounded checks do
not establish full hardware accuracy or commercial compatibility, and do not
complete Issue #3. No post-merge verification is required for this Issue.

Precise IRQ/NMI comparison/assertion/sampling offsets, late HTIME behavior,
revision-dependent refresh/startup alignment, complete device synchronization,
interlace/overscan/PAL geometry, pixel fetching and physical arbitration remain
unresolved. The reset policy is qualified above. Existing early HBlank/HDMA
and rendering conventions remain unchanged.

All 24 inventory scopes are retained: `cpu`, `bus`, `scheduler`, `dma`, `hdma`,
`controllers`, `ppu-bg`, `ppu-offset`, `ppu-hires`, `ppu-mode7`, `ppu-objects`,
`ppu-color`, `ppu-ports`, `ppu-display`, `spc`, `ipl`, `dsp-voices`, `dsp-noise`,
`dsp-pitch`, `dsp-envelope`, `dsp-echo`, `dsp-timing` remain partial;
`ppu-raster` and `pal` remain unimplemented. None becomes fully verified.
