# Issue #93: BRR predictor arithmetic and stream continuity

Implemented for [Issue #93](https://github.com/takahirox/snes-rust/issues/93).
Evaluation baseline: `c19d29c8cc9afecf92f894ef67814e0cb5626309`.
The complete bounded matrix passes on that baseline and the candidate. No
arithmetic or continuation defect was demonstrated. The decoder, voice state,
KON queue and sample processing implementation are retained unchanged; only
conditional test-module wiring is added to `dsp.rs`.

## Source reconciliation

Inspected on 2026-10-24:

- [Anomie/jwdonal revision 1212](https://gist.github.com/nyanpasu64/a0d916ce6924912a7116682bf778e9a0#file-apudsp_jwdonal-txt),
  **BRR DECODING**, signed data/shift, exact filter equations, final clamp/clip,
  per-voice history across groups/blocks, end redirection and initialization.
  Its equations use half-amplitude samples, wide signed intermediates and
  individually shifted predictor terms. Saturation precedes signed 15-bit
  truncation; the truncated result supplies future history. The physical ring
  history described for KON is not equivalent to zero initialization.
- [Fullsnes BRR Samples](https://problemkaputt.de/fullsnes.htm#snesapudspbrrsamples),
  **Bit Rate Reduction (BRR) Format**, exact formulas, reserved ranges, clamp,
  lost-sign effect, history feedback and loop notes. These agree with revision
  1212. The decimal coefficients are descriptive only; the following exact
  SAR formulas determine rounding. The header table's shorthand “0=Silent”
  does not replace its later shift-zero equation: negative/odd inputs can
  survive the arithmetic shift. Sequential progression is nine bytes; end
  redirects to the loop word without clearing predictors, even when headers
  change filter/range.
- [Issue #89 reconciliation](issue-89-validation.md#source-reconciliation) and
  [Issue #91 implementation boundary](issue-91-validation.md#source-reconciliation-and-implementation-boundary)
  retain the sample-call mapping, five silent calls, selected first window,
  queued continuation and preceding-envelope order. No earlier numerical
  expectation is weakened or replaced.

There is no disagreement with the Issue's candidate arithmetic contract. Let
`P` be the preceding even signed-16 full-amplitude result, `R` the second
preceding result, `Q=ASR(R,1)`, and `n` signed four-bit data. In wide signed
arithmetic (the oracle uses i64, implementation i32):

```text
B = floor(n * 2^r / 2) for r<=12; otherwise -2048 if n<0, else 0
T0 = B
T1 = B + floor(P/2) + floor(-P/32)
T2 = B + P - Q + floor(Q/16) + floor(-3P/64)
T3 = B + P - Q + floor(-13P/128) + floor(3Q/16)
C = min(32767, max(-32768, T))
F = ((2C + 32768) mod 65536) - 32768
next full history = [F, P]
```

Floor divisions express arithmetic right shifts, including negatives; terms
must stay separately rounded. Products and sums must not narrow before C.
The modulo above is nonnegative. F is even and fits signed 16 bits; it is not
the saturated intermediate and is the value fed back.

The representation conversion is exact: production `p=P/2`, `q=R/2=Q`,
with `p,q` signed 15-bit in [-16384,16383]. Thus `ASR(P,1)=p`,
`ASR(-P,5)=ASR(-p,4)`, `ASR(-3P,6)=ASR(-3p,5)` and
`ASR(-13P,7)=ASR(-13p,6)`. The remaining terms substitute `P=2p`,
`Q=q` directly. Doubling C, wrapping as signed 16, then ASR by one is
exactly signed 15-bit truncation of C: `S=F/2`. This proves the candidate
full-amplitude contract equivalent to the core's stored half amplitude,
including sign loss and saturation endpoints. Positive C=32767 gives F=-2,
S=-1; negative C=-32768 gives F=S=0.

Default/core KON zero predictors in these tests are an explicit initialized
API convention, not evidence of physical KON predictor retention. The hardware
research describes prior physical ring contents. Final pre-KON decode, live
selection and retention remain outside this Issue, as in #89/#91. Public ENDX
here follows the retained scalar next-block-decode convention, not physical
lookahead/publication timing. No emulator implementation or trace was used as
an oracle.

## Independent numerical and stream evidence

[`dsp_brr_tests.rs`](../crates/snes-core/src/dsp_brr_tests.rs) contains four
private-state tests. Its independent model uses full-amplitude i64 arithmetic,
Euclidean division and modular reduction, with no production helper or table.
Frozen anchors constrain the model as well as the implementation comparisons.

The matrix is 3 filters × 16 ranges × 16 nibbles × 17 valid history pairs =
13,056 starting cases, each decoding two samples (26,112 comparisons). History
pairs include zero, both signs, unequal and reversed positive/negative pairs,
small rounding-sensitive values, and both even signed-16 endpoints. Both history
entries and the decoded result are checked after each sample, including wrapped
feedback into the second sample. Saturation and wrapping are distinct cases:

| Filter; n/r; full [P,R] | Separately rounded T calculation | Full result F |
| --- | --- | ---: |
| 1; 0/0; [2,0] | 0+1-1=0 | 0 |
| 1; 0/0; [-2,0] | 0-1+0=-1 | -2 |
| 2; 0/0; [6,-10] | 0+6+5-1-1=9 | 18 |
| 2; 0/0; [-10,6] | 0-10-3+0+0=-13 | -26 |
| 3; 0/0; [6,-10] | 0+6+5-1-1=9 | 18 |
| 3; 0/0; [-10,6] | 0-10-3+1+0=-12 | -24 |
| 1; 7/12; [32766,-32768] | 14336+16383-1024=29695; no saturation | -6146 |
| 1; -8/12; [-32768,32766] | -16384-16384+1024=-31744; no saturation | 2048 |
| 2; 7/12; [32766,-32768] | 14336+32766+16384-1024-1536=60926; C=32767 | -2 |
| 2; -8/12; [-32768,32766] | -16384-32768-16383+1023+1536=-62976; C=-32768 | 0 |
| 3; 7/12; [32766,-32768] | 14336+32766+16384-3328-3072=57086; C=32767 | -2 |
| 3; -8/12; [-32768,32766] | -16384-32768-16383+3328+3071=-59136; C=-32768 | 0 |

For example filter 1, n=-8/r=12, P=-32768 has T=-31744 without
saturation, then F=2048 after wrapping. This negative lost-sign case and all
reserved ranges are also in the matrix. Combining divisions would fail the
small anchors: filter 2 at P=6, Q=-5 has separate corrections -1 and -1;
combining those divisions gives floor(-38/64)=-1, incorrectly yielding F=20
instead of 18.

The stream test interleaves two voices with different initial histories and
start words for 112 samples each. Voice 0 follows A→B→C→D→C→D→C:

| Block | Address | Header | Eight payload bytes |
| --- | --- | --- | --- |
| A | $3000 | $94 (range9/filter1/sequential) | 4c 2f 61 d3 85 e7 09 2b |
| B | $3009 | $7b (range7/filter2/end+loop) | 6e repeated |
| C | $3100 | $ac (range10/filter3/sequential) | 4c 2f 61 d3 85 e7 09 2b |
| D | $3109 | $d7 (range13/filter1/end+loop) | 8f repeated |

Both loop words are $3100; voice 1 starts at C. Every decode asserts the
sample, newest/second-newest history and end result, plus isolation of the other
voice. At every block transition, zeroed, reversed, stale and other-voice
histories must produce a different result. Fourteen literal voice-0 boundary
pairs constrain the model. Examples, full amplitude newest first: sample 15
[-1612,1012], 16 [-3256,-1612], 31 [10928,9496], 32 [16016,10928],
63 [19804,25494], 64 [18966,19804], 95 [10286,15342], 96 [10112,10286].
For B's first sample, B=384, P=-1612, Q=506, giving
T=384-1612-506+31+75=-1628, hence F=-3256. For C's first sample,
B=2048, P=10928, Q=4748 gives
T=2048+10928-4748-1110+890=8008, hence F=16016.

The preload test separately runs filters 1–3 with the signed original payload
above at range 9. The signed nibble order reuses the repository's original MIT
#89 payload; predictor headers, mixed-block streams and expectations are new. Three literal first-block half-amplitude sequences constrain
its independent recurrence. Silent calls 2/3/4 must decode 4/8/12 samples;
call 4 restores audible window 0–3 but predictors stay at samples 11/10.
Thirty-two zero-pitch output calls preserve that separation. Integer resume
checks every window through position 48; queued samples 4–11 never change
predictor history, and position 9 decodes sample 12 using 11/10 exactly once.
Additional holds immediately before and after that boundary preserve address,
queue cursor, history and window. This discriminates substituting the selected
window history, decoding the queue again, or resuming from stale history.

## Public PCM, OUTX and uploaded guest

[`dsp_brr_filters.rs`](../crates/snes-core/tests/dsp_brr_filters.rs) contains
two tests covering all three filters. Each uses the same original range-9
payload with end+loop back to itself. Forty-nine literal output anchors per
filter cover windows 0–48 and multiple loops. These literals were calculated
from the contract above, not collected from core output. Phase zero uses
published taps [370,1305,374,0]/1024, flooring products separately, signed-16
wrapping the first three contributions, clearing the low bit, then applying
preceding E=1024 and clearing the low bit again. Voice volumes +64/-128 and
master +64 yield PCM `(O>>2, (-O)>>1)`; OUTX is O's high byte.

| Filter | First four half-amplitude decoded samples | Tap sum → even source → O | First PCM / OUTX |
| --- | --- | --- | --- |
| 1 | [1024,-64,452,167] | 370-82+165=453 → 452 → 226 | (56,-113) / 0 |
| 2 | [1024,928,1321,1392] | 370+1182+482=2034 → 2034 → 1016 | (254,-508) / 3 |
| 3 | [1024,816,1146,1140] | 370+1039+418=1827 → 1826 → 912 | (228,-456) / 3 |

For instance filter 2's second nibble has B=-1024, P=2048, Q=0,
T=-1024+2048-96=928; F=1856, stored sample=928. Its third nibble
has B=512, P=1856, Q=1024, T=512+1856-1024+64-87=1321.
Public checks retain five silent calls, first window at zero pitch, a long hold,
resume, both sides of the queue boundary, OUTX/ENVX/ENDX and loop continuity.
Direct GAIN 64→0→64 while held requires the preceding envelope on the first
call after each write and the new value only on the next call, preserving #91.

The original SPC700 guest uploads fewer than 1024 bytes at $0200 through the
firmware-free port protocol. Setup writes DSP registers through $F2/$F3 and
signals ready=$7f before sample 100. Host checkpoints start at sample 200,
allowing that bound plus five silent samples before the checked 20-frame
stable interval. Eleven commands read ENVX/OUTX/ENDX via $F2/$F3 into
$1100–$1120. After each of the first ten records, a pitch pulse advances one
window: MOV dp,#imm takes five SPC clocks; eleven two-clock NOPs between the
pitch writes make their $F3 effects exactly 5+22+5=32 clocks apart. Thus one
DSP sample advances at $1000 pitch regardless of clock phase. Pitch then
returns to zero. This is this core's whole-instruction effect convention, not
physical SPC/DSP bus-edge evidence. The final command acknowledges and STOPs.

Every upload/ready poll has a 1000×21-master-clock bound. Each command must
acknowledge within ten PCM frames at a fixed checkpoint. Two fresh APUs per
filter run whole intervals versus irregular [1,7,666,1001,21,8193] master tick
partitions. **Each run independently** checks the expected PCM on twenty frames,
registers and the accumulated literal records before comparing every PCM sample,
all DSP registers, all RAM and output ports. Equality alone proves determinism.
Both executions require STOP and no error. New fixture authorship and MIT terms
are recorded in [PROVENANCE.md](../tests/roms/PROVENANCE.md#issue-93-original-brr-predictor-vectors-and-spc700-guest).

## Exact-baseline evidence

A detached worktree at the exact baseline received only the new private/public
test files and conditional module wiring. No baseline production behavior was
changed:

```sh
git worktree add --detach .cache/issue-93/baseline c19d29c8cc9afecf92f894ef67814e0cb5626309
# Copy dsp_brr_tests.rs, dsp_brr_filters.rs and add the cfg(test) module declaration.
CARGO_TARGET_DIR="$PWD/.cache/issue-93/baseline-target" cargo test \
  --manifest-path .cache/issue-93/baseline/Cargo.toml --locked \
  -p snes-core --lib dsp::brr_tests
CARGO_TARGET_DIR="$PWD/.cache/issue-93/baseline-target" cargo test \
  --manifest-path .cache/issue-93/baseline/Cargo.toml --locked \
  -p snes-core --test dsp_brr_filters
```

Baseline: four private and two public tests pass, zero failures/ignored. Candidate:
the same six tests pass. There is no baseline behavioral failure to correct and
no production correction is claimed. Earlier Issue #89/#91 production changes
are not repeated. During test authoring one provisional filter-1 anchor was
corrected from -6150 to the worked -6146 above; the guest's first checkpoint
was moved from 100 to 200 to respect its documented upload/setup bound rather
than asserting PCM before initialization. Existing expectations were untouched.

## Mandatory validation actually executed

Local raw logs: `.cache/issue-93` (not committed). Host: macOS arm64; native
Rust 1.94.0; rustup stable Rust 1.99.0 for SPC assembler and Wasm; Node 24.12.0,
Python 3.13.13. The first tool build with native Rust failed on the auxiliary
assembler's newer Rust features; the documented RUSTC override succeeded.

| Command | Actual result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `git diff --cached --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --workspace --locked` | Passed: 463 tests, zero failed/ignored |
| `cargo test --locked -p snes-core --test dsp_brr_filters` | Passed: two tests, each exercising all three filters |
| Exact-baseline private/public commands above | Passed: four private + two public tests, zero failed/ignored |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Passed: unchanged pinned cc65, bass-untech, spcasm sources |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Passed: three 600-frame suites, CPU full/basic and SPC markers 0649/0452/0557 |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Passed: eleven 16-scanline grayscale bands, last shade through line 223 |
| `python3 tools/make-fixture.py .cache/smoke.bin` then `cmp tests/roms/smoke.bin .cache/smoke.bin` | Passed: identical bytes |
| `python3 tools/build-cleared-games.py` twice | Passed: both games reproduce expected hashes on both builds |
| `npm --prefix web ci` | Passed: zero audit vulnerabilities |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Passed: Wasm/web build; wasm32 target already installed |
| `npx --prefix web playwright install chromium` | Passed |
| `cd web && RUSTC="$(rustup which rustc)" SNES_TEST_PORT=18085 npm test -- --grep-invert '301 seconds'` | Passed: 37 tests, zero skips, 2.8 minutes |

The browser command rebuilt all eight README original CPU-built fixture sets.
It retained Castle's five-minute gameplay and complete five-map/credits route,
Memory gameplay/music/effects, Web Audio, queue/drift and freeze assertions.
Only the prescribed optional unaudited `301 seconds` tests were excluded. No
mandatory check remains skipped or unperformed.

| Rebuilt artifact | SHA-256 (unchanged) |
| --- | --- |
| Smoke | `6e5518b3959883cc22e914621fc5b5d9f5c0fe4eb96ee61000769afdbb68d6e5` |
| CPU full | `1727e7c46ebb5df55711598264ba19005986bd34e915621383966adf674b3cfa` |
| CPU basic | `514d58950ae1ca940d6673c2a028b258b2ab17293bbc7cbd7cb54dded32abc5b` |
| SPC | `279d6a92d051ff8a98e190ce73e40482ff2cc981812509c7bca35a047484507e` |
| PPU/HDMA | `aef339e5d4a7319db31de577b253d977b45f5c45f09adfaa99a302ca300a957a` |
| Castle, both builds | `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869` |
| Memory, both builds | `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1` |

Filter-0, KON, directory, terminal-header, envelope, PMON, noise, mixing and echo
regressions all passed unchanged in the workspace run. Only original or
explicitly licensed source-built fixtures were used. No fixture/tool/game pin,
source allowlist, license, expected hash or existing test assertion was changed.

## Inventory and remaining limits

All 24 categories retain their full [inherited scopes](issue-81-validation.md#inventory-and-remaining-limits)
and subsequent corrections in [#89](issue-89-validation.md#inventory-and-remaining-limitations)
and [#91](issue-91-validation.md#inventory-and-remaining-limitations). There remain
22 partial, two unimplemented and none verified:

- Partial: `cpu`, `bus`, `scheduler`, `dma`, `hdma`, `controllers`, `ppu-bg`,
  `ppu-offset`, `ppu-hires`, `ppu-mode7`, `ppu-objects`, `ppu-color`, `ppu-ports`,
  `ppu-display`, `spc`, `ipl`, `dsp-voices`, `dsp-noise`, `dsp-pitch`,
  `dsp-envelope`, `dsp-echo`, `dsp-timing`.
- Unimplemented: `ppu-raster`, `pal`.

This closes bounded stable-input filters 1–3 arithmetic and ordinary predictor
continuity evidence, including the existing twelve-sample preload boundary.
It does not establish complete hardware accuracy or commercial compatibility,
and does not complete Issue #3. Outstanding work retains exhaustive interpolation
phases, simultaneous key/mixing combinations, KON history and selection questions,
final pre-KON decode/filter retention, physical header/key/directory/publication
timing, DSP subcycle scheduling, SPC instruction bus edges, register/RAM races,
torn writes and physical reset retention. The NON advancement dispute remains
unresolved. No new interpolation algorithm, firmware, UI or enhancement chip is
included. No physical-device testing was performed; no optional validation or
required post-merge verification is specified by #93.
