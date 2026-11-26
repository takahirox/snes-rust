# Save profile and snapshot validation

Tested on Apple M4 Max, macOS 27.2.1, Rust 1.99.0, Node 26.12.1 and
Playwright Chromium 149.0.7827.55.

## Core and production build

- `cargo fmt --all -- --check` and
  `cargo clippy --workspace --all-targets --locked -- -D warnings` passed.
  `cargo test --workspace --locked` passed 532 tests. The snapshot tests compare
  all encoded hardware fields, video, PCM and SRAM after replay. They include
  both bundled games after their opening inputs, ordinary LoROM and HiROM,
  Mode 5 and Mode 7, the diagnostic's DMA and NMI, and live HDMA and SPC timers.
  Format tests exercised unsupported versions, wrong ROM identity, invalid
  machine fields with a valid checksum, truncation and compressed length bounds.
- The nine original CPU fixture examples built. The smoke ROM regenerated
  byte-identically. Both source-built Castle and Memory ROMs rebuilt twice
  with their pinned hashes. The licensed full and basic 65C816 suites and
  SPC700 suite reached markers `0649`, `0452` and `0557`. The source-built
  HDMA fixture displayed all eleven expected 16-line grayscale bands.
- `npm --prefix web ci` and `npm --prefix web run build` succeeded. The
  production Wasm was 252,220 bytes. The local build used the installed
  Rust 1.99.0 linker through a wrapper that located its LLVM dylib. The build
  and the Pages artifact add no ROMs or firmware.

## Chromium

All 225 tests passed in the production Chromium suite
(`npm --prefix web test -- --grep-invert '301 seconds'`). A focused rerun of
the 59 reset, launch, cartridge library and save tests also passed after the
hidden-tab SRAM fix. The suite covers the Pages subpath, both bundled games, local and
retained ROMs, and desktop, narrow portrait and short landscape layouts.
New tests exercised separated SRAM profiles, recovery after a failed backup
or failed replacement write, IndexedDB abort and denial, canceling a delayed
restore, and keyboard/touch, playback and Web Audio behavior during restore.
The browser round-tripped raw SRAM and state exports and rejected invalid
size, incompatible version and wrong-ROM imports.

A non-battery test saved 105 distinct snapshots, reloaded through the library,
paged the entire history, checked a thumbnail against the restored frame,
loaded an arbitrary earlier record and removed an entry. Reimporting the ROM
after removing its library entry restored access to its history. On this
host the median capture and durable write was 9.0 ms and the p95 was
10.3 ms. The sample restore took 54 ms including Playwright interaction.
The sample compressed diagnostic payload was 3,663 bytes before its JPEG
thumbnail and IndexedDB overhead.

Both bundled demos replayed the same PCM and full snapshot bytes in production
Wasm after restoring. The sampled Shionari and Yonaki payloads were 114,434
and 220,149 bytes. Encode times were 11.5 and 8.5 ms, and validation and decode
times were 3.7 and 8.2 ms, respectively.

## Gameplay performance

Five alternating optimized native runs per revision used 600 measured
fields after the existing workload warm-ups. The baseline was `3ee93a1`.
Video, audio, WRAM, SRAM, cycle and instruction results matched in every
comparison. These are local median times in milliseconds.

| Workload | Baseline | With snapshots available |
| --- | ---: | ---: |
| Shionari | 144.25 | 143.97 |
| Yonaki | 203.02 | 202.46 |
| Source-built HDMA | 82.34 | 82.65 |

The optional 301-second playthroughs using separately supplied game images
were skipped. The required source-built Castle and Memory playthroughs ran.
Published Pages verification remains a post-merge check.
