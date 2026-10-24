# Issue #67: two standard joypads, manual serial reads

## Reference record (read before implementation, 2026-10-24)

[Fullsnes](https://problemkaputt.de/fullsnes.htm), sections **SNES Controllers
I/O Ports - Manual Reading**, **Automatic Reading**, **Hardware ID Codes**,
**Controller Mods / Shift Registers**, and **SNES Unpredictable Things / Open
Bus**: OUT0 connects to both ports; each input register generates its own port's
clock. The button table assigns B through R to positions 1–12 and zero to
13–16; the normal-pad ID extends those zeros with eight ones. OUT0 high holds
the first button: “shift register gets stuck on the first bit”. Joypads use
parallel-input shift registers. The open-bus table gives masks FC/E0; JOYB's grounded inputs yield bits 4–2 high.

[Anomie's Register Doc](https://raw.githubusercontent.com/gilligan/snesdev/master/docs/snes_registers.txt),
revision 1157 (2007-07-12), **4016/4017 JOYSER0/JOYSER1** (lines 861–896),
**CONTROLLERS / Generic / Open Port / Joypads** (lines 1924–1932, 1974–1992):
the shared latch is controlled by write bit 0.
A read samples the current data before the returning clock selects the next
bit, separately per port (“Clock is set back to high”). Standard pads return
`byetUDLRaxlr0000`, followed by ones until relatching. Data2 is unconnected; an unconnected line reads zero.
The register layouts are `------ca` and `---111db`, with dashes open bus.

No disagreement was found for this scope. Fullsnes describes electrical low as
CPU one, whereas Anomie qualifies latch polarity; this implementation uses CPU
register values, not electrical voltage names. Level-sensitive parallel loading
and Fullsnes' held first button imply live B while high and capture when going
low. Repeated low writes cannot reload a parallel input. Host changes while low
therefore wait for the next latch. These are protocol deductions, not measurements
of setup/hold times. Input bits 3–0 are unsupported and masked on both APIs.

## Scope and remaining limits

Only manual reads with automatic polling disabled are addressed. Reset continues
the emulator policy of releasing both inputs, without a physical warm-reset claim.
Automatic/manual shared latch/clock activity, JOY1–JOY4 routing/update visibility,
busy/start/completion/enable-edge timing, interference, physical pulse widths,
peripherals, multitaps, disconnected-device electrical behavior and frontend
multiplayer remain outside scope. Existing automatic timing conventions are
regression coverage only. Controllers remain partial: the supplied 24-category
inventory remains 21 partial, one implemented_unverified, two unimplemented,
zero verified. This does not establish commercial-game compatibility.

## Implementation and validation

`Machine::set_buttons` and the existing public first-pad bus input remain
compatible. `Machine::set_buttons2` supplies the second pad independently.
Two private shift registers share the existing latch; low reads shift only the
selected register, filling with ones. Both input APIs and captured snapshots
mask bits 3–0. Automatic polling code and its first-pad results are unchanged.

Eight original tests in `crates/snes-core/tests/joypad_serial.rs` cover:

- Every single button on each pad, all sixteen positions, eight trailing ones,
  and unsupported low input bits on both APIs.
- Literal complementary $A590/$5A60 sequences, unequal/interleaved counts with
  either port leading, and input setter independence.
- Repeated high reads and writes, live B changes while high, capture after an
  input change between the last high write and falling edge, frozen snapshots
  after changes while low, repeated low writes with other bits set, and relatch.
- FC/E0 open-bus masks, driven bits and both B values with prior bus bytes
  $00/$FF/$A5/$5A, across all 128 system banks. Alternating banks during a
  single low-latch sequence establishes shared mirror state. All remaining
  cartridge banks and both WRAM banks retain their own decoding without
  clocking or relatching controllers.
- Reset from an asserted latch releases both pads and clears serial/latch state.
- An original MIT CPU guest records 24 bits per pad in WRAM through ordinary
  instructions and natural scheduling. The host supplies only ROM and buttons;
  instruction/frame deadlines are explicit. See [provenance](../tests/roms/PROVENANCE.md#issue-67-original-manual-two-pad-serial-guest).

Existing DMA/HDMA unit controls now include both serial registers and assert
both shift-register snapshots survive excluded reads. This includes general
DMA, descriptors, direct/indirect payloads, pointer fetches and subsequent
metadata reloads. The existing DMA CPU guest additionally records independent
CPU reads after excluded accesses to both ports. Existing assertions remain.
The excluded-read byte remains the zero approximation documented for #25;
this is not verification of DMA open-bus electrical values or the Fullsnes
qualification about $4017's grounded inputs during DMA.

## Baseline failure

A detached worktree at `0f6e89d38055ccfed22ffff998eb31760bfcbff7` received only
`machine()` and `released_second_pad_trails_with_ones` (the file prefix before
`// MSB-first`). It uses no new API. Executed:

```sh
git worktree add --detach .cache/issue-67/baseline 0f6e89d38055ccfed22ffff998eb31760bfcbff7
# Copy that prefix into baseline/crates/snes-core/tests/joypad_serial.rs.
cargo test --manifest-path .cache/issue-67/baseline/Cargo.toml --locked -p snes-core --test joypad_serial
```

It compiled and failed at zero-based position 16 (the seventeenth read): actual
0, expected 1. The preceding sixteen released-pad reads passed. The identical
test passes after this change, as do repeated trailing reads.

## Mandatory pre-merge gates (2026-10-24)

Native checks used Homebrew Rust 1.94.0. Tool assembly and the web build used
`RUSTC="$(rustup which rustc)"` (rustup Rust 1.99.0 with the wasm target), as
README documents for this mixed installation. Node was v24.12.0, Python 3.13.13.

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Pass |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Pass |
| `cargo test --workspace --locked` | Pass: 358 tests, zero failed/ignored |
| `cargo test --locked -p snes-core --test joypad_serial` | Pass: 8 tests |
| `cargo test --locked -p snes-core --lib machine::dma_tests` | Pass: 48 tests |
| `cargo test --locked -p snes-core --test dma_access` | Pass: 6 CPU guests |
| `python3 tools/make-fixture.py .cache/smoke.bin` then `cmp tests/roms/smoke.bin .cache/smoke.bin` | Pass: byte-identical diagnostic |
| `python3 tools/build-test-tools.py --spc` | Pass with compiler selection above; pinned ca65/ld65, bass and spcasm built |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Pass: full/basic CPU and SPC suites |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Pass: 11 grayscale bands and final shade through line 223 |
| `python3 tools/build-cleared-games.py` twice | Pass: both fresh builds match the pinned expected game hashes |
| `npm --prefix web ci` | Pass |
| `npm --prefix web run build` | Pass with compiler selection above |
| `(cd web && npx playwright install chromium)` | Pass |
| `(cd web && SNES_TEST_PORT=18085 npm test -- --grep-invert '301 seconds')` | Pass: 37 tests in 2.7 minutes, no failures/skips |

The initial Python SPC archive download timed out. Downloading that same pinned
archive with curl succeeded; its SHA-256 was verified against `toolchains.json`
before placing it in the tool cache and rerunning the documented build. No
license/hash check or source-build step was bypassed. The first browser invocation
could not bind occupied port 8080 and ran no tests; the README-supported port
override uses 18085. Test inputs, assertions, expected game results and provenance
decisions are unchanged.

Hardware outputs retain source pins and expected results: CPU full $0649 and
basic $0452 at PC $0081A2, SPC $0557 at PC $008060. The PPU fixture retains
SHA-256 `aef339e5d4a7319db31de577b253d977b45f5c45f09adfaa99a302ca300a957a`.
Both cleared builds produce Castle
`197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869` and Memory
`6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.

The unchanged mandatory browser suite passed both 18100-frame game replays;
Memory won/restarted 18 times. Castle's separate 11097-frame route visited all
five maps and completed credits (scroll 287, hash `97663f45`). Music/effect,
real Web Audio queue/drift, freeze negative control, diagnostic input/reset,
hires/Mode 7/OBJ, and Pages artifact checks also passed. The optional
user-supplied commercial/uncleared game tests named “301 seconds” were excluded
by the required filter; no mandatory check was skipped.

Detailed local command logs are ignored under `.cache/issue-67/`, hardware
reports under `.cache/hardware-tests/results.json` and
`.cache/ppu-test/results.json`, and browser evidence under `web/test-results/`.
This checked-in record preserves the results for downstream review.
No optional validation or post-merge verification is required by #67.
