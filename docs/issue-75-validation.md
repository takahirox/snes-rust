# Issue #75 sample-level hidden envelope validation

Implemented on 2026-10-24 for [Issue #75](https://github.com/takahirox/snes-rust/issues/75),
evaluation baseline `d39804fa5ac6fb528b6518cfae0008c37a3f08e4`.
`dsp-envelope` remains partial. No inventory category is promoted to verified.

## Specification reconciliation

Consulted on 2026-10-24:

- [Fullsnes, SNES APU DSP ADSR/Gain Envelope](https://problemkaputt.de/fullsnes.htm#snesapudspadsrgainenvelope),
  currently served text (no revision identifier supplied), VxADSR1/2, VxGAIN,
  Gain Notes, Misc Notes, and Control Registers KON/KOFF/FLG. It gives the
  arithmetic/rates, phase tracking during GAIN, five empty KON samples and
  unconditional direct GAIN/release. Its Misc Notes describe an 11-bit clipped
  hidden result, including negative results comparing above `$600`. Its
  `$7e0` attack and `<=` decay descriptions conflict with the more detailed
  reference below.
- [Anomie's S-DSP Doc, jwdonal revision 1212](https://gist.github.com/nyanpasu64/a0d916ce6924912a7116682bf778e9a0#file-apudsp_jwdonal-txt),
  dated 2015-09-28, VxADSR1/VxADSR2/VxGAIN: every sample computes a new value;
  only commitment is rate-controlled. Save the pre-clamp result for the next
  bent increment. Phase tracking also occurs in GAIN: decay matches the
  candidate's upper bits against the selected register's upper bits; attack
  overflow, including negative values, enters decay. SOUND GENERATION separates
  envelope application from update; COUNTERS specifies the shared countdown.
- To resolve clipping/signedness, also consulted Shay Green/blargg's original
  research implementation, identified as **snes_spc 0.9.0, copyright 2007**,
  [SPC_DSP.cpp, run_envelope and V3c](https://github.com/snes9xgit/snes9x/blob/8028d3b6ca994f21fdff220d513a08382e37afc3/apu/bapu/dsp/SPC_DSP.cpp#L551)
  (research code mirrored in Snes9x; file SHA-256
  `881884fb0cd59b8a17cd78c736686f94d22076178bc72ee897702d3fe7a599ef`).
  [The author's account](https://www.slack.net/~ant/libs/audio.html#snes_spc)
  identifies original device research. This corroborates pre-clamp storage,
  an unsigned bend comparison without an 11-bit mask, phase evaluation before
  the rate gate, release retaining the latch, and zeroing both envelope values
  during KON delay. This LGPL research code was read for facts; no source,
  executable, output trace or test data was imported as an oracle.
  The author's linked archive returned 404; the linked mirror was accessible.

Resolution: use revision 1212's pre-clamp rule, with the unsigned comparison
clarified by the primary research implementation. Fullsnes' 11-bit clipping would
turn candidate `$800` into zero and choose the opposite bend branch; that wording
is not used. Neither reference's simplified single-`E` bent formula alone
specifies the preceding-sample latch. All expectations below are independently
calculated from the resolved rules, not hardware captures or another emulator's
output.

## Implemented operation order

For each active voice after its key-on delay:

1. Release commits `max(E-8,0)` immediately and retains `H`.
2. Otherwise start candidate `T` from committed `E`, select rate and compute
   ADSR/GAIN arithmetic even at rate zero. Direct GAIN instead sets `T=GAIN*16`
   and uses always-eligible rate 31. Bent increase adds 32 if unsigned `H<$600`,
   otherwise 8. Candidates use signed `i32`; reachable values fit without
   overflow. Negative H is interpreted as unsigned for the comparison; neither
   negative nor overflowing H is clamped or masked to eleven bits.
3. Decay enters sustain if `T>>8` equals `ADSR2>>5` in ADSR, or `GAIN>>5` in
   GAIN. Save `H=T`. Attack enters decay if unsigned `T>$7ff` (including T<0).
   These actions do not depend on the rate event, and do not change the rate
   already selected for this sample.
4. Only an eligible rate commits `clamp(T,0,$7ff)` to E. ENVX and audio consume
   E, never H. Rate zero therefore updates H/phase while holding E.

The observed distinction does not depend on choosing a particular physical latch
width beyond retaining the candidate's sign and overflow. `i32`/`u32` implement
that comparison; they do not assert a 32-bit physical envelope register.

KON replaces the voice with E=H=0 and Attack, and the existing five empty samples
perform no envelope evaluation. Release reaching zero and FLG soft reset retain H
in the inactive voice; the next KON clears it. Constructing Dsp/Apu, including
Machine reset's Bus reconstruction, initializes H=0. Inactive-voice early exit
and deterministic reconstruction are retained sample-API conventions, not claims
about physical startup/reset retention. No new reset entry point is added.

The Issue #13 counter convention remains C=0 then decrement before each sample's
poll, modulus 30720; writes/KON/KOFF do not restart it. Audio still consumes the
newly committed E in the same API sample. Revision 1212's physical SOUND
GENERATION applies the previous envelope before updating it; implementing that
subcycle pipeline is outside this Issue. Guest timing and PCM here explicitly
use the retained sample API, not a claim of physical SPC/DSP race timing.

## Corrected earlier contract

The older keyed ADSR test assumed a committed-envelope decay transition.
For ADSR1=$ff, ADSR2=$c3, tick 69 starts with E=1799 and computes T=1791.
Its upper bits match sustain level 6, so it enters sustain **on the hold**.
Tick 70 polls sustain rate 3 and holds E=1799; tick 536 commits 1791.
The former expectations (tick 69 still decay, E=1791 at 70 and 1784 at 536)
were contradicted by revision 1212 and are corrected, with the historical
[Issue #13 record](issue-13-validation.md) explicitly marked as superseded here.
Attack now tests unsigned overflow rather than committed E>=2047; candidate
`$7e0` remains attack. Phase tracking in GAIN also uses the candidate and the
GAIN sustain-level bits. No scheduler, noise, pitch, echo or mixer rule changes.

## Independent numerical evidence

Six unit tests in [dsp_envelope_tests.rs](../crates/snes-core/src/dsp_envelope_tests.rs)
exercise actual samples. Expectations are literal values. Threshold vectors
start via public direct GAIN/KON, including five asserted silent samples.
No test seeds private H. The held E=1504 sequence yields H=1536,1512,1536 on
consecutive rate-zero or nonzero holds, then commits 1512 or 1536 depending on
which preceding H applies. E=1488 always prepares 1520; E=1536 prepares 1544;
E=1552 prepares 1560. A rate-3 vector holds E=1504 through tick 535, commits
1512 at tick 536, then holds E with H=1544 at 537, proving arithmetic restarts
from E rather than accumulating H.

Selected additional vectors (each starts from direct GAIN):

| Starting E | Operation | Committed E | H | Next bent-31 E |
| --- | --- | --- | --- | --- |
| 16 | linear decrease, rate 0 | 16 | -16 | 24 |
| 16 | linear decrease, rate 31 | 0 | -16 | 8 |
| 2016 | linear increase, rate 0 | 2016 | 2048 | 2024 |
| 2032 | linear increase, rate 31 | 2047 | 2064 | 2047 |
| 1536 | exponential decrease, rate 0 | 1536 | 1530 | 1568 |
| 1536 | linear decrease, rate 0 | 1536 | 1504 | 1568 |
| 1504 | slow ADSR attack hold | 1504 | 1536 | 1512 |
| 2032 | slow attack then decay hold | 2032 | 2024 | 2040 |

Tests assert exact E/H/ENVX and selected OUTX/stereo PCM, GAIN phase transitions,
ADSR re-entry, release overriding direct GAIN, release-to-zero, retrigger,
five-sample delay, FLG reset/inactive isolation and reconstruction.
With NON held at seed `$4000`, signed source is -32768; envelope output is
`-16E`, OUTX is its high byte, and voice/master volumes 64 each give PCM `-4E`.
Thus E=1512 yields OUTX=161 and stereo -6048; E=2047 yields OUTX=128 and -8188.

The [public-register discriminator](../crates/snes-core/tests/dsp_hidden_envelope.rs)
uses direct 94, one bent-$e0 hold, then bent-$ff. It asserts E=1504 through PCM
-6016 and ENVX=94, then final PCM -6048, ENVX=94 and OUTX=161 without reading
private state. Its second test applies 27 identical write/checkpoint boundaries
to two APUs, comparing **all 128 DSP registers and every PCM sample**, with
whole intervals versus irregular chunks `[1,7,666,1001,21,313]` of master clocks.
Each interval additionally has exact numerical PCM and ENVX/OUTX expectations.
It crosses ticks 30719/30720/30721, with dynamic, direct, ADSR, release and KON
transitions. Target clocks use `ceil(tick*32*21477272/1024000)`.

## Original uploaded SPC700 guest

[dsp_envelope.rs](../crates/snes-core/tests/dsp_envelope.rs) retains the original
rate guest and adds `hidden_guest`, 133 original instruction bytes generated in
memory. It uploads to $0200 through the existing firmware-free IPL protocol,
then configures NON, echo-write disable, volumes 64 and direct GAIN=94 via
$F2/$F3. Pitch is zero; no BRR data or firmware is needed.

| Host command at absolute DSP tick | Guest operation | Recorded ENVX |
| --- | --- | --- |
| 1 at 80 | Record direct level | 94 at $1010 |
| 2 at 90 | Select linear GAIN $c0; record held E | 94 at $1011 |
| 3 at 100 | Select bent GAIN $ff; twelve NOPs; record first update | 94 at $1012 |

Linear rate zero keeps H=1536 stable through all intervening samples. Bent's
first evaluation therefore adds 8 to E=1504. The NOPs occupy 24 SPC clocks;
ENVX selection/read follows before the second bent sample in this API.
Acknowledgements must arrive by tick+1; command 3 must finish at tick 101.
The guest then stops, checked after ten additional 21-master-clock advances.
Upload/setup must finish before tick 80. Every acknowledgement is bounded by
1000 advances, and each target window by 100000 advances, of 21 master clocks.
Ticks include upload and launch. The host asserts every PCM value in samples
81..100 is -6016/32768, and both channels of sample 101 are -6048/32768.
These bounded windows establish this sample API's behavior, not subcycle timing.
[Provenance](../tests/roms/PROVENANCE.md#original-envelope-rate-spc700-guest-approved)
records original MIT authorship; no commercial ROM, firmware or external asset
was acquired.

## Exact baseline failures

Temporarily replaced only dsp.rs with
`git show d39804fa5ac6fb528b6518cfae0008c37a3f08e4:crates/snes-core/src/dsp.rs`,
ran each command below, and restored the fixed source in a `finally` block:

```sh
cargo test -p snes-core --locked --test dsp_hidden_envelope held_bent_candidate_changes_next_committed_envx_and_pcm -- --exact --nocapture
cargo test -p snes-core --locked --test dsp_envelope uploaded_spc_records_first_bent_commit_from_a_held_linear_candidate -- --exact --nocapture
```

Both selected one test and exited 101. The public test observed stereo
**(-6144,-6144)** instead of **(-6048,-6048)**; the guest recorded command-3
ENVX **96** instead of **94**. Both pass with the correction. The new private
field is not needed to compile either baseline discriminator. An initial fixed
DSP run also exposed the legacy tick-69 stage assertion, corrected above from
the reconciled specification rather than preserved as an oracle.

## Mandatory validation

Host: macOS arm64, native Rust 1.94.0, rustup Rust 1.99.0 selected for the
SPC assembler and Wasm, Node 24.12.0, Python 3.13.13, Chromium/Playwright.

| Command | Actual result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --workspace --locked` | Passed: 401 tests, zero failures/ignored; includes all README focused Cargo suites |
| `cargo test -p snes-core --lib dsp::envelope_tests --locked` | Passed: six new unit tests |
| `cargo test -p snes-core --test dsp_envelope --test dsp_hidden_envelope --locked` | Passed: both uploaded guests, public discriminator and APU partition test |
| `python3 tools/make-fixture.py .cache/smoke.bin` and `cmp tests/roms/smoke.bin .cache/smoke.bin` | Passed, byte-identical |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Passed: cc65, bass-untech, spcasm from unchanged pins |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Passed: CPU full/basic and SPC, 600 frames each, terminal markers 0649/0452/0557 |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Passed: 11 grayscale bands, 16 scanlines each, final shade through line 223 |
| `python3 tools/build-cleared-games.py` twice | Passed, fresh builds with identical expected Castle/Memory hashes |
| `npm --prefix web ci` | Passed |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Passed |
| `npx --prefix web playwright install chromium` | Passed |
| `cd web && SNES_TEST_PORT=18085 npm test -- --grep-invert '301 seconds'` | Passed: all 37 selected checks, no skips, 2.8 minutes |
| `git diff --cached --check` | Passed after staging all changed/new files |

The browser suite built all eight README CPU fixture examples (hires,
pseudo-hires, obj-sizes, obj-limits, mode7-scroll, mode7-extbg, mode7-transform,
mode7-row). It passed both 18,100-frame cleared-game replays, Castle's
five-map/credits route and freeze negative control, Memory wins/restarts and
numerical/live Web Audio checks, diagnostic audio/control/pause/reset/save
checks, controller and PPU canvas checks, and Pages subpath boot. Optional
unaudited tests named `301 seconds` were excluded as requested. No mandatory
check remains failed, skipped or unperformed. The final fmt/clippy/workspace
run includes the added nonzero-hold and attack-boundary vectors.

Assembler source pins were unchanged: cc65
`555282497c3ecf8b313d87d5973093af19c35bd5`, bass-untech
`9db6088a378061afc7b82f50997a5b9a1d951175`, spcasm
`c2d57b7aa8ddd6b22a18424047b8c2d93f0373e7`.
Measured SHA-256 values (all match the existing fixtures/build assertions):

| Artifact | SHA-256 |
| --- | --- |
| Original smoke | `6e5518b3959883cc22e914621fc5b5d9f5c0fe4eb96ee61000769afdbb68d6e5` |
| CPU full | `1727e7c46ebb5df55711598264ba19005986bd34e915621383966adf674b3cfa` |
| CPU basic | `514d58950ae1ca940d6673c2a028b258b2ab17293bbc7cbd7cb54dded32abc5b` |
| SPC | `279d6a92d051ff8a98e190ce73e40482ff2cc981812509c7bca35a047484507e` |
| HDMA | `aef339e5d4a7319db31de577b253d977b45f5c45f09adfaa99a302ca300a957a` |
| Cleared Castle | `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869` |
| Cleared Memory | `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1` |

## Remaining limits

Exhaustive ADSR/GAIN transitions and register-write combinations, physical
startup/reset retention, subcycle register sampling and SPC/DSP races remain
unverified. Partition equality is determinism evidence and supplements the
numerical vectors; it cannot establish physical timing. No physical capture or
subjective listening was required or performed.

All inventory scopes remain unchanged: partial `cpu`, `bus`, `scheduler`, `dma`,
`hdma`, `controllers`, `ppu-bg`, `ppu-offset`, `ppu-hires`, `ppu-mode7`,
`ppu-objects`, `ppu-color`, `ppu-ports`, `ppu-display`, `spc`, `ipl`, `dsp-voices`,
`dsp-noise`, `dsp-pitch`, `dsp-envelope`, `dsp-echo`, `dsp-timing`; unimplemented
`ppu-raster`, `pal`. Fixture pins, licensing, replay routes, hashes, durations,
music/effect assertions and browser coverage are preserved. This change claims
neither full hardware accuracy nor commercial compatibility and does not
complete Issue #3. No post-merge verification is required by Issue #75.
