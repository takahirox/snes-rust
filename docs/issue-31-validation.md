# Issue #31: HDMA frame-initialization prefetch

Implemented for [Issue #31](https://github.com/takahirox/snes-rust/issues/31)
on 2026-10-23 in the assigned worktree, starting at checkpoint
`37df444d8e56b07d7d047465396635ff0d194bed`. Its production baseline matches
`d6d878b15963e5174084473f8be5592228a1d23d`.

Issue #33 subsequently replaces the deferred row-reload behavior and the
initial-row-only prepared mask described here. Its
[row-end validation](issue-33-validation.md) records the corrected channel phases,
termination effects and old-test derivations. This document retains the original
Issue #31 results and timing limits as historical evidence.

Issue #35 subsequently separates scanline HDMA service eligibility at 1112
master clocks from HBlank/rendering at 1096. The old H=274 request described
here is historical emulator behavior, not established hardware request-edge
evidence; see the [phase reconciliation](issue-35-validation.md#evidence-and-event-terminology).

## Specification evidence and supported state transitions

Retrieved both named references before implementation. Local reference copies
remain ignored under `.cache/issue-31/`; third-party documentation is not added
to the repository. SHA-256 values match Issue #29's retrieved versions:

| Reference | Version | SHA-256 |
| --- | --- | --- |
| [Anomie's Register Doc](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt) | Revision 1157, 2007-07-12 | `19dfdf3f1c206655b8ede11bcd6dc053e36634f876819e40bb39cfb88a237dfd` |
| [Fullsnes](https://problemkaputt.de/fullsnes.htm) | Extracted from no$sns v1.5, retrieved 2026-10-23 | `3853b65a7331bf12eea431ff494a2948a1c78ef22f7e819db4b139fd5a79f6c6` |

Line numbers refer to raw text/HTML. Anomie **420C HDMAEN** (1028–1056),
**43x0–43xA** (1190–1348), and **DMA AND HDMA → HDMA** (2422–2513) separate
initialization from transfers, preserve source registers, distinguish metadata
reads from payload direction, and define line/repeat state. Fullsnes **DMA and
HDMA Start/Enable Registers** (692–720), **Channel 0..7 Registers** (725–859),
**Notes** (861–887), and **Timing H/V Events** (26984–27030) corroborate channel
priority, table/payload banks, incrementing addresses, VBlank setup and scanline-0
reload. No other emulator implementation was inspected or used as an oracle.

| Event / channel state | Supported operation and observable result | Basis |
| --- | --- | --- |
| Scanline-0 initialization | Visit enabled channels 0 through 7; copy `$43x2/3` into `$43x8/9`; preserve `$43x2/3/4/7`; disabled channels generate no reads or register changes | Anomie initial steps 1–4, 43x2–9; Fullsnes channel order/registers |
| Direct, nonzero descriptor | Read descriptor through A bus into `$43xA`, advance table address once, prepare first transfer; no payload read/write or counter decrement | Anomie initial steps 2/4 and separate transfer section |
| Indirect, nonzero descriptor | Same, then read little-endian pointer into `$43x5/6`, advancing table address twice more; `$43x4` is table bank, `$43x7` is payload bank | Anomie initial step 3, 43x5–7; Fullsnes table formats |
| Direct zero descriptor | Expose zero in `$43xA`, advance once, terminate internally; programmed `$420C` stays set | Anomie initial step 2 and termination rule; Fullsnes zero table entry |
| Indirect zero, a later enabled channel remains | Load both pointer bytes before termination; three total table reads | Derived application of Anomie's indirect-load and last-active-channel rule to initial step 3 |
| Indirect zero, no later enabled channel remains | Use zero as pointer low byte; fetch only the next table byte as high; two total table reads | Same derivation from Anomie HDMA step 5b oddity (2486–2489); Fullsnes does not independently specify this quirk |
| First HBlank | Use prepared metadata; read/write payload only now; increment direct/indirect payload address, decrement line state once | Anomie separate transfer steps 1–4; Fullsnes count/pointer always read, payload follows direction |
| Initial `$80` descriptor | First transfer occurs before decrement to `$7F`; then hold for the remaining lines | Anomie 43xA explicitly describes 128 lines without repeat; Fullsnes table entry range |
| Write `$420C` after initialization | Does not create another initialization event; supported setup is during VBlank for the next frame | Anomie 420C; Fullsnes mid-frame notes retain uncertainty |

The zero-header pointer rule is a documented derivation, not a claim that either
reference directly lists every initial-zero bus cycle. The broader indirect-load
rule is used instead of treating termination as permission to skip all reads.
Channel order means later enabled channels have not yet been examined, even if
their own descriptors will be zero. Tests cover both cases, including reverse
HDMA and table-bank wrap. All table reads retain the existing A-bus decoding,
restrictions and status-register read effects. No simultaneous B port exists
for a descriptor or pointer fetch, so a WRAM table does not cause a payload
WRAM conflict.

## Timing convention and retained approximations

Fullsnes lists initialization at H=6, V=0 and expresses horizontal positions in
dots; its refresh entry explicitly relates 40 master clocks to 10 dots.
Anomie calls this position approximate. The scheduler requests initialization
when scanline 0 crosses 24 master clocks, including the initial modeled frame,
then services it at the next bus/event boundary. Frame wrap itself no longer
requests initialization. `$420C` only changes its programmed mask.

This establishes initialization-before-payload ordering, not exact CPU revision,
subcycle or DMA alignment. Existing `hdma_read` charges eight clocks per actual
table read and advances raster/APU; no new global initialization overhead is
claimed. Anomie describes approximately 18 clocks of startup overhead; complete
overhead/alignment remains unimplemented. The retained HBlank payload request at
1096 master clocks (H=274) is earlier than the references' H=278 transfer point;
this Issue does not certify or revise that convention. Subsequent row reloads
remain at the next payload event rather than the references' end-of-previous-row
placement. Their zero-header pointer quirks remain outside this initial-row fix.

CPU guests configure in VBlank and poll its end, wait 12 NOPs, then inspect/edit
metadata well before either HBlank position. They do not depend on enabling HDMA
after the initialization point. Host scheduler tests use broad completion
margins (200/300 master clocks for initialization), not numeric hardware cycle
oracles. Tests of late enabling only assert no manufactured initialization;
arbitrary mid-frame activation/resume behavior is not certified.

## Implementation and original regression evidence

`tick` schedules the separate scanline-0 event. `hdma_events` prepares only enabled
channels through `hdma_read`, publishes line/pointer registers, handles initial
zero headers and terminates internally without changing HDMAEN. A one-byte mask
marks the prepared first row until its first payload, distinguishing an initial
`$80` header from `$80` reached by decrementing a repeated row. No payload byte
is cached. General DMA already checks HDMA between bytes; both initialization
and first HBlank use that existing preemption path.

Nine original scheduler tests use ordinary `idle`/CPU bus progression, never
setting initialization/pending flags or active/line state to manufacture their
events. They cover initial frame and wrap, pre-payload register visibility,
metadata edits with live payloads, all eight channels, ordering via IRQ
acknowledgement, disabled NMI-read controls, direct/indirect zero headers,
16-bit table/pointer wrap, direction-independent metadata reads, restricted
A addresses, status-pointer side effects, initial `$80` and no B-port latch or
WMDATA side effects. Different-channel general DMA crosses initialization and
two HBlanks for direct and indirect HDMA, both initially and at frame wrap: exactly two row transfers, final
DMA address/count, raster/frame progress and APU sample cadence are asserted.
Only one CPU instruction executes while its bus call services the whole DMA.

The existing brightness contention regression now reaches its starting line
through normal clock progression. It keeps the same three rendered shades.
Older payload-access tests intentionally retain their synthetic init/HBlank
helper, now positioned beyond the natural initialization phase to prevent a
second event; their hardware-side-effect and byte-count assertions are preserved.
No existing assertion was weakened to hide a conflicting hardware contract.

Two original in-memory CPU guests in `tests/hdma_init.rs` configure registers and
tables, synchronize through `$4212`, save prepared registers, edit metadata and
payload, read back CGRAM/source registers and write numeric WRAM results through
executed instructions. Each requires its success marker and STP within 100,000
instructions and fewer than three frame wraps. Host code only supplies the ROM
and observes results. [Provenance](../tests/roms/PROVENANCE.md#original-hdma-initialization-cpu-guests-approved)
records original MIT code, data and header; no commercial ROM or firmware was
acquired.

Before production changes, the first seven scheduler tests all failed. In
particular, pre-HBlank `$43xA` was zero instead of `$82`, metadata edits yielded
payload zero instead of live `$66`, and indirect address registers stayed zero.
Both CPU guests also failed on the baseline: each recorded
`[0,$10,$EE,$A6,$5A,$EE,0,0,0,$10,0]`. Expected direct results are
`[1,$10,$82,$A6,$5A,$81,$66,$12,0,$10,0]`; indirect results are
`[3,$10,$82,0,$12,$81,$66,$12,0,$10,0]`. Baseline production was still unchanged
when those executable failures were captured. The final nine scheduler tests
and both guests pass after correction. Logs are ignored in `.cache/issue-31/`.

During implementation the existing `$80` status-descriptor test exposed an
initial-row integration error: the payload path tried to reload an already
prepared `$80` descriptor. The prepared mask fixes that distinction; the old
expectation passes unchanged. Two additional scheduler tests then covered fresh
`$80`, direct zero and all eight channels.

## Mandatory validation

All required commands ran in the assigned worktree and passed. Workspace Rust
checks use Homebrew Rust 1.94.0. Wasm and the separately built SPC assembler use
rustup Rust 1.99.0 via README's `RUSTC="$(rustup which rustc)"` selection. No source
pins, fixture assertions, licensing decisions, deadlines or frame counts changed.

| Command | Actual result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Pass |
| `cargo test --workspace --locked` | Pass: 175 tests, zero failed/ignored; doctests pass |
| `cargo test --locked -p snes-core --lib machine::` | Pass: 68 tests, including nine new scheduler tests |
| `cargo test --locked -p snes-core --test dma_access` | Pass: six existing CPU guests |
| `cargo test --locked -p snes-core --test hdma_init` | Pass: two new CPU guests |
| `python3 tools/make-fixture.py .cache/smoke.bin` and `cmp tests/roms/smoke.bin .cache/smoke.bin` | Pass: identical |
| `RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc` | Pass: all three pinned assemblers built |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Pass: full CPU `$0649`, basic CPU `$0452`, SPC `$0557`; each 600 frames and expected terminal PC |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Pass: eleven grayscale bands, final held shade through line 223 |
| `python3 tools/build-cleared-games.py` (twice) | Pass: both fresh builds reproduce both expected hashes |
| `npm --prefix web ci` | Pass; zero reported vulnerabilities |
| `RUSTC="$(rustup which rustc)" npm --prefix web run build` | Pass |
| `(cd web && npx playwright install chromium)` | Pass |
| `SNES_TEST_PORT=18085 npm --prefix web test -- --grep-invert '301 seconds'` | Pass: all 18 tests, 2.5 minutes |

The original bounded Fullsnes download timed out partway through; the subsequent
complete download matched the recorded hash. The pinned SPC archive downloaded
slowly in the tool builder but completed and passed the unchanged hash check
`979b0fbe19830ffb49d2f187a5512044b7514914f1d289686c340ffd1d6a6e63`.
An independent 60-second download probe timed out after 4,359,752 bytes; that
partial copy was not used. A preliminary CPU-only hardware invocation also
passed, followed by the mandatory combined CPU/SPC command above. The baseline
failures and initial `$80` integration failure are described separately above;
final mandatory checks have no failures, pending checks or skips.

The browser results retain 18,100 frames (301.17 emulated seconds) and twenty
gameplay windows for each game. Castle traversed all five maps and completed
credits at frame 11,097; Memory won/restarted eighteen times. Independent music
and finite effects, gesture-activated Web Audio, queue/drift, freeze rejection,
SRAM-failure handling and the Pages subpath smoke checks passed. The documented
alternate port 18085 was selected before starting tests.

Reproduced hashes:

- Smoke: `73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb`.
- Castle: `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`.
- Memory: `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
- Licensed HDMA fixture: `aef339e5d4a7319db31de577b253d977b45f5c45f09adfaa99a302ca300a957a`.

Logs and generated evidence remain ignored under `.cache/issue-31/`,
`.cache/hardware-tests/`, `.cache/ppu-test/`, `.cache/cleared-games/` and
`web/test-results/`.

## Remaining uncertainty and inventory

DMA/HDMA remain partial. Subsequent reload placement and its zero-header effects,
complete startup/overhead/alignment, arbitrary mid-frame enabling, same-channel
DMA/HDMA contention, exact electrical open-bus values, PAL/interlace/overscan and
pixel-fetch timing remain separate work. The implementation improves initial
metadata prefetch without certifying the entire HDMA category or commercial
compatibility. Existing licensing decisions, source pins and fixture assertions
remain intact. No optional or post-merge checks are required.

All 24 inventory scopes/statuses remain unchanged: `cpu`, `bus`, `scheduler`,
`dma`, `hdma`, `controllers`, `ppu-bg`, `ppu-offset`, `ppu-mode7`, `ppu-objects`,
`ppu-color`, `ppu-ports`, `ppu-display`, `spc`, `ipl`, `dsp-noise`, `dsp-pitch`,
`dsp-envelope`, `dsp-echo`, `dsp-timing` remain partial; `dsp-voices` remains
implemented_unverified; `ppu-hires`, `ppu-raster`, `pal` remain unimplemented.
