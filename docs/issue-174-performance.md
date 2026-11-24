# SNES performance checkpoint for Issue #174

This checkpoint benchmarks the production Rust core against libretro Snes9x
on the same host and measures several independent optimization steps. The
competitive target is **80% of Snes9x's same-platform field throughput**,
or at most 1.25 times its milliseconds per field, on each comparable
workload. The initiative remains open. The browser playback results and
the uncapped comparisons answer different questions.

## Reference and provenance

- **Snes9x** is the practical throughput reference. The same pinned
  libretro core runs natively and under Emscripten with video and audio
  enabled. It covers the ordinary NTSC LoROM/HiROM mapper, Mode 5 and Mode 7
  cases measured here. Its PPU latches per-scanline state and flushes batches
  of lines at selected register writes. The tile renderer caches planar
  conversions and uses dedicated draw routines. Its CPU uses width-specific
  instruction handlers, mapped memory and a next-horizontal-event scheduler.
  The sound core runs the SMP and DSP and delivers resampled stereo to the
  host. [Libretro options](https://docs.libretro.com/library/snes9x/),
  [pinned CPU scheduler](https://github.com/libretro/snes9x/blob/204ea1c0b851f960cbf223075eb1c25641cb483c/cpuexec.cpp),
  [pinned renderer](https://github.com/libretro/snes9x/blob/204ea1c0b851f960cbf223075eb1c25641cb483c/gfx.cpp),
  [pinned tile routines](https://github.com/libretro/snes9x/blob/204ea1c0b851f960cbf223075eb1c25641cb483c/tile.c).
- **ares** provides a useful accuracy-oriented architecture comparison. The
  native desktop builds support SNES and the core uses cooperative
  component scheduling. **bsnes** provides native and libretro builds with
  scanline and more detailed PPU options. The timed runs used Snes9x.
  The matched Snes9x libretro build and output settings supply
  the common target on both platforms. [ares project](https://github.com/ares-emulator/ares),
  [bsnes core options and license](https://docs.libretro.com/library/bsnes/).
- Snes9x is subject to its noncommercial license, and bsnes is GPLv3.
  ares licenses its own code under ISC and lists separate terms for third-party
  components. Snes9x builds stay in the ignored local cache. The MIT
  project contains a local benchmark host that uses the public libretro API
  and independently implemented optimizations of the existing core.
  [Snes9x license](https://github.com/libretro/snes9x/blob/204ea1c0b851f960cbf223075eb1c25641cb483c/LICENSE),
  [ares license](https://github.com/ares-emulator/ares/blob/master/LICENSE).

The documented optional paths `GOTHICVANIA_ROM` and `CASTLE_ROM` were unset.
A scoped search of this worktree and the related `snes-rust` and `snes-roms`
project directories found the two checked-in originals and no additional
standalone game images to use. Castle is rebuilt locally from the audited
sources and assets. The Mode 5 and HiROM Mode 7 diagnostics use this
repository's original fixtures. The HDMA workload uses the source-built,
zlib-licensed timing demonstration. The HiROM variant moves the existing Mode 7
program and vectors to the high half of a 64 KiB image, updates the mapping,
size and checksum, and runs the same program at bank zero, $8000. Reports
contain hashes and measurements. [Bundled demo provenance](../demos/NOTICE.md),
[cleared-game audit](../tests/roms/CLEARED-GAMES.md),
[HDMA provenance](../tests/roms/PROVENANCE.md#undisbelieversnes-test-roms-hdma-timing-demonstration).

## Method

The host was an Apple M4 Max (arm64, 64 GiB) running macOS 27.2.1 (26B131).
Rust 1.99.1 built the release example and production Wasm with the workspace
release profile (opt-level 3, LTO, one codegen unit). Snes9x libretro
`204ea1c0b851f960cbf223075eb1c25641cb483c` (2026-11-17) used its
Makefile's default optimized release flags, including `-O3 -DNDEBUG`.
Native used Apple clang 21.0.0 and Wasm used Emscripten 6.0.13-git.
Node 26.12.1 ran both Wasm builds. Python 3.13.13 scheduled child processes.
The matched uncapped trials were rerun on 2026-11-24. Each trial records
its build's `source_revision`.

| Build | Source revision |
| --- | --- |
| Rust baseline | `416b6c4097c447cd2f2a58298c153ab6295f243a` |
| Rust final | `7a8bc5351a15092b63e11617ed5d5dcea4e98497` (PR #175 after the STAT77 fix) |
| Snes9x reference | `204ea1c0b851f960cbf223075eb1c25641cb483c` |

The final binaries were built from the review checkpoint `b22a9b9`,
whose Git tree matches the listed PR revision. The STAT77 fix composes
elapsed pixels before reads during the HBlank picture tail so that delayed
OBJ overflow remains visible there. Both native Rust builds use the same
benchmark example. The browser comparison uses the same playback and audio
probes for both builds. The final playback build used the production sources
at PR revision `a22292320927250abb252ceb54558d1d310bfbf4`, which match the
final uncapped build.

The drivers start a fresh guest for each trial, use controller masks from
[the workload manifest](../tools/performance/workloads.json) from reset,
and time 600 frame calls after either 120 or 240 warm-up calls in the
uncapped suite. The masks are zero-based, start inclusive and end exclusive.
Castle uses the existing recorded route. The diagnostic workloads use idle
input while their CPU, raster and APU keep running. The timed window
includes emulation and delivery of video/audio buffers for every field.
The Rust paths produce packed RGBA (256 or 512 by 224), while Snes9x sends
RGB565 and uses its normal overscan crop to 224 lines. Snes9x produces
about 32,040 stereo pairs per guest second, compared with Rust's 32,000.
Reference calls count the callback buffers and Rust moves out its audio
vector. There is no vsync, audio sync, frameskip, pause, or frontend filter
in the throughput runs. Snes9x's CPU/APU/PPU timing, power-on state and
RGB565 conversion differ. In particular, its scanline renderer has
different raster-write granularity from Rust's current model.

Rust's native driver confirms the master clocks, CPU instruction count
and 600 fields in the timed window. Wasm calls the same production
`run_frame` path using the same input. The suite checks per-engine
determinism across backends and revisions using framebuffer geometry
and digests, stereo frame counts, WRAM and, for Rust, float audio and SRAM
digests. Digests are sampled every 120 measured calls outside the timing
windows. The reference had an extra video callback during initialization
on the three games. Both engines generated 600 fields' worth of audio
in each timed window.

For a cross-emulator sanity check at the initial optimization checkpoint
(`3fb053e`), native checkpoints were compared pixel by pixel after RGB565
expansion into 8-bit channels. The
[scene checks](performance/issue-174-scenes.csv) record both the mean
absolute channel error and the share of channels within eight levels. Both demos changed through the
scripted inputs. In the six checked snapshots per demo, over 99.99% of
Shionari channels and every Yonaki channel were within eight levels
(0–255) of the other engine. Castle was
in map 0, gameplay state 0 in both engines at the six sampled checkpoints
from reset through field call 840. Its framebuffer agreement within eight
levels ranged from 89% to 96% and its scroll/sprite positions diverged
in parts of the replay. Castle is a gameplay workload with silent audio
buffers in both engines. Both demos generated nonzero audio. The Mode 5
fixture reached 512×224 and its compared RGB snapshots agreed at all
sampled locations within the same tolerance, as did the sampled HiROM
Mode 7 and HDMA images. The initially tried HiROM conversion of the repository
smoke ROM stalled in the reference during its IPL handshake. It was
excluded as a throughput comparator and the HiROM Mode 7 workload was
used for its verifiable output.

## Reproduce

From the repository root on macOS. Use the ordinary README tool and
Chromium setup for the cleared games and browser. A Unix Python with
`wait4` collects child CPU time, peak RSS and wall time. Snes9x is built
locally under its own license.

```sh
mkdir -p .cache/performance
perf="$PWD/.cache/performance"
rev=204ea1c0b851f960cbf223075eb1c25641cb483c
git clone https://github.com/libretro/snes9x.git "$perf/snes9x"
git -C "$perf/snes9x" checkout --detach "$rev"
make -C "$perf/snes9x/libretro" -j8
core="$perf/snes9x/libretro/snes9x_libretro.dylib"
install_name_tool -id @rpath/snes9x_libretro.dylib "$core"
codesign --force --sign - "$core"
clang -std=c11 -O3 -Wall -Wextra -Werror -dynamiclib \
  -I"$perf/snes9x/libretro" tools/performance/libretro-host.c "$core" \
  -Wl,-rpath,"$perf/snes9x/libretro" -o "$perf/reference.dylib"

git clone --shared "$perf/snes9x" "$perf/snes9x-wasm"
git -C "$perf/snes9x-wasm" checkout --detach "$rev"
wasm_core="$perf/snes9x-wasm/libretro"
make -C "$wasm_core" -j8 platform=emscripten CC=emcc CXX=em++ AR=emar \
  STATIC_LINKING=0 STATIC_LINKING_LINK=1
cp "$wasm_core/snes9x_libretro_emscripten.bc" "$perf/snes9x.a"
emcc -std=c11 -O3 -Wall -Wextra -Werror -I"$wasm_core" \
  -c tools/performance/libretro-host.c -o "$perf/host.o"
exports=_malloc,_free,_bench_load,_bench_unload,_bench_frame
exports="$exports,_bench_video_frames,_bench_audio_frames,_bench_width,_bench_height"
exports="$exports,_bench_video,_bench_wram,_bench_audio_energy"
em++ -O3 "$perf/host.o" "$perf/snes9x.a" -sMODULARIZE -sALLOW_MEMORY_GROWTH \
  -sEXPORTED_RUNTIME_METHODS=HEAPU8 -sEXPORTED_FUNCTIONS="$exports" \
  -o "$perf/reference.mjs"
```

```sh
export RUSTC="$(rustup which rustc --toolchain 1.99.1)"
export RUSTDOC="$(rustup which rustdoc --toolchain 1.99.1)"
mkdir -p "$perf/source-baseline"
git archive 416b6c4097c447cd2f2a58298c153ab6295f243a | tar -x -C "$perf/source-baseline"
cp crates/snes-core/examples/benchmark.rs "$perf/source-baseline/crates/snes-core/examples/"
(
  cd "$perf/source-baseline"
  cargo build --release --locked -p snes-core --example benchmark
  npm --prefix web run build
)
cargo build --release --locked -p snes-core --example benchmark
npm --prefix web run build
python3 tools/build-test-tools.py
python3 tools/build-cleared-games.py
python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"
cargo run --locked -p snes-core --example hires-fixture -- .cache/hires
cargo run --locked -p snes-core --example mode7-transform-fixture -- \
  "$perf/mode7-hirom" --hirom
python3 tools/performance/suite.py \
  --native baseline="$perf/source-baseline/target/release/examples/benchmark" \
  --native final=target/release/examples/benchmark --native-ref "$perf/reference.dylib" \
  --wasm baseline="$perf/source-baseline/web/dist/snes_wasm.wasm" \
  --wasm final=web/dist/snes_wasm.wasm --wasm-ref "$perf/reference.mjs" \
  --output "$perf/throughput.jsonl"
```

```sh
npm ci --prefix web
npx --prefix web playwright install chromium
node --test tools/performance/browser-{audio,playback}.test.mjs
export SNES_BENCH_METAL=1
run_playback() {
  workload=$1 trials=$2 device=$3
  node tools/performance/browser.mjs "$perf/source-baseline/web/dist" \
    "$workload" "$trials" "$device" > "$perf/browser-baseline-$workload-$device.jsonl" &&
  node tools/performance/browser.mjs web/dist \
    "$workload" "$trials" "$device" > "$perf/browser-final-$workload-$device.jsonl"
}
run_playback shionari 2 desktop &&
run_playback yonaki 3 desktop &&
run_playback hires 3 desktop &&
run_playback yonaki 2 mobile
```

For native profiling, run the benchmark with a longer replay starting at the
same reset and sample the child with macOS `sample PID 2 1 -file PROFILE`.
For Wasm, use Node `--cpu-prof` and
`tools/performance/wasm.mjs rust WASM ROM MASKS WARMUP FRAMES`, where
`MASKS` is one decimal SNES mask per line. Gather profiles separately from
the timed runs. Use distinct output filenames for each experimental build.

## Results

The [uncapped trial records](performance/issue-174-throughput.jsonl) include
all 108 trials and their deterministic output checkpoints. Every trial
reproduced the ROM hash and output checkpoints published before the STAT77
fix. The [browser trial records](performance/issue-174-browser.csv) include
40 measured playback windows.

### Uncapped throughput

Medians of three trials in measured fields per second, with the trial range
in parentheses. The final column gives final milliseconds per field, multiple
of 60.10 Hz, and throughput as a percent of same-platform Snes9x.

| Workload | Build | Baseline | Final | Snes9x | Final ms / field · NTSC speed · ref share |
| --- | --- | ---: | ---: | ---: | ---: |
| shionari | native | 301 (267–306) | 471 (450–492) | 4463 (4321–4562) | 2.12 · 7.8× · 10.6% |
| shionari | wasm | 255 (238–264) | 419 (400–432) | 2775 (2774–2989) | 2.39 · 7.0× · 15.1% |
| yonaki | native | 190 (132–277) | 308 (269–351) | 2113 (2067–2382) | 3.25 · 5.1× · 14.6% |
| yonaki | wasm | 145 (142–175) | 233 (219–256) | 1268 (1217–1453) | 4.30 · 3.9× · 18.4% |
| castle | native | 227 (154–233) | 339 (339–406) | 3512 (3125–3901) | 2.95 · 5.6× · 9.7% |
| castle | wasm | 185 (182–185) | 297 (200–325) | 1860 (1398–2079) | 3.37 · 4.9× · 15.9% |
| hires | native | 162 (161–198) | 272 (265–312) | 2486 (2472–2545) | 3.68 · 4.5× · 10.9% |
| hires | wasm | 147 (87–169) | 286 (285–290) | 1818 (1759–2099) | 3.50 · 4.8× · 15.7% |
| mode7 | native | 348 (323–349) | 430 (408–432) | 4745 (4508–4778) | 2.32 · 7.2× · 9.1% |
| mode7 | wasm | 379 (358–380) | 560 (548–562) | 3207 (3113–3234) | 1.79 · 9.3× · 17.5% |
| hdma | native | 461 (457–469) | 709 (708–717) | 5257 (5188–5302) | 1.41 · 11.8× · 13.5% |
| hdma | wasm | 369 (367–369) | 649 (644–652) | 3559 (3538–3634) | 1.54 · 10.8× · 18.2% |

The final build reached 9.1–14.6% of Snes9x's median throughput in native
trials and 15.1–18.4% in Wasm trials. All six workloads remain below the
80% target on both platforms.

The reported rates exclude warm-up, checkpoint hashing, ROM loading and
process startup. Whole-process CPU, wall time and peak RSS include those
costs. The measurements capture the host runtimes as well as each core,
including Python for native Snes9x and Node for the Wasm builds.

| Build | CPU ms for 720–840 calls, across workloads | Peak RSS MiB | ROM load ms | Wasm initialization ms |
| --- | ---: | ---: | ---: | ---: |
| native-baseline | 1542–4324 | 2.7–10.8 | 0.01–0.32 | — |
| native-final | 1010–2645 | 2.7–10.7 | 0.01–0.54 | — |
| native-ref | 167–401 | 41.3–46.0 | 1.33–27.73 | — |
| wasm-baseline | 2026–4476 | 55.2–71.2 | 0.53–1.30 | 0.48–2.42 |
| wasm-final | 1238–2677 | 55.8–69.8 | 0.55–1.32 | 0.48–1.15 |
| wasm-ref | 595–1089 | 213.9–286.9 | 7.02–64.18 | 5.91–36.92 |

SHA-256 of each measured workload

- shionari, LoRom, `1132c59cfba4cc7b0c1a7d7957b4b5de84c6038e0e061ca2e6372f8b8b4a01ca`
- yonaki, LoRom, `639cbdd93e2fc41befd6ec2398bbb369f8bc09c88eb64b77630c8c7f2cbe79fa`
- castle, LoRom, `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`
- hires, LoRom, `137c8d63c3ee3a22a645421a7be4b2430b0096ccc6f50f8bafad4668d97661b2`
- mode7, HiRom, `561895398c94ede8b656b4b736cb423bc3d4b4fe580eb47c870a644cff46626a`
- hdma, LoRom, `aef339e5d4a7319db31de577b253d977b45f5c45f09adfaa99a302ca300a957a`

### Browser playback

The playback trials were rerun on 2026-11-24. Chromium 149.0.7827.55 used
ANGLE Metal on the M4 Max. Desktop viewport was 1280×800 at DPR 2.
The emulated mobile runs used a 390×844 viewport at DPR 3
and 4× CDP CPU throttling on the same host, with the same GPU. Timing starts
when the animation callback that reaches the warm-up target finishes. It ends
when the first callback with at least 600 measured calls finishes. Catch-up
callbacks can run up to three fields and present them together. Keeping the
callbacks intact can add up to two fields at each boundary. The trial records
give the actual warm-up and measured field counts. Fields beyond the scripted
replay use idle input. The measured callbacks include framebuffer upload,
Pixel/CRT presentation and Web Audio scheduling. Each counted field must have
a matching core timing and scheduled audio buffer. Of the 40 trials, 36
measured 600 fields, three measured 601 and one measured 602. Audio buffer
counts matched field counts in every trial.

The trial records include the requested mode, the active mode at both window
boundaries and all mode changes through the final measured callback. Each
change records whether it preceded the window, how many fields had run
relative to the end of warm-up, and the elapsed milliseconds from that same
boundary. A fallback before measurement gives a Pixel window. A change during
measurement gives a mixed window, including when it occurs in the final
measured callback. The mode and display status freeze at the closing boundary.

The probe follows the audio queue through starts and stops, including
buffers scheduled during warm-up. A stop drops a queued buffer or
shortens an active buffer to the stop time. A restart scheduled during
the window counts as an audio gap if it leaves more than 1 ms between
the effective end of the prior audio and the new start. A gap is late
when the new start is scheduled after that effective end on the Web
Audio clock. The trial records include canceled buffers and the total
duration of detected gaps. These are Web Audio scheduling measurements.
The physical output device was outside the measurement.

Rows group trials by requested and measured mode and give median rates
and percentile metrics across repeats. Counts are the sums across repeats.
A render callback with guest progress counts as late when it arrives more
than 1.1 times the nominal 16.64 ms period after the previous such callback.
The first arrival uses the last warm-up callback. The work threshold uses
the nominal period.

| Device / workload | Build | Mode | Guest fields/s | Work p50/p95/p99 ms | Arrival p50/p95/p99 ms | Over-budget work / late arrivals | Late audio / all gaps | Total audio gap ms |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: | ---: |
| desktop / shionari | baseline | pixel | 60.1 | 3.9/4.1/4.1 | 16.7/17.9/25.0 | 0 / 32 | 0 / 0 | 0 |
| desktop / shionari | baseline | crt | 60.1 | 4.1/4.5/4.6 | 16.7/17.8/24.9 | 0 / 30 | 0 / 0 | 0 |
| desktop / shionari | final | pixel | 60.1 | 2.4/2.6/2.8 | 16.7/18.0/24.2 | 0 / 26 | 0 / 0 | 0 |
| desktop / shionari | final | crt | 60.1 | 2.9/3.3/3.4 | 16.7/18.5/25.2 | 0 / 70 | 0 / 0 | 0 |
| desktop / yonaki | baseline | pixel | 60.1 | 4.2/4.4/4.5 | 16.7/18.3/25.0 | 0 / 91 | 0 / 0 | 0 |
| desktop / yonaki | baseline | crt | 60.1 | 4.7/4.8/5.0 | 16.7/18.6/25.1 | 0 / 119 | 0 / 0 | 0 |
| desktop / yonaki | final | pixel | 60.1 | 2.6/2.8/2.9 | 16.7/18.1/25.0 | 0 / 64 | 0 / 0 | 0 |
| desktop / yonaki | final | crt | 60.1 | 3.0/3.1/3.2 | 16.7/18.2/25.3 | 0 / 68 | 0 / 0 | 0 |
| desktop / hires | baseline | pixel | 60.1 | 4.8/5.0/5.1 | 16.7/18.1/25.2 | 0 / 60 | 0 / 0 | 0 |
| desktop / hires | baseline | crt | 60.1 | 5.2/5.4/5.5 | 16.7/18.0/25.3 | 0 / 49 | 0 / 0 | 0 |
| desktop / hires | final | pixel | 60.1 | 2.8/2.9/3.0 | 16.7/18.2/25.2 | 0 / 67 | 0 / 0 | 0 |
| desktop / hires | final | crt | 60.1 | 3.2/3.3/3.3 | 16.7/18.1/25.2 | 0 / 49 | 0 / 0 | 0 |
| mobile / yonaki | baseline | pixel | 58.6 | 33.6/51.4/52.0 | 32.7/50.9/58.4 | 450 / 410 | 10 / 10 | 410 |
| mobile / yonaki | baseline | mixed CRT → Pixel | 58.5 | 34.4/51.5/52.0 | 37.7/51.7/58.1 | 501 / 501 | 11 / 11 | 458 |
| mobile / yonaki | final | pixel | 60.1 | 10.5/11.2/11.8 | 16.6/18.4/25.1 | 0 / 71 | 0 / 0 | 0 |
| mobile / yonaki | final | crt | 60.1 | 11.3/12.3/12.6 | 16.6/23.9/25.2 | 0 / 105 | 0 / 0 | 0 |

The four throttled baseline trials recorded 21 late gaps totaling about
868 ms. Both baseline CRT trials exceeded the frame budget and switched to
Pixel after 121 measured fields, 2.03–2.04 s into each window. The final
build sustained about 60.1 fields/s in both modes under throttling.
Every other trial recorded zero audio gaps. The live trials recorded
zero canceled buffers, so the controlled replay supplies the evidence
for the probe's cancellation handling.

The browser evidence also records scheduled buffers, guest calls, main-thread
CPU time, JS heap estimates and launch latency. The opt-in driver records
playback metrics inside the callback boundaries and samples CPU and heap
separately via CDP around that interval. A real low-power device was outside
the measured set.

### Optimization sequence

The [iteration measurements](performance/issue-174-iterations.csv) retain
the original trials through the initial optimization checkpoint
(`3fb053e`), before the STAT77 fix. The host had concurrent system activity,
and wall-time ranges were wide. Whole-process CPU time provides a second view
for variants of the same engine and driver. It includes warm-up and evidence
generation, so it is reported separately from steady field throughput.

1. **Borrow the OBJ scanline.** Native sampling put 226 Shionari leaf
   samples in memmove. Raster composition was copying 256 sprite slots for
   many short spans. Borrowing the stored line removed memmove from the
   leading symbols. Shionari's median native child CPU time dropped from
   3,058 to 2,564 ms.
2. **Share composition between hires phases.** The renderer resolves the
   main and sub screens once per full dot, including distinct Mode 5/6
   source phases, and leaves an unused low-res subscreen unevaluated.
   The Mode 5 fixture's native child CPU time dropped from 3,764 to
   2,932 ms and Wasm from 4,309 to 3,270 ms. Non-hires benefits varied.
3. **Batch raster work during complete-field runs.** Draw elapsed pixels
   before CPU/DMA/HDMA PPU writes and before OAMDATA reads that change
   priority rotation. Compose at HBlank to publish STAT77, and at
   the picture end to handle the four trailing pixels. A return from
   `run_frame` exposes all elapsed output, including on error.
   Instruction stepping still exposes intermediate pixels. Shionari's
   native child CPU time went from 2,508 to 2,374 ms, and Wasm from
   3,236 to 2,955 ms.
4. **Skip idle scheduler and timer bookkeeping.** The CPU calls the DMA
   event handler when a transfer or initialization is pending. Each SPC
   timer's continuously running divider is a phase of the existing SPC
   clock counter. The change preserves timer stage equality, enable
   semantics and DSP order. The paired trial set put Shionari's
   child CPU time at 2,511 → 2,400 ms native and 3,093 → 2,893 ms Wasm.
5. **Reuse the current planar tile row.** Each background remembers its
   current resolved source row within a span. This saves repeated map and
   plane reads while retaining offset, mosaic, flips, priorities and
   direct color. The cache lifetime ends with the span. Shionari and
   Yonaki child CPU times improved by about 6–7% native and 11% Wasm in
   the paired trial set. Native Castle and Mode 7 incurred about 2%
   and 5% extra whole-process CPU time against the previous stage,
   respectively. Both workloads are listed separately in the endpoint
   comparison. A regression test directly checks all eight plane bits
   and horizontal flips.

The profiling-only runner used frame hashing and independent diagnostics.
Throughput uses the production frame loop, timing each frame before
checkpoint hashing. An exploratory desktop run used software WebGL
(SwiftShader) with a lower and more variable CRT presentation cadence.
The repeated measurements in the browser table use hardware Metal.
The 4× results use that same GPU with CPU throttling enabled.

### Remaining material gaps

The [profile counts](performance/issue-174-profiles.csv) include native
leaf samples from macOS `sample` and V8 leaf samples from Node
`--cpu-prof` at the initial optimization checkpoint. The remaining
renderer, bus/DMA and APU costs appear in both backends. In those Wasm
profiles the raster function accounted
for about 51% of sampled Shionari time, 54% of Yonaki, 50% of Castle,
62% of Mode 5, 44% of HiROM Mode 7 and 37% of HDMA. In the HDMA
run, bus ticking and the HDMA handler contributed another 28%.
Inlining rolls layer fetch and
composition into that function. Native Snes9x samples put Shionari in
the CPU main loop, SMP/DSP and tile draw routines. On Yonaki, Snes9x's
offset and clipped tile routines dominated the sample. Rust still
processes display selection, windows and compositing per dot in straight
Rust, while Snes9x amortizes background setup over tiles and batches
scanlines until selected writes.

The ceiling imposed by the remaining costs is more important than the
speed of any individual pixel routine. Even eliminating half of
Yonaki's sampled Wasm raster cost gives a simple Amdahl estimate of
roughly 1.4× overall speedup, far short of the required per-workload
multiplier. The native profiles also show nontrivial bus, DMA event and
APU cost on the silent diagnostics, where video and timing still run.
The evidence supports this remaining work within Issue #174.

- **Prioritize event-driven WAI.** Profile time spent in six-clock CPU
  idle intervals for both demos, Castle and the Mode 7 diagnostic.
  Experiment with advancing only up to the next wake-up, math completion,
  refresh or DMA boundary. Compare exact CPU recognition and APU
  ordering with eager stepping through the existing IRQ, HDMA and SPC
  diagnostics before measuring throughput again.
- **Keep dot-sensitive composition while expanding tile work.** Profile
  decoded tile access and color math independently with native and
  Wasm source maps. For Mode 5 and Yonaki, try expanding per-layer runs
  before final compositing, and test invalidation on midline register and
  VRAM writes. For the HiROM outlier, try computing Mode 7's invariant
  origin once per span while retaining the current signed arithmetic
  and per-dot matrix products.
- **Measure browser scheduling at the margin.** Repeat with the same
  fixed replay and hardware GPU on a physical low-power device. Track
  Web Audio lead, arrival misses and CRT fallback separately. Include a
  longer playable HiROM guest when an authorized one is available.
- **Keep the reference and target stable.** Treat accuracy differences
  as measured comparability limits. This checkpoint demonstrates a
  residual architectural cost and provides next experiments. It does
  not establish an intrinsic performance floor.

## Validation and status

After the STAT77 fix, `cargo fmt --all -- --check`,
`cargo clippy --workspace --all-targets --locked -- -D warnings`, and all
522 tests in `cargo test --workspace --locked` passed. The source-built
eleven-band HDMA gate passed on the same revision. The benchmark probe
was also checked with a controlled replay that forced catch-up at both
boundaries and verified that the samples stay fixed after the final callback.

The audio probe regression uses Chromium's offline audio renderer. It
stops one active and eight queued buffers and schedules a restart 40 ms
ahead on the same clock tick. The rendered waveform contains 1,280
silent samples at 32 kHz and the probe reports one gap of 40 ms. A
second restart after the replacement buffer expires also registers as
late.

The playback probe regression uses the production app in Chromium. WebGL
unavailability before warm-up gives a Pixel window. A context-loss event
inside the window gives a mixed window with a recorded field and time offset.
The same event after window close leaves the completed trial labeled CRT.
The production Wasm build, Rust workspace checks and 11 focused Chromium
checks for CRT fallback and catch-up, queued audio and the Pages subpath
passed after the display probe fix.

The focused Chromium run passed seven checks, covering catch-up, Mode 5
and Mode 6, the local Pages subpath, and both demo replays. The eighth
check, live display selection, timed out expecting CRT after the keyboard
shortcut. It passed on an isolated rerun with the same production build
and Playwright configuration.

After the audio probe fix, 83 of 84 focused Chromium checks passed on
the first run, covering emulator playback, reset, volume, display and
the local Pages subpath. The clean-storage CRT startup check saw Pixel
on the first run and passed on an isolated rerun.

### Initial optimization checkpoint

- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets
  --locked -- -D warnings` and `cargo test --workspace --locked` passed.
  The workspace ran 521 Rust tests, including the CPU/DMA raster boundary,
  OBJ priority rotation, SPC timers, hires, Mode 7 and VRAM regressions.
- The nine README fixture examples built. The original smoke image
  regenerated byte-identically. Both cleared games rebuilt twice with
  their pinned hashes. The source-built 65C816 full/basic, SPC700 and
  eleven-band HDMA gates passed.
- The production build passed 207 Chromium checks with
  `SNES_TEST_PORT=18086 npm --prefix web test -- --grep-invert '301 seconds'`.
  Coverage included the five-minute cleared-game replays, Castle's full
  route, Memory's music and effects, both Pages demos under the local
  subpath, wide modes, sound queues, SRAM, personal cartridges, touch,
  fullscreen and Pixel/CRT. The CRT readback failure in #157 and the
  external-tool transport failure in #159 did not recur on these checks.
- The optional original Gothicvania and Castle replay paths were absent.
  The local Pages smoke tests covered the production artifact. Published
  Pages verification belongs after a merge.
- On this mixed macOS toolchain, native Rust 1.99.1 release builds emitted
  a debug-symbol stripping warning from `rust-objcopy` and still succeeded.
  The warning affected both native Rust endpoints. Rust 1.94.0 ran the
  workspace tests and Clippy. Rust 1.99.1 built the production Wasm and
  the pinned SPC assembler.

### Issue progress

- [x] Same-host baseline with native and production Wasm measurements
  against pinned Snes9x, six workloads, fixed replays and an 80% target.
- [x] Multiple implementation and measurement rounds, with deterministic
  guest evidence, full applicable regression checks and Pixel/CRT probes.
- [ ] Reach 80% of Snes9x on representative native and Wasm workloads.
- [ ] Establish sustained Pixel and CRT headroom on physical low-power
  hardware and complete the next architectural experiments above.
- [ ] Verify the deployed Pages build after a production change lands.
  Local Pages smoke coverage is part of the browser suite.

[Issue #174](https://github.com/takahirox/snes-rust/issues/174) remains the
umbrella for this work.
