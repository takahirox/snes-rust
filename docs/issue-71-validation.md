# Issue #71: sample-level FIR and echo arithmetic

Baseline: `9b3a7403f440f709593ff9288eb19cf4c8806a56`. Scope: fixed ESA/EDL,
existing sample boundary, bounded voice sends. No imported emulator code or ROM.

## Reconciled contract (established before golden vectors)

[Fullsnes, **SNES APU DSP Echo Registers**, xFh FIRx, ESA/EDL, EFB/EVOL/EON](https://problemkaputt.de/fullsnes.htm#snesapudspechoregisters)
specifies signed RAM, input SAR 1, oldest-to-newest coefficients, seven wrapping
terms then a saturating addition, signed volumes and even writeback. Its FIR
pseudocode omits the FIR low-bit mask and feedback saturation.
[Anomie/jwdonal revision 1212, **SOUND GENERATION**, **VOLUME CONTROL & ECHO**,
**$xf FFCx**](https://gist.github.com/nyanpasu64/a0d916ce6924912a7116682bf778e9a0#file-apudsp_jwdonal-txt)
agrees on orientation/scaling, explicitly supplies FIR masking and feedback
clamping. Its unparenthesized formulas must shift each product, not the sum.
[Fullsnes, **SNES APU DSP Control Registers**, 6Ch FLG](https://problemkaputt.de/fullsnes.htm#snesapudspcontrolregisters)
and Anomie's **$6c FLG** agree on continuing reads/history/address processing
under write-disable and continuing internal processing under mute.

The -128 coefficient/input endpoint can produce +32768 after shifting.
Fullsnes warns of multiply overflow but neither prose formula explicitly narrows
the final product. The research implementation [Shay Green, snes_spc 0.9.0,
**Echo**, CALC_FIR, echo_25/26, echo_output](https://github.com/bsnes-emu/bsnes/blob/78474e5d39f58872dc21aca18cf9eb583b147fe7/bsnes/sfc/dsp/SPC_DSP.cpp#L583-L672)
resolves that ambiguity by signed-16 narrowing of the final tap and each scaled
volume/feedback term. This is corroborating emulator convention consistent with
the documented 16-bit intermediates, not new physical-device verification.
No implementation source was copied. Endpoint tests explicitly exercise this
choice. Mute is idealized digital silence; analog DAC transients are outside scope.

Executable mathematics, independently for each channel (`SAR` is floor division;
`W16(v)=((v+32768) mod 65536)-32768`, `C16` clamps to [-32768,32767]):

```text
S[n] = signed_little_endian_RAM_word(address) SAR 1
T[k] = (S[n-7+k] * signed8(FIR[k])) SAR 6, k=0..7
P = W16(T[0]+T[1]+T[2]+T[3]+T[4]+T[5]+T[6])
F = C16(P + W16(T[7])) & -2
B = W16((F * signed8(EFB)) SAR 7)
RAMword = C16(voice_send + B) & -2
E = W16((F * signed8(EVOL)) SAR 7)
D = W16((voice_dry * signed8(MVOL)) SAR 7)
PCM = C16(D+E), or zero when FLG.6=1
```

Products use enough signed bits without overflow. Seven-term modular addition
in coefficient order is equivalent to narrowing after each addition. There is
no other FIR saturation. Low-bit masking follows saturation. For bounded voice
inputs the feedback addition uses a wide signed sum, then clamps, never wraps.
FIR ages are 7,6,5,4,3,2,1,0 for coefficients 0,1,2,3,4,5,6,7.

FLG.5 suppresses both stereo RAM writes only. Reads, FIR history, filtering,
PCM, history index and buffer offset continue. FLG.6 affects PCM only; feedback
writes continue. EON gates voice sends, not RAM reads or echo playback. Every
sample advances four bytes modulo max(4,(EDL&15)*2048); addresses wrap at 16 bits.
This fixed-register reduction makes no claim about live ESA/EDL sampling.

This standalone Python form is executable without the core (history is eight
signed RAM words, oldest first; sends/dry are already bounded voice sums):

```python
def wrap16(v):
    return (v + 32768) % 65536 - 32768

def clamp16(v):
    return max(-32768, min(32767, v))

def echo_channel(history, coefficients, efb, evol, mvol, send=0, dry=0):
    terms = [((word >> 1) * coefficient) >> 6
             for word, coefficient in zip(history, coefficients)]
    fir = clamp16(wrap16(sum(terms[:7])) + wrap16(terms[7])) & -2
    stored = clamp16(send + wrap16((fir * efb) >> 7)) & -2
    pcm = clamp16(wrap16((dry * mvol) >> 7) + wrap16((fir * evol) >> 7))
    return fir, stored, pcm

assert echo_channel([0]*7 + [16386], [0]*7 + [64], 64, 64, 0) == (8192, 4096, 4096)
assert echo_channel([16384]*8, [127]*7 + [64], 64, 64, 0) == (-9088, -4544, -4544)
assert echo_channel([-32768]*8, [16]*7 + [127], 64, 64, 0) == (-32768, -16384, -16384)
```

## Original derivations

For a $4000 left impulse, coefficients 1..8 and EVOLL=64: S=8192;
the successive nonzero terms are 1024,896,768,640,512,384,256,128.
Thus nine PCM values are **512,448,384,320,256,192,128,64,0**.
With -$2000 on the right and EVOLR=64: **-256,-224,-192,-160,-128,-96,-64,-32,0**.

Odd inputs 131/-131 become 65/-66; coefficient 127 gives 128/-131,
FIR masking gives 128/-132. EVOL=-128 produces -128/132. For input -32768
and coefficient -128 the product term is +32768; final-tap narrowing makes
-32768. EFB=-128 then also narrows +32768 to -32768 before addition.

Seven inputs 16384 with coefficient 127 give 7*16256=113792, wrapping to
-17280. A newest coefficient 64 adds 8192: F=-9088. Opposite inputs give
+17280-8192=9088. Unrestricted sums would saturate to the opposite endpoints.
Seven coefficients 16 and newest 127 with input 32766 give P=28658 and
T7=32510: saturation/masking yields 32766. Negative input -32768 gives
P=-28672, T7=-32512, F=-32768.

With only FIR7=64 and EDL=0, F is the even current RAM word divided by two.
Starting with 16386/-8194, EFB=64 writes 4096/-2050,1024/-514,256/-130,64/-34;
EFB=-64 writes -4096/2048,1024/-512,-256/128,64/-32.
EVOL=64 produces respectively 4096/-2049,1024/-513,256/-129,64/-33 and
4096/-2049,-1024/512,256/-128,-64/32. These literals use the equations,
never captured production output. Draft expectations initially treated coefficient
64 as unity; the explicit input SAR 1 calculation corrected that drafting error
before the final baseline experiment.

The old noise assertion `((2*127)>>7)=1` must become 0 on RAM writeback.
Other changed left words merely clear that same low bit; source, ENVX, OUTX,
right words and dry PCM stay unchanged. See Anomie SOUND GENERATION echo output
and Fullsnes ESA byte layout above.

## Validation

### Baseline failure / fixed pass

A Python `try/finally` harness temporarily installed the exact baseline
`crates/snes-core/src/dsp.rs` from `git show`, adding only the new test-module
include. It ran:

```sh
cargo test --locked -p snes-core --lib dsp::echo_tests::issue_impulse_reproduction
```

Actual baseline exit 101: first PCM was `(64,0)`, expected `(512,0)`. The harness
restored the fixed production file byte for byte; the identical command passed.
The full nine-value expected sequence is independently derived above. The full
historical noise suite passes with the low-bit correction; all source/ENVX/OUTX
and dry-PCM assertions are retained. A second `try/finally` experiment installed
both exact baseline DSP and noise-test files and ran
`cargo test --locked -p snes-core --lib dsp::noise_tests::signed_noise_envelope_outx_stereo_and_echo_vectors`:
exit 0 with the historical send expression (including sample 2 -> RAM 1).
Restoring both fixed files and rerunning passed, exit 0, with sample 2 -> RAM 0.
No other assertion in that historical test was changed.

### Focused numerical coverage

Eleven original unit tests cover every coefficient's age individually, signed
stereo odd inputs and negative rounding, overlapping asymmetric impulses across
history wrap, both coefficient endpoints, multiply/feedback/EVOL narrowing,
seven-term wrapping versus final-add saturation for both signs with a nonzero
last tap, positive/negative feedback recurrences, saturated bounded voice sends,
echo-only and dry-plus-echo PCM, zero FIR/EFB/EON/EVOL controls, write-disable,
mute, history continuity, fixed EDL 0/1, and 16-bit RAM-address wrap. PCM words,
history input values and RAM words/bytes are literal independent expectations.
The first-seven overflow pair is F=-9088/+9088, hence PCM and EFB=64 writes
-4544/+4544 (bytes `40 EE C0 11`). Final-add saturation gives F=32766/-32768,
hence PCM 16383/-16384 and RAM 16382/-16384 (`FE 3F 00 C0`). These distinguish
wrapping from partial clamping, unrestricted accumulation and final-only clamping.

One held-noise voice supplies dry/send 16384/-8192 at half envelope. With FIR7=127
and input 32766/-32768, F=32510/-32512, EFB=127 gives 32256/-32258, and send
addition saturates RAM to 32766/-32768. MVOL=64 and EVOL=127 saturate PCM to
32767/-32768 over four recurrences. EON=0 instead writes 32256/-32258, preserving
dry PCM. EFB=EVOL=-128 gives RAM -16126/24320 and PCM -24318/28416. This isolates
the echo consumers without extending the general multi-voice mixer contract.

The original uploaded guest is built by Cargo from its in-memory instruction
builder (no assembler/firmware). It executes `$F2/$F3` configuration, waits for
commands, enables echo writes, records `$6000..$6003` into `$1000..$100B`, then
executes STOP. Initial RAM is a repeated original stereo pair 16386/-8194.
FIR7=64, EFB=64, EVOL=64, EDL=1 imply 512-sample passes: PCM 4096/-2049,
1024/-513,256/-129,64/-33. Guest commands after 600/1120/1640 output samples
record respectively 4096/-2050,1024/-514,256/-130. The first public RAM mutation
anchors the pass without assigning private DSP state. Upload is below 256 bytes,
ready before 100 samples, acknowledgements are bounded by 1000 advances, and
completion is before first-write+1650 samples, with STOP and no core error.
All PCM from first write through completion is checked numerically.

The second integration test compares every PCM sample, all 128 DSP registers and
all echo RAM bytes for whole versus chunks `[1,7,666,1001,21,8193]` through 1537
samples for both EDL=0 and 1. It retains exact master-clock/sample count checks
and supplies independent literal checkpoints across three delay wraps and many
history wraps. No APU scheduler or cadence checks were changed.

### Mandatory commands

Host (2026-10-24): macOS arm64, native Homebrew Rust 1.94.0, installed rustup
Rust 1.99.0 for SPC assembler/Wasm, Node 24.12.0, Python 3.13.13. Tool-build and web-build use
README's `RUSTC="$(rustup which rustc)"` selection because the native compiler
does not own the installed Wasm target; this is the documented toolchain
prerequisite, with no dependency/tool pin substitution.
Generated logs are under `.cache/issue-71`, hardware JSON under
`.cache/hardware-tests` / `.cache/ppu-test`, and browser evidence under
`web/test-results`; these artifacts are ignored, with relevant results retained
here.

| Command | Actual result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --workspace --locked` | 385 passed, 0 failed, 0 ignored |
| `cargo test --locked -p snes-core --lib dsp::echo_tests` | 11 passed |
| `cargo test --locked -p snes-core --test dsp_echo` | 2 passed |
| Standalone Python arithmetic block above | All three worked assertions passed |
| `python3 tools/make-fixture.py .cache/smoke.bin` | Passed |
| `cmp tests/roms/smoke.bin .cache/smoke.bin` | Byte-identical, SHA-256 `6e5518b3959883cc22e914621fc5b5d9f5c0fe4eb96ee61000769afdbb68d6e5` |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | All pinned assemblers built |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | CPU full/basic and SPC suites passed, 600 frames each |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | 11 grayscale bands, 16 scanlines each, final shade through line 223 |
| `python3 tools/build-cleared-games.py` twice | Both builds passed; identical pinned Castle/Memory hashes |
| `npm --prefix web ci` | Passed, 0 vulnerabilities |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Passed |
| `(cd web && npx playwright install chromium)` | Passed |
| `SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'` | 37 passed, no skips, 2.8 minutes |

The first browser invocation was started before the cleared-game builds had
finished. It had 30 passing tests and seven failures, all ENOENT for unavailable
Castle/Memory ROM paths. This was a validation-order error, with no emulation or
assertion change. The complete suite was rerun after both fresh builds completed: **37 passed**,
no failures or skips. Both cleared five-minute replays (18,100 frames), Castle's
complete five-map/credits route and freeze negative control, Memory's numerical
music/effect checks, actual gesture-activated Web Audio and bounded queue/drift
checks all remain. Only the pre-existing optional unaudited games matching
`301 seconds` were excluded. No mandatory check remains failed, skipped or
unperformed.

Hardware terminal evidence and unchanged hashes:

| Fixture | Terminal evidence | SHA-256 |
| --- | --- | --- |
| CPU full | last test $0649 / PC $0081A2 | `1727e7c46ebb5df55711598264ba19005986bd34e915621383966adf674b3cfa` |
| CPU basic | last test $0452 / PC $0081A2 | `514d58950ae1ca940d6673c2a028b258b2ab17293bbc7cbd7cb54dded32abc5b` |
| SPC | last test $0557 / PC $008060 | `279d6a92d051ff8a98e190ce73e40482ff2cc981812509c7bca35a047484507e` |
| HDMA bands | final shade through line 223 | `aef339e5d4a7319db31de577b253d977b45f5c45f09adfaa99a302ca300a957a` |
| cleared Castle | both source builds | `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869` |
| cleared Memory | both source builds | `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1` |

No unrelated assertions, replay lengths, source/tool pins, fixture hashes,
provenance gates or inventory statuses were weakened. Existing envelope, noise,
pitch, controller and DMA/HDMA regressions remain in the passing workspace gate.
Only original and provenance-approved source-built fixtures were used.

## Inventory and remaining scope

All 24 categories retain their inherited status and feature scope: 21 partial,
`dsp-voices` implemented_unverified, `ppu-raster` and `pal` unimplemented, none
verified. The table retains the inherited inventory from
[Issue #63](issue-63-validation.md); subsequent bounded PPU-port and controller
evidence in Issues #65/#67/#69 also remains in force. Only the selected echo
evidence is refined here; no status is promoted.

| Category | Status | Retained scope and limitations |
| --- | --- | --- |
| `cpu` | partial | Complete 65C816 semantics, interrupt edges, bus order/count and revision-specific timing; existing licensed suites do not prove all edges |
| `bus` | partial | Cartridge maps, mirrors, I/O and open bus; unsupported maps/chips and electrical floating values remain |
| `scheduler` | partial | CPU/PPU/APU/DMA synchronization and NTSC events; exact access alignment, arbitration and physical blanking latency remain |
| `dma` | partial | Transfer modes, direction, access restrictions and WRAM conflicts; full timing, electrical values and interaction matrix remain |
| `hdma` | partial | Initialization, descriptors, counters, direct/indirect reloads and phases; exact startup/overheads, arbitrary enabling and same-channel contention remain |
| `controllers` | partial | Serial and automatic joypad reads, busy/completion and input; complete device/protocol/timing coverage remains |
| `ppu-bg` | partial | Planar tiles/maps, scroll, priorities, direct color and mosaic; complete mode interactions and timed mosaic state remain |
| `ppu-offset` | partial | Modes 2/4/6 H/V descriptors, sizes and boundaries; mosaic interactions and exact fetch/raster behavior remain |
| `ppu-hires` | partial | Ordinary Mode 5/6 and math-disabled SETINI output; hires-math dispute, color-window clipping/prevention, ordinary-hires composition, mosaic, full OBJ behavior, interlace/overscan and switching/fetch timing remain |
| `ppu-mode7` | partial | Transform/direct color and identity pseudo-hires control; EXTBG and complete transform/overflow/edge behavior remain |
| `ppu-objects` | partial | OAM attributes, rendering, priorities and simple screen/window routing; physical evaluation/fetch and overflow assertion timing, rotation oddities, complete size/mode interactions, interlace and hires behavior remain |
| `ppu-color` | partial | Palettes, direct color, composition, brightness and math; comprehensive windows, clipping/prevention, subtraction/saturation, halving and physical DAC behavior remain |
| `ppu-ports` | partial | VRAM/OAM/CGRAM ports, access ownership, latches and counters; exact rendering-owned corruption, latch retention, access latency and pixel-fetch arbitration remain |
| `ppu-raster` | unimplemented | Dot-level pipeline, mid-scanline writes, fetch arbitration and physical mode-switch timing |
| `ppu-display` | partial | Packed framebuffer, geometry, blanking and display shape; interlace/overscan, physical scanout and switching timing remain |
| `spc` | partial | SPC700 instructions, timers and communication; full timing and register/RAM races remain |
| `ipl` | partial | Firmware-free high-level upload; IPL instruction timing and general firmware execution remain |
| `dsp-voices` | implemented_unverified | BRR decode, voice mixing, interpolation and key-on behavior; exhaustive BRR filters, interpolation phases and hidden behavior remain |
| `dsp-noise` | partial | Shared noise generator/rates and NON routing; DSP subcycle sampling and write races remain |
| `dsp-pitch` | partial | Pitch/PMON, interpolation advancement and noise interaction; subcycle PMON sampling and torn/live writes remain |
| `dsp-envelope` | partial | ADSR/GAIN shared rates, release/retrigger and boundaries; hidden/bent GAIN, exhaustive transitions and live interactions remain |
| `dsp-echo` | partial | Fixed-register FIR/feedback arithmetic, saturation, even writeback and numerical PCM/RAM vectors; live ESA/EDL sampling, subcycle register/RAM races, SPC bus synchronization, physical startup/reset retention and comprehensive multi-voice dry/send saturation remain |
| `dsp-timing` | partial | Deterministic sample cadence and browser queues; DSP subcycles, exact hardware timing and audible fidelity remain |
| `pal` | unimplemented | PAL timing, video geometry and audio synchronization |

`dsp-echo` and `dsp-timing` are not complete. Live ESA/EDL register changes,
subcycle DSP register/RAM races, SPC bus synchronization, physical startup/reset
retention and comprehensive multi-voice dry/send saturation remain outside this
change. Analog mute transients and hardware traces are unverified. All other
inherited scopes/limitations remain. No existing partial Issue #3 work, full
hardware accuracy or commercial compatibility is claimed. No post-merge
verification is required. Optional physical-device and unaudited-game checks
were not requested or performed.
