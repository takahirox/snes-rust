# Issue #105: provisional late H-only IRQ policy

## Decision recorded before runtime changes (2026-10-25)

**PROVISIONAL / HARDWARE UNVERIFIED.** The current
[Issue #105](https://github.com/takahirox/snes-rust/issues/105) explicitly selects
a deterministic software policy and defers resolution of the physical endpoint
rule. This supersedes the earlier Issue text requiring a hardware-backed table,
and the H-only late fallback expectations in
`horizontal_irq_zero_delay_and_approximate_late_controls`. Combined H/V late
controls remain an explicitly unverified policy.

Documented facts, recorded in the [#99](issue-99-validation.md),
[#103](issue-103-validation.md) and [geometry](issue-95-validation.md) source
reconciliations: Fullsnes's detailed H/V Events uses `4*HTIME+14` master clocks
for supported same-line H-only assertions. H-only ignores VTIME. Ordinary NTSC
non-interlaced lines have 1364 clocks; odd-field V=240 has 1360. Fields contain
262 lines; STAT78 changes separately at V=0/H=4. Register summaries do not
establish an exact late comparator/delivery rule. The
[research comment](https://github.com/takahirox/snes-rust/issues/105#issuecomment-6392266037)
explicitly leaves eligibility, last-dot sampling, suppression and boundary
ownership unresolved. These records are context, not new hardware measurements.

Arithmetic alone gives candidate offsets 1358/1362/1366/1370/1374/1378/2058.
The **selected software policy** emits only when the offset is strictly inside
the current line; otherwise it suppresses, with no undelayed fallback and no
cross-line or cross-field carry. It does not claim these candidates establish
physical comparator eligibility. Each new line evaluates independently:

| HTIME | Ordinary line (1364 clocks) | Odd-field V=240 (1360 clocks) |
| ---: | --- | --- |
| 336 | H=1358, same line/field | H=1358, same line/field |
| 337 | H=1362, same line/field | Suppress |
| 338 | Suppress | Suppress |
| 339 | Suppress | Suppress |
| 340 | Suppress | Suppress |
| 341 | Suppress | Suppress |
| 511 | Suppress | Suppress |

Thus V=239/240/241 and V=261→0 receive no carried event. An already pending
IRQ remains pending across rollover until acknowledged. HTIME=0, startup,
V-only, combined mode and existing CPU/DMA service conventions are preserved.
Stable timer configuration is the tested domain; live writes are excluded.

The rationale is to remove the inconsistent undelayed fallback with a small,
reviewable policy while evidence is unavailable. The risk is disagreement with
hardware on endpoint eligibility or delayed delivery. Future corroboration
should run an original licensed diagnostic on physical NTSC SNES hardware,
record TIMEUP observations for 336–341/511 on both line geometries and both
field boundaries, account for CPU access phases, and compare independent
emulators to identify discriminating cases. Revise this policy in a focused
follow-up when reliable evidence arrives. Passing software regressions cannot
resolve those hardware questions.

## Implementation and validation

`SystemBus::tick` always retains the delayed offset for H-only mode; its
existing strict line-end guard suppresses out-of-line events. It introduces no
new scheduler state. Combined mode keeps its previous fallback. The older
horizontal zero/late control now expects H-only suppression while retaining
combined late expectations unchanged.

Six original unit regressions and one integration test (two source-built guest
variants) cover the policy:

- Fresh buses visit natural V=0/1/239/240/241/261 in fields 1 and 2, with
  HTIME=336/337/338/339/340/341/511 and VTIME=0/240/262/511. Literal oracles
  check before/at/after eligible events, former fallback clocks, rollover,
  next H=0..15, acknowledgement, and the next line's independently owned
  event. Earlier pending flags clear at target start minus one, before the
  comparison. Early reads cannot consume a future event. Raster and reported
  field parity are checked after natural transitions.
- Pending retention spans an eligible V=239 event, suppressed short V=240,
  subsequent eligible V=241 and another field; acknowledgement clears once.
- One-clock, irregular 1/7/1361/2/43/8193/5 and whole observation intervals
  visit all 1048 lines of four natural fields for HTIME=337 and 338. Each
  line checks the pre-event/old-fallback interval, eligible event, following
  clock and rollover. Literal field ends are 357368/714732/1072100/1429464.
  Uninterrupted runs also cross all four fields without intermediate reads,
  including one whole multi-field tick; only 337 retains a pending IRQ.
  Controller results are [$A590,$5A60,0,0], both serial registers are $FFFF,
  automatic polling is complete, and retained HDMA masks, counters, transfer
  flags and service requests are checked. At field 4/V=0/H=0 STAT78 still
  reports the previous parity; after H=4 it changes. Independent stereo
  counts at clocks 1429464 and 1430001 are twice the floor of elapsed clocks
  times 32000/21477272. PCM equality establishes partition determinism only.
- Timed six-clock CPU TIMEUP reads cover eligible offsets, former fallbacks
  and line/field rollover, including reads that acknowledge early and cannot
  consume later delivery. Access-end sampling and immediate read clearing are
  emulator conventions, not physical coincident-read evidence.
- Real fixed-A TIMEUP→WMDATA DMA reads straddle ordinary/short-line and field
  boundaries. Existing phases are 16 setup clocks, reads at +20/+28/+36 and
  writes at +24/+32/+40. Literal destination bytes, exhausted counts, fixed
  source, WMDATA pointer, enable/pending masks, clocks/raster and later quiet
  interval are checked. Suppressed candidates produce three zero bytes.
- A 100-byte WRAM→VRAM DMA starts at 686066 (odd V=240/H=1338), before the
  old fallback. With 16 setup, 800 payload and one 40-clock refresh hold it
  ends at 686922/V=241/H=834. HTIME=336 retains an IRQ; 337/338/339 remain
  suppressed. PC, stack, instruction count and waiting state remain unchanged
  during the transfer; all payload bytes, DMA completion and independently
  calculated audio count are checked. Only the eligible case services IRQ
  after CPU resumption. These DMA/service phases are existing conventions.
- The original MIT guest variants keep HTIME=337 or 338, VTIME=511 and H-only
  enable stable across four natural fields. The 337 guest distinguishes short
  line suppression through WAI/counter observations; 338 progresses solely
  through counter polling and rejects any pending IRQ. Every literal record
  byte is compared in guest code with executed pass/fail markers and bounded
  deadlines. Host code only builds, loads, steps and observes. Authorship,
  construction and independent geometry/CPU arithmetic are recorded in
  [PROVENANCE](../tests/roms/PROVENANCE.md#original-late-horizontal-irq-diagnostics-issue-105).

## Exact-baseline counterexamples

Exported exact evaluation baseline `8b5e3a7b3d6960375250e92804924c41614ff2b8`.
Only the two new test sources and necessary unit module registration were
transferred, leaving runtime and older tests untouched:

```sh
mkdir -p .cache/issue105/baseline
git archive 8b5e3a7b3d6960375250e92804924c41614ff2b8 | tar -x -C .cache/issue105/baseline
cp crates/snes-core/src/machine/late_horizontal_irq_tests.rs .cache/issue105/baseline/crates/snes-core/src/machine/
cp crates/snes-core/tests/late_horizontal_irq.rs .cache/issue105/baseline/crates/snes-core/tests/
cat >> .cache/issue105/baseline/crates/snes-core/src/machine.rs <<'RS'

#[cfg(test)]
#[path = "machine/late_horizontal_irq_tests.rs"]
mod late_horizontal_irq_tests;
RS
cargo test --manifest-path .cache/issue105/baseline/Cargo.toml --locked -p snes-core --lib late_horizontal_irq_fresh_policy_matrix_and_no_rollover_carry --no-run
cargo test --manifest-path .cache/issue105/baseline/Cargo.toml --locked -p snes-core --test late_horizontal_irq --no-run
cargo test --manifest-path .cache/issue105/baseline/Cargo.toml --locked -p snes-core --lib late_horizontal_irq_fresh_policy_matrix_and_no_rollover_carry
cargo test --manifest-path .cache/issue105/baseline/Cargo.toml --locked -p snes-core --test late_horizontal_irq
```

Both `--no-run` builds succeed (exit 0). Both behavioral commands exit 101:

- The fresh matrix fails for HTIME=338, field=1, V=0, VTIME=0, probe=1351.
  Advancing from that early quiet observation through rollover observes the
  baseline's undelayed H=1352 assertion, violating suppression/no carry.
- The guest executes failure=$EE, success=$00. Its 337 records contain
  V=241/241/241/241; the literal policy oracle requires 241/242/241/242.
  The other record bytes match, including parity and pending/acknowledged IRQ.
  This failure occurs in executed guest comparisons after all four fields.

The corrected implementation passes both: six focused units and both guest
variants complete, with failure=$00 and success=$A5. These baseline failures
show a software-policy difference, not a newly measured hardware rule.

## Executed acceptance gates (2026-10-25)

| Command | Actual result |
| --- | --- |
| `cargo test --locked -p snes-core late_horizontal_irq` | Pass: 6 units + 1 integration test (two guest variants) |
| `cargo test --locked -p snes-core horizontal_irq` | Pass: 13 units + 2 integration tests, including the new late tests |
| `cargo test --locked -p snes-core horizontal_zero_irq` | Pass: 7 units + 1 guest |
| `cargo test --locked -p snes-core combined_zero_irq` | Pass: 7 units + 1 guest |
| `cargo test --locked -p snes-core v_only_irq` | Pass: 7 units + 1 guest |
| `cargo test --locked -p snes-core --test ntsc_timing` | Pass: four-field geometry guest |
| `cargo test --locked -p snes-core --lib machine::ntsc_timing_tests` | Pass: 8 units |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Pass |
| `cargo test --workspace --locked` | Pass: 509 tests, 0 failed, 0 ignored |
| `python3 tools/make-fixture.py .cache/smoke.bin` and `cmp tests/roms/smoke.bin .cache/smoke.bin` | Pass: byte-identical smoke fixture |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Pass: all three pinned assemblers |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Pass: full/basic CPU and SPC suites |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Pass: all 11 HDMA grayscale bands |
| `python3 tools/build-cleared-games.py` (twice) | Pass: both pinned game hashes match in both builds |
| `npm --prefix web ci` | Pass |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Pass: fresh production Wasm/browser output |
| `npm --prefix web exec playwright install chromium` | Pass |
| `SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'` | Pass: all 37 checks, 2.9 minutes |

Castle and Memory gameplay replays each execute 18100 fields (301.170679
emulated seconds). Castle's fixed controller route reaches all five maps and
credits in 11097 fields; Memory completes 18 wins. Music, independent finite
effects, repeat playback, gesture-activated Web Audio and Pages artifact checks
all pass. The documented compiler selection and port override were used from
the start. No acceptance gate failed or was waived; the intentional baseline
behavioral failures above are the only test failures in this work.

Full/basic CPU suites end at $0081A2 with last tests $0649/$0452; SPC ends at
$008060 with $0557. HDMA ROM SHA-256 remains
`aef339e5d4a7319db31de577b253d977b45f5c45f09adfaa99a302ca300a957a`.
Smoke SHA-256 is
`6e5518b3959883cc22e914621fc5b5d9f5c0fe4eb96ee61000769afdbb68d6e5`.
Cleared-game SHA-256 remains Castle
`197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`
and Memory
`6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
No existing fixture/game source pin, hash, replay input, gameplay/progression/
music assertion, deadline or tolerance was changed. No commercial ROM or
firmware was acquired. New diagnostics use original MIT sources; external
fixtures retain their existing explicit licensing/provenance decisions.

Local raw logs are under `.cache/issue105/`, source-built hardware results under
`.cache/hardware-tests/results.json` and `.cache/ppu-test/results.json`, game
hashes under `.cache/cleared-games/results.json`, browser artifacts under
`web/test-results/`. These ignored outputs are not checkpoint files; this record
preserves actual commands, baseline counterexamples and gate results.

## Remaining uncertainty and exclusions

The policy is explicitly provisional; physical late comparator eligibility,
last-dot sampling, suppression versus delayed assertion and line/field ownership
remain unverified. Combined late IRQ/VTIME ownership, live timer/mode writes,
TIMEUP write clearing and coincident reads, startup/reset alignment, NMI delays,
refresh revisions, physical CPU recognition, DMA alignment, PAL/interlace/
overscan and raster rendering remain outside scope. Full device synchronization,
including APU-before-raster advancement, remains incomplete. All CPU/WAI, zero/
nonzero IRQ, NMI, geometry, controller, DMA/HDMA, PPU-port and audio regressions
remain required.

Inventory scopes remain intact: `cpu`, `bus`, `scheduler`, `dma`, `hdma`,
`controllers`, `ppu-bg`, `ppu-offset`, `ppu-hires`, `ppu-mode7`, `ppu-objects`,
`ppu-color`, `ppu-ports`, `ppu-display`, `spc`, `ipl`, `dsp-voices`, `dsp-noise`,
`dsp-pitch`, `dsp-envelope`, `dsp-echo` and `dsp-timing` remain partial;
`ppu-raster` and `pal` remain unimplemented. No category becomes fully verified.
This does not complete Issue #3 or establish commercial compatibility. Physical
corroboration and the optional 301-second wall-clock browser soak are not
required and were not performed. No post-merge verification is required.
