# Issue #107: raster-progress rendering foundation

Implements the required visible-output foundation for ordinary NTSC,
non-interlaced operation. `ppu-raster` is **partial**, not hardware verified.
No emulator implementation, commercial ROM, firmware or game-specific workaround
was imported. Issue #3's tracking/delivery status is unchanged.

## Timing facts and selected convention

References consulted on 2026-10-28:

- [Fullsnes](https://problemkaputt.de/fullsnes.htm), SNES Timing / H/V Events and
  H/V counters: picture dots approximately 22–277, visible lines 1–224, HBlank
  flag at 274, HDMA eligibility at 278; PPU long dots at 323/327 differ from CPU
  timer units. These support separate picture and HBlank boundaries.
- [SNESdev Timing](https://snes.nesdev.org/wiki/Timing): the picture occupies
  master clocks 88–1112; V=0 is hidden. Ordinary lines have 1364 clocks; odd-field
  V=240 has 1360 with interlace disabled. Direct access returned HTTP 403 here;
  the search engine's indexed text was available and checked.
- [Issue #95's reconciliation](issue-95-validation.md) remains applicable:
  Fullsnes's observable counter has 340 positions with two six-clock long dots;
  the alternate four-five-clock-dot description describes another division.
  Both total 1364. All visible output precedes those long positions, so visible
  pixels use four clocks each. The short line is outside the visible picture.

**PROVISIONAL / HARDWARE UNVERIFIED:** there is no physical write-to-DAC latency
measurement in this change. Select a deterministic right-edge convention:

- Low-resolution pixel `x` occupies `[88+4*x, 92+4*x)` and is emitted when the
  right edge has elapsed. Flush that old-state output before committing a write.
- At H=91, pixel 0 is still pending and takes the new state. At H=92, pixel 0
  retains the old state and pixel 1 takes the new state. H=93 has the same split.
  No clock between consecutive commits means no output between them.
- Literal independent split vectors include H=343/344/345 → 63/64/64 old pixels
  and H=855/856/857 → 191/192/192. H=1111 leaves pixel 255 pending; H=1112 completes
  all 256. A write after completion cannot recolor that row.
- HBlank at 1096 leaves four picture pixels pending. The HDMA service threshold
  remains 1112, and service still waits for its existing CPU/DMA boundary.
- Hires/pseudo-hires emits the existing even-sub/odd-main pair together at each
  low-resolution right edge. Half-dot live effects are not claimed.

This output convention is not an observed hardware phase. CPU HTIME continues
using its separate four-clock timer units and existing IRQ offsets, including
Issue #105's provisional late-H-only suppression. No pixel-coordinate substitution
was made in IRQ scheduling.

## Implementation and supported subset

`SystemBus::tick` renders each elapsed visible span before line rollover. CPU
bus accesses, refresh holds and both four-clock DMA transfer halves already
advance that scheduler. The shared PPU-write decoder explicitly flushes elapsed
output before committing CPU, general-DMA or HDMA register changes. Large ticks
process each crossed line; a per-line pixel cursor prevents repeated rendering.
APU advancement and deferred interrupt/DMA/HDMA ordering are retained.

The existing BG/OBJ/window/palette/color-math compositor now accepts a pixel
range. Required live effects are INIDISP forced blank and brightness 0/7/15,
and COLDATA fixed-color arithmetic. Deterministic coverage includes backdrop
math and non-mosaic Mode 1 BG1 planar red/green pixels with blue addition, blank
on/off, dimming, and preservation of the prefix. Existing static Mode 0–7,
hires/pseudo-hires, offset, Mode 7, window and OBJ tests remain the regression
contract. Other registers are sampled for future composition, without a claim
about their physical latch/fetch latency.

OBJ pixels are evaluated once at the first nonblank span and reused for that
line; this is a bounded scanline cache, not an OAM pipeline. Overflow bits are
staged and published at the existing coarse HBlank boundary; static
`render_line` callers still publish immediately. The existing OBJ timing test
remains unchanged. The older HDMA test now asserts the already-emitted 251
pixels at H=1094, replacing its obsolete assumption of an untouched row there;
its later service and prior-pixel-preservation assertions remain.

Frame geometry stays packed 256×224 or 512×224 RGBA. Mid-line promotion duplicates
already emitted low-resolution pixels, including row zero, and does not erase
them. Subsequent low-resolution spans duplicate into the promoted stride.
Shrinking occurs only at the next frame's first emitted low-resolution pixel.
Blank spans write opaque black. The line cursor and OBJ cache reset on every
natural line transition; reset/cartridge replacement use the existing defaults.
No framebuffer/Wasm ABI or frontend product changes were needed.

## Original source-built guest and snapshots

The annotated [original MIT byte emitter](../crates/snes-core/tests/common/raster.rs)
builds a 32 KiB 65C816 LoROM without an assembler or external assets. It polls
HVBJOY, resets red/full brightness during HBlank, then uses NOP delays and
ordinary CPU stores for red-off, green-on, brightness 7, forced blank, unblank,
and green-off. A CPU-triggered fixed-source one-byte DMA then writes blue to
COLDATA. The only host input is the ROM. This is a diagnostic, not a game.
See [provenance](../tests/roms/PROVENANCE.md#original-raster-progress-65c816dma-guest-approved).

ROM SHA-256: `eb05c223d468723e430ea2131608891d97550fc3bf344d60fc9a73cb7b78a8b2`.
After exactly three frames, framebuffer FNV-1a32 is `6041bd2d`; the native runner's
FNV-1a64 is `ecda9c5474b272cd`. Polling phase varies across fields, so this is a
fixed-frame software snapshot, not a hardware oracle. Instruction timings use
the existing bus scheduler: each slow-ROM NOP is 8 fetch + 6 idle clocks, and
LDA-immediate/STA-absolute is 16 + 30 clocks; refresh and DMA add their existing
elapsed clocks. Native and Wasm independently execute the same guest.

Each row below has these eight successive RGBA colors: red, black, green,
`[0,119,0,255]`, black, green, black, blue (other nonzero channels are 255).
These bounded transition traces are asserted along with **every pixel** of each
row, including both neighbors of every edge:

| Framebuffer row | Segment starts (x) |
| ---: | --- |
| 0 | 0, 40, 51, 77, 102, 138, 149, 167 |
| 1 | 0, 47, 59, 84, 110, 145, 157, 174 |
| 100 | 0, 42, 53, 79, 104, 140, 151, 169 |
| 223 | 0, 46, 58, 83, 109, 144, 156, 173 |

Negative control: temporarily compiling the unchanged upstream `a0435c7`
`machine.rs` and `ppu.rs` against the new native guest test exits 101. Its row 0
trace is only `[(0, [0,0,255,255])]`, instead of the eight required segments.
The current files were restored immediately afterward. This proves the guest
fails for the original whole-line sampling defect, rather than a missing API.

The native runner also emitted `.cache/raster/frame3.ppm` and a bounded 12-entry
CPU trace. Chromium checks the exact frame/hash twice after reset, then actual
UI playback/canvas pixels inside the bands. It saves JSON snapshots and a
screenshot in the ignored Playwright results directory. No ROM enters
`web/dist` or GitHub Pages; the artifact smoke test enforces this boundary.

## Executed validation

All commands below were run in the assigned worktree on 2026-10-28. Rust native
builds use Homebrew rustc 1.94; Wasm and the optional SPC assembler use the
installed rustup compiler via `RUSTC="$(rustup which rustc)"`. The SPC build script
sets its documented experimental flag only for that separate build-time tool.

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
cargo test --workspace --locked
cargo test --locked -p snes-core --lib machine::raster_tests
cargo test --locked -p snes-core --test ppu_raster -- --nocapture
cargo run --locked -p snes-core --example raster-fixture -- .cache/raster
cargo run --release --locked -p snes-core --bin snes-run -- \
  .cache/raster/raster.sfc --frames 3 --timeout 10 --expect 0x30=0x6b \
  --ppm .cache/raster/frame3.ppm --trace 12
python3 tools/make-fixture.py .cache/smoke.bin
cmp tests/roms/smoke.bin .cache/smoke.bin
RUSTC="$(rustup which rustc)" python3 tools/build-test-tools.py --spc
python3 tools/hardware-tests.py \
  --ca65 "$PWD/.cache/tools/cc65/bin/ca65" \
  --ld65 "$PWD/.cache/tools/cc65/bin/ld65" \
  --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"
python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"
python3 tools/build-cleared-games.py
python3 tools/build-cleared-games.py
npm --prefix web ci
RUSTC="$(rustup which rustc)" npm --prefix web run build
npx --prefix web playwright install chromium
SNES_TEST_PORT=18107 npm --prefix web test -- tests/raster.spec.js
SNES_TEST_PORT=18107 npm --prefix web test -- --grep-invert '301 seconds'
```

- Formatting and warning-denying Clippy: pass.
- Eight focused raster unit tests plus native guest snapshot: pass. Independent
  literal clocks include left/right and picture edges, consecutive commits,
  multiple changes, brightness/blank, HBlank tail, line/VBlank/frame transitions,
  Mode 1 interleaved colors, mid-row width promotion and next-frame shrinking.
- Whole, irregular `[3,127,2,1367,19]`, and single-clock advances compare complete
  frames, audio samples, clock/field/line state across multiple fields including
  the odd short line. A test-only scheduler quantum of 1/3/17/whole also compares
  actual guest CPU/DMA output and natural CPU-serviced HDMA, with unchanged
  framebuffer, audio and execution state. No production stepping API was added.
- Native fixture runner: pass, three frames within the ten-second timeout,
  marker `$30=$6B`, 3194 interleaved audio samples (silent, as expected).
- Source-built CPU full/basic and SPC700: pass, 600 frames each; terminal tests
  `$0649/$0452/$0557` and PCs `$0081A2/$0081A2/$008060` respectively.
- Source-built HDMA: pass, 11 grayscale bands with final shade through row 223.
- Smoke fixture rebuild and byte comparison: pass.
- Both cleared-game builds: pass twice, unchanged ROM SHA-256 values:
  Castle `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`;
  Memory `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
- Wasm build and focused real Chromium raster check: pass.
- Workspace: **518 tests passed**, including all existing IRQ, PPU access, Mode 0–7,
  256/512 geometry, DMA/HDMA, CPU/SPC and DSP regressions.
- Complete real Chromium suite: **38 passed** in 3.7 minutes, including the new
  raster check, Castle's five-minute replay and five-map/credits route, Memory's
  five-minute playthrough, finite effects, gesture-activated music/audio queue
  checks, existing hires/Mode 7/OBJ checks, and the actual Pages `/snes-rust/`
  artifact smoke check. The unaudited external-game `301 seconds` cases were
  explicitly excluded by the documented command; neither cleared-game target
  was skipped. No optional external game or hardware capture was run.

The first browser attempt used occupied port 8080 and failed to start its local
server. Re-running on the documented alternate-port mechanism succeeded. The
initial workspace run found the obsolete HDMA untouched-row assertion and the
OBJ flag-publication regression; both were corrected as described above, then
the workspace was rerun. No test was disabled to hide either failure.

## Accuracy limits and follow-up targets

Pixel emission is a deterministic output policy. Physical register-to-DAC
latency, within-dot hires effects, rendering-owned VRAM/CGRAM/OAM behavior and
arbitration, OAM evaluation/fetch timing, scroll/tilemap fetch latency, mosaic and
offset interactions, interlace/overscan/PAL and enhancement chips remain outside
this foundation. Mid-line OBJ memory changes are not supported by the scanline
cache. APU still advances before raster processing within a tick; that existing
cross-device ordering is not made fully synchronous here. IRQ software policies
remain as documented for Issues #95–105.

No physical hardware or reference-emulator captures were performed. A future
hardware diagnostic should measure write-to-output latency and phase before
promoting this convention. `ppu-raster` joins the other 22 partial categories;
`pal` remains unimplemented and no category is fully verified. This scoped Issue
requires no post-merge check; normal Pages deployment remains the existing flow.
