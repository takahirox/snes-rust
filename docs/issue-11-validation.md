# Issue #11 direct-color validation

Validated on 2026-10-23 for [Issue #11](https://github.com/takahirox/snes-rust/issues/11),
against evaluation baseline `7b265281def439c97a18b006ad12e313198c1bcd`.
The change selects BGR555 direct color for nonzero BG1 pixels in Modes 3/4 and
Mode 7 with EXTBG disabled. It keeps the existing scanline composition pipeline.
The channel wiring follows the Direct Color Mode table in
[Fullsnes](https://problemkaputt.de/fullsnes.htm), consulted during implementation.

## Original coverage and baseline evidence

- The private conversion helper has an exhaustive 256-pixel × 8-palette test.
  Its independent oracle sums per-input-bit output weights, with fixed vectors
  for all individual pixel and palette bits, `$0004`, `$0006`, `$639c`, `$73de`.
  Conversion of zero is tested numerically; transparency is tested separately.
- Actual planar Mode 3/4 and identity-transform Mode 7 rendering checks each
  input channel bit, all tile palette fields, direct enable/disable, and CGRAM
  mutation. Mode 7 always uses a zero palette contribution.
- Integration checks raw-zero transparency for every palette field, main and
  subscreen color math, fixed color, halving, layer and math windows, reduced
  brightness, BG priority, and unchanged BG2/OBJ/backdrop/paletted-mode colors.
  EXTBG retains the prior palette path; this does not implement EXTBG.
- Three guest tests execute original Rust-generated 32 KiB LoROMs in `Machine`.
  All register, VRAM and CGRAM writes execute as 65C816 instructions. Twelve
  variants cover Modes 3/4/7, direct color off/on, and blue/red CGRAM entries.
  Each has a two-frame and 100,000-step bound, numeric framebuffer assertions,
  and a transparent tile. Tests require no external assembler or ROM download.
  [Provenance](../tests/roms/PROVENANCE.md#original-direct-color-guest-tests-approved)
  records original authorship and MIT licensing for code and data.

To establish that the new tests detect the original defect, the PPU source was
temporarily replaced with the exact file from the evaluation baseline, and
`cargo test --locked -p snes-core --test direct_color` was run. It exited 101:
six failures, one pass. Each separate Mode 3/4/7 guest test produced blue
`[0,0,255,255]` instead of direct-color `[230,230,197,255]`. Rendering selection,
composition and priority assertions also failed; raw-zero transparency passed.
The implementation was restored before final validation. No existing hash,
duration, assertion or fixture was weakened.

## Validation commands

Host: macOS arm64, Cargo/Rust 1.94.0 for workspace/native checks, Rust 1.99.0 for
the auxiliary SPC assembler and Wasm, Node 24.12.0, npm 11.6.2, Python 3.13.13,
Playwright 1.61.1 with its installed Chromium. The native compiler meets the
README's Rust 1.94+ requirement; the SPC assembler uses the documented 1.99 toolchain.

| Command | Result |
| --- | --- |
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --workspace --all-targets --locked -- -D warnings` | Passed |
| `cargo test --workspace --locked` | Passed: 19 unit, 7 direct-color integration, 39 existing regressions; no ignored tests |
| `python3 tools/make-fixture.py .cache/smoke.bin` | Passed |
| `cmp tests/roms/smoke.bin .cache/smoke.bin` | Passed, byte-identical diagnostic |
| `RUSTC="$(rustup which --toolchain 1.99.0 rustc)" python3 tools/build-test-tools.py --spc` | Passed: all three pinned assemblers built; nonfatal `rust-objcopy` LLVM warning noted below |
| `python3 tools/hardware-tests.py --ca65 "$PWD/.cache/tools/cc65/bin/ca65" --ld65 "$PWD/.cache/tools/cc65/bin/ld65" --spcasm "$PWD/.cache/tools/spcasm/target/release/spcasm"` | Passed: CPU full/basic and SPC suites, 600 frames each; last-test markers 0649/0452/0557 and expected terminal PCs |
| `python3 tools/ppu-test.py --bass "$PWD/.cache/tools/bass-untech/bass/out/bass-untech"` | Passed: 11 grayscale HDMA bands, 16 lines each, final shade through line 223 |
| `python3 tools/build-cleared-games.py` (twice) | Both passed with identical pinned Castle/Memory ROM hashes |
| `npm --prefix web ci` | Passed |
| `npm --prefix web run build` with Rust 1.99 and the local linker wrapper below | Passed after fixing local dynamic-library lookup |
| `cd web && npx playwright install chromium` | Passed |
| `cd web && SNES_TEST_PORT=18085 npm test -- --grep-invert '301 seconds'` | Passed: all 18 selected tests, no skips (2.5 minutes) |

The browser gate passed both 18,100-frame cleared-game replays (20 windows each),
Castle's 11,097-frame five-map/credits route, Memory's 18 wins and restarts,
independent music/effect and real Web Audio checks, the Castle freeze negative
control, diagnostic controller/audio/save tests, and the Pages subpath smoke.
The optional unaudited game tests named `301 seconds` were excluded as specified.
No mandatory gate remains failed, skipped or unperformed.

The Rust 1.99 `rust-lld` could not find its existing `<sysroot>/lib/libLLVM.dylib`,
so the initial Wasm build failed. A `DYLD_FALLBACK_LIBRARY_PATH` invocation also
failed. An executable Python wrapper under ignored `.cache/issue-11` sets
`DYLD_LIBRARY_PATH` to that compiler's `<sysroot>/lib`, then execs its
`lib/rustlib/aarch64-apple-darwin/bin/rust-lld` with `-flavor wasm` and the
original linker arguments. The successful invocation was:

```sh
RUSTC="$(rustup which --toolchain 1.99.0 rustc)" \
CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_LINKER="$PWD/.cache/issue-11/rust-lld-wrapper" \
npm --prefix web run build
```

This changes only the build environment; repository build scripts and compiler
installations are unchanged. The auxiliary assembler build emitted the analogous
nonfatal `rust-objcopy` warning; it completed, and its generated SPC fixture passed.
The first browser invocation failed before running tests because port 8080 was
occupied. The final run used the README-documented alternate port 18085.

## Fixture integrity

Only pinned licensed source-built hardware fixtures and the two cleared modified
games were acquired. Hardware fonts remain the documented zero replacements.
[Hardware provenance](../tests/roms/PROVENANCE.md),
[assembler pins](../tests/roms/toolchains.json), and the
[cleared-game audit](../tests/roms/CLEARED-GAMES.md) retain their existing scope.
Observed SHA-256 values:

| Fixture | SHA-256 |
| --- | --- |
| Original smoke diagnostic | `73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb` |
| CPU full | `1727e7c46ebb5df55711598264ba19005986bd34e915621383966adf674b3cfa` |
| CPU basic | `514d58950ae1ca940d6673c2a028b258b2ab17293bbc7cbd7cb54dded32abc5b` |
| SPC | `279d6a92d051ff8a98e190ce73e40482ff2cc981812509c7bca35a047484507e` |
| HDMA | `aef339e5d4a7319db31de577b253d977b45f5c45f09adfaa99a302ca300a957a` |
| Cleared Castle | `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869` |
| Cleared Memory | `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1` |

The twelve original guest hashes are recorded in
[`issue-11-guests.json`](../tests/results/issue-11-guests.json). These were measured
by exporting the exact test builder's bytes into ignored cache files; the tests
themselves generate them in memory. No generated guest ROM is committed.

## Limits

`ppu-color` remains partial. Comprehensive window logic, clipping/prevention
interactions, subtraction/saturation, halving and brightness coverage remain
incomplete. Mode 7 EXTBG and transform edges remain separate gaps. This bounded
change does not establish full PPU accuracy or commercial compatibility, and
makes no completion claim for other inventory categories or Issue #3.
Optional tile-flip/scroll/mosaic direct-color combinations and unaudited optional
game replays were not run. No post-merge verification is required by this Issue.
