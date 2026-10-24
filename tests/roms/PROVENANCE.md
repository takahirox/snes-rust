# ROM and build provenance

No commercial ROM, Nintendo firmware, external game ROM, or third-party emulator
implementation is distributed. `.sfc`, `.smc`, saves, downloads, compiler sources,
build products and screenshots are ignored. A file being available to download
is not a redistribution decision. The only committed binary is our original
`smoke.bin` diagnostic. The original-release investigations below remain
incomplete. **Two different source-built modified game images are now cleared:**
Castle Platformer with replacement credits and Memory Game with replacement
lettering and original game-event sound effects. Their complete allowlist,
build decisions, notices and output hashes
are in [CLEARED-GAMES.md](CLEARED-GAMES.md) and
[cleared-games.json](cleared-games.json). That approval does not apply to the
original release binaries below. Castle remains silent. Modified Memory Game
executes an original MIT SPC program and BRR waveform through the generic core;
this does not demonstrate an upstream music driver. Current first-playable
acceptance is recorded in [the #5 validation](../../docs/issue-5-validation.md);
#3 was closed only as a tracker handoff. The historical investigations below
are not release acceptance evidence.

## Original diagnostic: approved

- Source: `tools/make-fixture.py` in this repository; no upstream input.
- Artifact: `tests/roms/smoke.bin`, 32,768 bytes, ordinary NTSC LoROM with SRAM.
- SHA-256: `6e5518b3959883cc22e914621fc5b5d9f5c0fe4eb96ee61000769afdbb68d6e5`.
  Issue #69 adds a busy-assertion wait before completion in the NMI handler.
  The original `73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb`
  was reproduced byte for byte before correction; original input/video/DMA/audio
  assertions remain. See [the correction record](../../docs/issue-69-validation.md).
- Copyright: 2026 snes-rust contributors; [MIT](../../LICENSE).
- CPU/SPC machine code, checker pixels, BRR samples and headers are original.
  No fonts, graphics libraries, firmware, submodules or compiler runtime.
- Build: Python standard library only. Rebuild and compare bytes in CI.
- Decision: source and generated ROM may be redistributed under MIT. It is a
  diagnostic, not an independent homebrew game and not gameplay acceptance.

## Original DMA A-bus 65C816 guests: approved

- Source: [`crates/snes-core/tests/dma_access.rs`](../../crates/snes-core/tests/dma_access.rs),
  including its `guest`, `status_guest`, WRAM-conflict, WRAM-address and
  reverse-WMDATA guest builders;
  no upstream code or data input.
- Copyright: 2026 snes-rust contributors; [MIT](../../LICENSE).
- Six 32,768-byte LoROMs are generated in memory from original SEI, LDA/STA,
  AND, conditional-branch and STP instruction streams, literal transfer data
  and an original header.
  All register initialization and transfer triggers execute through `Machine`;
  host code supplies only the cartridge and controller button input.
- Forward coverage reproduces Issue #21's illegal A-bus WRAM-port reads into
  CGRAM and checks serial joypad suppression. Reverse coverage checks CPU/DMA
  register protection, legal B-bus CGRAM read progression and WRAM-port protection.
  Numeric CPU-written WRAM results and memory/register state are asserted.
- Issue #23's status guest configures VBlank/timer IRQ and automatic controller
  reads, waits using HVBJOY, initializes arithmetic and RDIO, and triggers DMA
  from RDNMI/TIMEUP and all remaining status/result offsets. CPU instructions
  mask floating bits and copy numeric results into WRAM. The host injects no
  private flags or expected result bytes. Division and multiplication results,
  first-controller `$A590`, acknowledgement and final DMA registers are checked.
- Issue #25's WRAM-conflict guest initializes `$7E:0100`, destination sentinels
  and a 17-bit WMDATA pointer through CPU instructions, triggers conflicting DMA,
  and records numeric A-address/bank/count and port-readback results in WRAM.
  A legal ROM-to-WMDATA control uses the original literal `$6B` at `$00:F000`.
  The host supplies only the generated cartridge and independently asserts results.
- Issue #27's WRAM-address guest uses original absolute/long LDA/STA instructions
  to initialize payloads, distinct correct/redirected-address sentinels and all
  17 pointer bits. It triggers WRAM-sourced DMA to each of `$2181–$2183`, including
  both bit-16 directions, and observes the pointer through two `$2180` reads.
  CPU instructions record numeric source/address/bank/count, sentinel and legal
  CPU/ROM-control results in WRAM, then write a success marker and stop. Original
  literal ROM payloads at `$00:F000–F003` are `$5A,$5A,$FE,$FF`. No host state or
  expected results are injected. Source, data and in-memory build remain MIT.
- Issue #29's reverse-WMDATA guest uses original CPU instructions to initialize
  destinations, the 17-bit pointer and `$A6/$37` sentinels. Reverse DMA targets
  `$7E:0100`, `$7F:0100` and `$00:0100`; subsequent CPU port reads observe pointer
  retention, including a `$1FFFF` wrap control. Numeric address/bank/count and
  readback results are recorded in WRAM. Separate ROM-write-protection and
  CGRAM-to-WRAM controls check legal reads/writes. The conflict byte is never
  asserted. The original header, code and literal ROM byte `$6B` at `$00:F000`
  are built in memory with no external source, asset, binary or assembler input.
  Source and generated code/data remain MIT; no guest binary is committed.
- The original two guests, WRAM-conflict, WRAM-address and reverse-WMDATA guests
  are bounded to 10,000 instructions; the status guest to 100,000 instructions. All must finish in
  fewer than two frames.
  No font, firmware, commercial ROM, external binary, assembler or runtime is used.
  The associated original unit vectors in `src/machine/dma_tests.rs` are also MIT.
- Decision: original source and generated ROMs may be redistributed under MIT.
  No additional binary is committed. These are diagnostics, not game or complete
  hardware acceptance. See [original validation](../../docs/issue-21-validation.md),
  [reverse-WMDATA validation](../../docs/issue-29-validation.md),
  [corrected-read validation](../../docs/issue-23-validation.md),
  [WRAM-conflict validation](../../docs/issue-25-validation.md) and
  [WRAM-address validation](../../docs/issue-27-validation.md).

## Original direct-color guest tests: approved

- Source: [`crates/snes-core/tests/direct_color.rs`](../../crates/snes-core/tests/direct_color.rs),
  including the `guest_rom` Rust builder; no upstream program or data input.
- Copyright: 2026 snes-rust contributors; [MIT](../../LICENSE).
- The 65C816 instruction stream, constant-color planar and Mode 7 graphics,
  tilemaps, palettes and LoROM header are original code/data authored for Issue #11.
  Hardware bit-layout expectations come from the Fullsnes documentation linked
  below; no emulator implementation, font, firmware or commercial ROM was used.
- `cargo test --workspace --locked` constructs twelve 32,768-byte guest variants
  in memory (Modes 3/4/7, direct color off/on, two CGRAM colors). Each executes
  through `Machine` for two frames with a 100,000-instruction limit and numeric
  framebuffer assertions. Nothing is downloaded and no generated ROM is committed.
- Decision: the original source and generated ROMs may be redistributed under MIT.
  These are bounded diagnostics, not game or comprehensive hardware acceptance.
  See [validation](../../docs/issue-11-validation.md) for baseline failure evidence
  and the unchanged external fixture provenance/hashes.

## Original offset-per-tile 65C816 guests and graphics: approved

- Source: [`crates/snes-core/tests/ppu_offset.rs`](../../crates/snes-core/tests/ppu_offset.rs),
  including `guest_rom`, the coordinate-labelled planar texture and literal vectors.
  No upstream program, graphics or implementation input.
- Copyright: 2026 snes-rust contributors; [MIT](../../LICENSE).
- Eight 32,768-byte LoROM variants are generated in memory: Modes 2/4,
  horizontal/vertical replacement, enabled/disabled. Original SEI, LDA/STA and
  BRA instruction streams initialize registers, VRAM and CGRAM through `Machine`
  CPU execution during forced blank. Tests never patch the guest's PPU state.
- Graphics are original solid red/blue/white characters, original maps and offset
  words, a green backdrop and an original cartridge header. The other rendering
  tests use mathematically generated nonuniform pixels/palettes. No font, firmware,
  commercial ROM, external binary, assembler or compiler runtime is used.
- Each guest is bounded to three frames and 150,000 instructions and checked
  against literal RGBA values at the first eligible boundary. Hardware facts and
  independent source-coordinate derivations are attributed in the
  [validation record](../../docs/issue-19-validation.md).
- Decision: original source/data and generated ROMs may be redistributed under
  MIT. No generated ROM is committed or downloaded. These are bounded diagnostics,
  not games, full hardware validation or commercial compatibility evidence.

## Original envelope-rate SPC700 guest: approved

- Source: [`crates/snes-core/tests/dsp_envelope.rs`](../../crates/snes-core/tests/dsp_envelope.rs),
  including its `guest` Rust builder; no upstream program or data input.
- Copyright: 2026 snes-rust contributors; [MIT](../../LICENSE).
- The original 146-byte SPC700 instruction stream is generated in memory during
  `cargo test --workspace --locked`. It configures two GAIN voices through
  `$F2/$F3`, waits for host commands through `$F4`, changes a rate without KON,
  records four ENVX pairs at `$1000..$1007`, acknowledges each checkpoint and
  stops. The host uses the existing firmware-free IPL upload port protocol.
- All instructions, command/checkpoint data and initially zeroed 64 KiB RAM are
  original. Pitch remains zero: no BRR sample, font, firmware, external test
  binary, game asset, compiler runtime or emulator implementation is imported.
  Source-level rate facts are attributed in the
  [validation record](../../docs/issue-13-validation.md#specification-and-counter-convention).
- Execution has explicit per-acknowledgement and per-checkpoint bounds. Checkpoint
  expectations are literal numerical vectors, independent of production helpers.
  No generated binary is committed or downloaded; no external assembler is needed.
- Issue #75 adds the original 133-byte `hidden_guest` builder in the same file.
  It configures a held NON voice and direct GAIN, selects rate-zero linear GAIN,
  then bent rate 31 through `$F2/$F3`, recording ENVX at `$1010..$1012`.
  Its twelve NOPs and bounded host acknowledgements select the first bent sample;
  exact guest checkpoints and PCM are derived in the
  [hidden-envelope validation](../../docs/issue-75-validation.md).
  The noise seed, register values, instructions and expected data are original;
  pitch remains zero and no firmware, BRR or external executable is acquired.
- Decision: original source and generated guest/data may be redistributed under
  MIT. These are bounded envelope diagnostics, not full DSP or game acceptance.

## Original noise SPC700 guest and BRR data: approved

- Source: [`crates/snes-core/tests/dsp_noise.rs`](../../crates/snes-core/tests/dsp_noise.rs)
  and [`crates/snes-core/src/dsp_noise_tests.rs`](../../crates/snes-core/src/dsp_noise_tests.rs);
  no upstream program, sample or implementation input.
- Copyright: 2026 snes-rust contributors; [MIT](../../LICENSE).
- The original 155-byte SPC700 guest is constructed in memory and uploaded using
  the firmware-free IPL protocol. It configures NON/FLG and a direct-GAIN voice
  through `$F2/$F3`, changes the noise rate without KON, records five numerical
  OUTX checkpoints at `$1000..$1004`, acknowledges host commands and stops.
- The zero filter-0 BRR blocks (looping and non-looping), constant range-9
  filter-0 loop, directory entries, initial RAM, command data, bit-stream oracle
  and literal numerical vectors are
  original MIT code/data. No external binary, firmware, font, game asset or
  emulator implementation is fetched, copied, linked or required for these tests.
  No guest binary is committed; no assembler is needed. Execution and
  acknowledgements are bounded. See the [noise validation record](../../docs/issue-15-validation.md)
  for specification attribution, independent derivations and baseline failures.
- Decision: original source and generated guest/data may be redistributed under
  MIT. This diagnostic establishes bounded sample-level behavior, not complete
  DSP accuracy or game acceptance. All external fixture decisions below remain.

## Original pitch-modulation SPC700 guest and BRR data: approved

- Source: [`crates/snes-core/tests/dsp_pitch.rs`](../../crates/snes-core/tests/dsp_pitch.rs)
  and [`crates/snes-core/src/dsp_pitch_tests.rs`](../../crates/snes-core/src/dsp_pitch_tests.rs).
  Copyright 2026 snes-rust contributors; [MIT](../../LICENSE).
- The original 163-byte SPC700 guest, directory entries, range-9 filter-0 signed
  ramp, range-12 constant block, zero loop and terminating variants, commands,
  numerical vectors and initial RAM are authored for Issue #17. Hardware facts
  and independent arithmetic are attributed in the
  [pitch validation record](../../docs/issue-17-validation.md).
- The guest is constructed in memory, uploaded through the existing firmware-free
  protocol, writes PMON through `$F2/$F3`, records target OUTX/ENDX at
  `$1000..$1005`, disables modulation without KON, acknowledges three bounded
  checkpoints and stops. Both PMON-enabled and disabled controls execute.
- No external implementation, binary, firmware, font, sample, game asset,
  assembler or compiler runtime is acquired for these diagnostics. No generated
  guest binary is committed. Source and generated code/data may be redistributed
  under MIT. All external fixture decisions and pins below remain unchanged.

## Original live-directory SPC700 guest and BRR data: approved

- Source: [`crates/snes-core/tests/dsp_directory.rs`](../../crates/snes-core/tests/dsp_directory.rs).
  Copyright 2026 snes-rust contributors; [MIT](../../LICENSE).
- The original 212-byte SPC700 guest is generated in memory and uploaded through
  the existing firmware-free port protocol. It configures one BRR voice through
  `$F2/$F3`, waits for four host commands, records OUTX/ENVX/ENDX at
  `$1000..$100b`, changes loop-word RAM, DIR and SRCN without another KON,
  acknowledges each bounded checkpoint through `$F4`, and stops.
- Directory words, constant signed range-10 filter-0 BRR blocks, normal and
  terminating controls, wrap cases, initial RAM, command data, instruction
  stream and numerical expectations are original MIT code/data. The hardware
  facts and independent arithmetic are attributed in the
  [validation record](../../docs/issue-77-validation.md).
- No commercial ROM, Nintendo firmware, external sample, font, game asset,
  executable or emulator implementation was acquired or imported for these
  diagnostics. No guest binary is committed; no external assembler is needed.
  The pinned blargg source was consulted as attributed research evidence, not
  copied, linked, executed or used as an output oracle.
- Decision: original source and generated guest/data may be redistributed under
  MIT. This is bounded sample-level evidence, not complete hardware accuracy or
  game acceptance. All external fixture decisions and pins below remain intact.

## Source-built hardware suites: approved with documented alterations

### gilyon/snes-tests

Source repository: <https://github.com/gilyon/snes-tests>, pinned commit
`5ecdf555da920f0bd7b157542141965a8120186d`. Copyright (c) 2023 gilyon,
[MIT notice](licenses/gilyon-MIT.txt). Exact fetched paths and their SHA-256
values are in [gilyon-source.json](gilyon-source.json); the URLs are
`https://raw.githubusercontent.com/gilyon/snes-tests/COMMIT/PATH`.

`tools/hardware-tests.py` fetches only the listed license, assembly, linker
configurations and Python test generators. It fetches **no font, prebuilt ROM,
firmware, game asset, submodule or emulator**. These files describe/generate CPU
instruction tests rather than implement an emulator. Their applicable license
is MIT. The explicit alteration is a new, all-zero 2,048-byte `font.bin` in each
suite, replacing the upstream bitmap whose separate provenance was not
established. The blank bitmap's SHA-256 is
`e5a00aa9991ac8a5ee3109844d84a55583bd20572ad3ffcd42792f3c36b183ad`.
Test assertions and instruction streams are unchanged. Screens do not show test
text; assertions use the test counters and terminal PCs.

Build artifacts are `.cache/hardware-tests/cputest/cputest-full.sfc`,
`cputest-basic.sfc` and `.cache/hardware-tests/spctest/spctest.sfc`. Build-time
SHA-256 values and terminal markers are recorded in `results.json`, with a
checkpoint copy in [validation.json](../results/validation.json). Assembler
versions and source pins are specified below. These are assembly-only programs;
no tool runtime is linked. Source generators have no third-party Python packages.
Decision: fetch these pinned source files and build modified diagnostic ROMs;
retain the MIT notice and this alteration record. No ambiguous font is imported.

### undisbeliever/snes-test-roms: HDMA timing demonstration

Source: <https://github.com/undisbeliever/snes-test-roms>, commit
`ac6ef8006809c0ff5dabc8ff137a4623967697fe`. Selected program:
`src/hardware-tests/hdma-timing-demonstration.asm`, Copyright © 2024 Marcus Rowe
<undisbeliever@gmail.com>, zlib. Shared headers carry © 2016–2019 Marcus Rowe;
[license text](licenses/snes-test-roms-zlib.txt) and original per-file notices
are retained in downloaded source. Exact source paths, including the character
index mapping (code, not glyph pixels), and SHA-256 values are in
[hdma-source.json](hdma-source.json). URLs follow the same pinned raw-file pattern.

`tools/ppu-test.py` imports no image. It creates its own zero 2,048-byte glyph
bitmap for the break handler and explicitly marks the shared `memory.inc` as
altered: two empty marker defines receive value `1`, making the pinned
assembler's `defined` checks work. This affects assembly metadata, not the
emitted test instructions or HDMA tables. No game-specific emulator change is
used. `bass-untech` is the upstream's pinned assembler submodule; no other
submodule or runtime is needed. The ROM is generated at `.cache/ppu-test/hdma.sfc`;
its hash and assertions are recorded in `results.json`.

Decision: the selected code is explicitly zlib licensed; fetch and build this
modified fixture, retaining notices and the alteration description. Verify its
11 grayscale bands at 16-scanline boundaries, including the final held shade.
This is a limited scanline/HDMA check; it is not a claim that all timing fixtures,
DMA contention, event ordering or dot-level hardware timing pass.

## Build tools

[toolchains.json](toolchains.json) records exact source archive URLs, commits,
SHA-256, licenses, authors and decisions. `tools/build-test-tools.py` verifies
those archives and builds under `.cache/tools` without global installation.

| Tool | Pin | License and use |
| --- | --- | --- |
| ca65/ld65 | cc65 `555282497c3ecf8b313d87d5973093af19c35bd5` (V2.19 tag; banner V2.18) | Zlib, cc65 contributors; assembler/linker only |
| bass-untech | `9db6088a378061afc7b82f50997a5b9a1d951175` | ISC per upstream README/source headers; Near, fork by The UnDisbeliever; nall utilities included in tool build |
| spcasm | Codeberg `c2d57b7aa8ddd6b22a18424047b8c2d93f0373e7` (2.0.0) | BSD-2-Clause, © 2022 kleines Filmröllchen; only `-p spcasm --bin spcasm` is built |

The spcasm source repository also contains a separate emulator and web/fonts.
Those packages are neither inspected for implementation, built, linked nor used.
Its assembler dependency versions come from its pinned `Cargo.lock`. The tool
requires Rust 1.99+ and experimental features (`RUSTC_BOOTSTRAP=1` only for that
build); our Rust core does not need experimental compiler features. All three
tools consume source and emit test machine code; none supplies emulator code or
links its implementation into a ROM or the emulator. Tool archives retain their
own license files/notices. They are not redistributed here. The Rust application
has no third-party Rust crates. Playwright 1.61.1 is a browser test tool, pinned
with npm integrity hashes in `web/package-lock.json`; it is not shipped in the
production frontend.

## Candidate games: investigation only; no redistribution approval

The exact local images below were exercised for development, then kept in the
ignored `.cache/validation` directory. Nothing fetches them in CI or the app.
The optional browser tests require an operator-supplied path and verify the
image SHA-256 before execution. Test hashes and input masks contain no ROM data.
This record **does not** turn a repository-level MIT license into clearance for
all binary contents. Finish the gaps before bundling or automatically fetching
any game image or declaring Issue #3 complete.

### Gothicvania

- Repository: <https://github.com/donth77/snes-homebrew>.
- Release: `v1.0.0`; tag commit `f94da8b5b79b019a9250fdae4b1667737d30412d`.
- Investigated source checkout: `119496e6a2f1e53b7704712fef8cb81814f1698a`.
- Precise image: release asset `gothicvania.sfc` at
  <https://github.com/donth77/snes-homebrew/releases/download/v1.0.0/gothicvania.sfc>.
- SHA-256: `11d32045c69839309490676fe2dd46e3385806f79c8dad279c2d1f4ffa6027df`.
- Code: MIT, © 2026 Tom D; [notice](licenses/gothicvania-MIT.txt).
- Graphics: adapted GothicVania Cemetery by ansimuz. The
  [author's asset page](https://ansimuz.itch.io/gothicvania-cemetery) identifies
  CC0 v1.0 Universal and describes bfxr-generated royalty-free sound effects.
  The source README credits CC0 art; `adapt_sfx.py` points to the same pack's
  Phaser demo sound directory. Original pack inputs were removed upstream.
- Music: source README describes original title/in-game music. Included
  `assets/Title.mid`, `assets/Baroque.mid`, generated tracker modules and
  sample conversions still need a per-artifact audit against the release.
- Build/runtime: upstream documents PVSnesLib 4.5.0, 816-tcc, WLA-DX, gfx4snes,
  smconv/snesmod and snesbrr. Their **linked runtime/driver/data** license and
  notice inventory and a reproducible release-to-source match remain unverified.
  Tools' licenses must be distinguished from licenses of code embedded in ROM.
- Decision: user-supplied development image only. Do not bundle or auto-fetch.
  Primary art licensing is verified; full sound/build/binary clearance is pending.

### Castle Platformer

- Repository: <https://github.com/undisbeliever/castle_platformer>.
- Release: `v1.04`; tag commit `847e75261c394e33deff21232cd1c051dca04e30`.
- Investigated source checkout: `79e2eb4e74702693b4248e15e00c7e6beebd271f`.
- Release archive:
  <https://github.com/undisbeliever/castle_platformer/releases/download/v1.04/Castle.Platformer.v1.04.zip>,
  SHA-256 `e75da7af6f3a1bd10c21cf4324d0c4321271ce3b9ef3408b2dc2db9cb05f9159`.
- Precise artifact: `Castle Platformer (v1.04)/Castle Platformer (v1.04).sfc`;
  SHA-256 `8b325cbc3b5eee257b906f11b0a0bced2925c503293e9f242ca0492f23804df8`.
- Game/release code: MIT, © 2015 The UnDisbeliever
  <undisbeliever@gmail.com>; [notice](licenses/castle-MIT.txt). The archive
  includes LICENSE and README explicitly licensing the game and crediting art.
- Graphics: [Jetrel's Castle Platformer](https://opengameart.org/content/castle-platformer),
  [qubodup's ice tiles](https://opengameart.org/content/simple-svg-ice-platformer-tiles-16x16-16x96-96x16),
  and [Classic hero and baddies](https://opengameart.org/content/classic-hero-and-baddies-pack)
  by Jason-Em (the current page credits GrafxKid), all CC0 as verified on the
  authors' distribution pages. Per-asset `.source` files in the checkout contain
  attribution and describe sprite alterations. Credits-image/font provenance and
  the exact release asset inventory still need verification.
- Submodule in investigated checkout: `snesdev-common`, repository
  <https://github.com/undisbeliever/1gam-common>, commit
  `aeefd05495cbe78910c1f402cdb0a132d4c169ac`, MIT, same author. This is a **game
  library**, not an emulator, and is not imported into our emulator.
- **Release-tag correction:** `v1.04` actually pins `snesdev-common` to
  `02f661ca902d6663cf43d9aacb4b43acf83a34ff`, also MIT with the same 2015
  notice. The `aeefd054...` pin above belongs only to the later investigated
  checkout. It must not be used to reproduce or audit the release.
- Build: ca65/ld65, pcx2snes, ucon64, Tiled, python3-tmx and ImageMagick are listed
  upstream. Release tool versions, generated graphics, fonts and exact submodule
  pin must be matched to `v1.04`; this release has not been reproduced locally.
- Decision: user-supplied development image only. Primary code/art rights are
  documented; full binary/build/font clearance is pending. No audio is expected
  from this selected demo; silence here is not the audio acceptance case.

### Review revision: additional audit evidence, still not cleared

The licensing finding remains open. The following checks narrow it; they do
**not** approve either complete image or satisfy the two-game acceptance gate.
No game binary, font, library code or game art was added to this repository.

- Gothicvania's entire `gothicvania/` source tree is byte-identical between the
  release tag and MIT-licensed commit `119496e6...`: `git diff TAG COMMIT --
  gothicvania` is empty. Only LICENSE and the root README changed. This ties
  that MIT notice to the tagged game sources, without licensing dependencies.
- The tagged MIDI files contain the track titles "Gothic Title Theme" and
  "Gothic Baroque Loop". The README attributes the music as original;
  `tools/mid2it.py` synthesizes square/triangle/saw samples mathematically,
  rather than importing instrument recordings. The precise inputs are listed
  in [game-audit.json](game-audit.json). Reproducing the tracker modules and
  matching their packed soundbank to the release remains unperformed.
- `tools/adapt_title.py` names `Kenney Pixel.ttf`, in addition to ansimuz's
  title art. [Kenney's own font page](https://kenney.nl/assets/kenney-fonts)
  identifies CC0. The exact font file used upstream is absent; a matching
  title-overlay rebuild has not been established.
- PVSnesLib 4.5.0's precise Darwin tool archive and source archive hashes are
  recorded in `game-audit.json`. Its LoROM SlowROM link inputs are
  `crt0_snes.obj`, `libc.obj`, `libm.obj`, `libtcc.obj`; the game has no
  submodules. The library's notices are [MIT](licenses/pvsneslib-MIT.txt) and
  [zlib-style](licenses/pvsneslib-zlib.txt); these are evidence, not a blanket
  clearance of everything linked. In particular, `source/libtcc.asm` identifies
  multiplication/division routines taken from WDC's *Programming the 65816*;
  `source/libc_c.c` separately credits Linux `vsprintf.c` (Linus Torvalds /
  Lars Wirzenius) and a Flipcode allocator. Their specific applicable terms,
  the emitted sections, and a complete release link match remain unresolved.
- The 5,522-byte `source/sm_spc.asm` driver data matches Gothicvania's release
  ROM **exactly** at file offset `0x5cde0`, SHA-256
  `d0221a0965a8562bfc8e36e2f02c2e2c3a3b17d99dce07b563c34fd4bdf4a265`.
  The assembly driver credits Mukunda Johnson and KungFuFurby; its CPU-side
  WLA adaptation credits KungFuFurby and Alekmaul and carries zlib-style terms.
  [Original SNESMOD terms](licenses/snesmod-MIT.txt) are pinned to upstream
  `3e4990a5405bbeba43710bb5f09dc7c34a7cd9f4/LICENSING.txt`. A complete
  source/derivative/build inventory is still needed before clearance.
- Castle's tagged `src/resources.s` enumerates five maps, one combined
  tileset/palette, five entity sprite sheets and the credits image. The tagged
  per-asset `.source` records identify Jetrel, qubodup and Jason-Em's CC0
  inputs and the author modifications. No sound module is linked by this
  Makefile. The release submodule pin is now verified above.
- Castle's `resources/images4bpp/credits.xcfgz` retains GIMP text layers using
  the generic font names **Sans** and **Sans Bold**, with no specific font
  file/version. The accompanying `.source` explains the art composition and
  that the text was made in GIMP, but does not identify the resolved fonts.
  Both credits-source hashes are recorded in `game-audit.json`. Do not infer
  a particular font license from a generic family name. The credits font and
  the source-to-release asset/build match remain unresolved.

### Replay marker derivation

Only tests contain game-specific addresses. The core has no game recognition or
game-specific fixes. The ROM SHA checks must pass before these markers are used.

- Gothicvania: tagged `src/game.c` declares `feetX/feetY` (8.8 fixed point),
  `vy` and `onGround`. A diagnostic link of the tagged C sources with the
  specified 4.5.0 compiler locates them at `$7e2040/$7e2044`, `$7e204a` and
  `$7f0009`. Missing graphical link inputs were replaced with zero data **only
  to obtain RAM symbols**; that diagnostic image was not used for gameplay and
  is not a reproduced release. Absolute accesses to the position globals also
  appear in the real release. Tests read the integer position words one byte
  into each fixed-point field. `src/main.c`'s permanent `st` local occupies
  `$1ffb`: reset sets S=`$1fff`, JSL pushes three bytes, and the compiler's
  first two-byte local occupies `$1ffb-$1ffc`. The enum is title=0, play=1,
  end=2, game over=3. Tests assert title→play→game over→title transitions;
  `play.c` requires grounding and a fresh jump press before setting negative vy.
- Castle: tagged `src/entity.h`, `entity-animation.h`, `entity-physics.h`
  and `entities/player.h` define the player at `$0200`, x/y integer words
  at `$0205/$0208`, vy at `$0220`, and standing-tile pointer at `$0222`.
  ca65 structure constants give offsets `4,7,0x20,0x22` from the player base.
  The release's game-loop dispatch contains `AD 5F 01 F0 DF` at file offset
  `0x2002`, loading `GameLoop__state` from `$015f`; `gameloop.h` defines
  playing=0, dead=2. Tests assert death/restart and grounded jump behavior.
  Map completion and the credits screen are not verified.

### Previously downloaded test releases (not used by reproducible checks)

For investigation, the upstream gilyon v1.4 archive was downloaded from
<https://github.com/gilyon/snes-tests/releases/download/v1.4/snes-tests.zip>.
Archive SHA-256: `c3a39e15a9f3789fe143e54dfa035c988204576eafacf97df8ff3546a917c5e9`.
The prebuilt fonts were not separately audited. These images remain ignored;
**use the source-built, font-replaced fixtures above** for reproducible checks.

| Artifact in archive | SHA-256 |
| --- | --- |
| `cputest/cputest-full.sfc` | `f61de78d346a68c166d67472a414e5d207368889d37b977d1338f9221ca6400e` |
| `cputest/cputest-basic.sfc` | `d479bde706b9e16d76c0dfe98e2b2d3f5348f9dcb911e6a36b898f4201fc98bb` |
| `spctest/spctest.sfc` | `3909f03d55b07081a4e9ac437ba5179731ec6c8ac3c715f712daec0f398dde63` |

SureInstinct was not fetched or substituted for the two required games. No
commercial cartridge was fetched, included, or tested. Hardware documentation
references (not implementation code): [WDC 65C816 documentation](https://www.westerndesigncenter.com/wdc/documentation/w65c816s.pdf)
and [nocash's hardware specifications](https://problemkaputt.de/fullsnes.htm).

## Original HDMA initialization CPU guests: approved

- Source: [`crates/snes-core/tests/hdma_init.rs`](../../crates/snes-core/tests/hdma_init.rs)
  and scheduler vectors in
  [`crates/snes-core/src/machine/hdma_init_tests.rs`](../../crates/snes-core/src/machine/hdma_init_tests.rs).
  Copyright 2026 snes-rust contributors; [MIT](../../LICENSE).
- Both original 65C816 guests are built in memory from explicitly encoded
  instructions. Their zero-filled 32 KiB LoROM, title/header, reset vector,
  direct/indirect tables, payloads and result vectors are original MIT data.
  They configure during VBlank, synchronize through executed `$4212` reads,
  record prepared registers, edit metadata/payload before HBlank, read back
  CGRAM and source registers, and report results in WRAM before STP.
- Host code supplies only the cartridge and observes results; it injects no
  private state or guest memory. Each run is bounded by 100,000 instructions
  and fewer than three frame wraps. No external assembler, firmware, font,
  commercial ROM, game asset, emulator implementation or test binary is used.
  Generated ROMs are not committed. Source-backed ordering, derived zero-header
  behavior and retained timing approximations are recorded in the
  [Issue #31 validation](../../docs/issue-31-validation.md).
- Decision: original sources and generated code/data may be redistributed under
  MIT. Existing external fixture pins, notices and licensing decisions remain.

## Original HDMA row-end reload CPU guests: approved

- Sources: [`crates/snes-core/tests/hdma_reload.rs`](../../crates/snes-core/tests/hdma_reload.rs)
  and [`crates/snes-core/src/machine/hdma_reload_tests.rs`](../../crates/snes-core/src/machine/hdma_reload_tests.rs).
  Copyright 2026 snes-rust contributors; [MIT](../../LICENSE).
- Two original 65C816 instruction streams, tables, payloads, result vectors,
  title/header and reset vector construct zero-filled 32 KiB LoROMs in memory.
  Encoded SEI, LDA/STA, AND, conditional branches and STP use no assembler,
  runtime, font, firmware, commercial ROM or external instruction/data source.
- Guests configure in VBlank and poll executed `$4212` reads through successive
  HBlanks. They record the prepared second descriptor/pointer, overwrite those
  table bytes, change live payload data, read numeric CGRAM and final register
  results into WRAM, then check a sentinel after termination. Host code only
  supplies the cartridge and observes WRAM and the stop/deadline state; no CPU,
  bus, scheduler or memory state is injected. Each guest has a 100,000-instruction,
  fewer-than-three-frame bound. Generated ROMs are not committed.
- Decision: original source and generated instruction/data artifacts may be
  redistributed under MIT. Existing fixture pins, game inputs, hashes and
  licensing decisions remain intact. The
  [Issue #33 record](../../docs/issue-33-validation.md) separates specification
  facts, derived expectations and retained scheduler approximations.

## Original HDMA phase CPU guests: approved

- Sources: [`crates/snes-core/tests/hdma_phase.rs`](../../crates/snes-core/tests/hdma_phase.rs)
  and [`crates/snes-core/src/machine/hdma_phase_tests.rs`](../../crates/snes-core/src/machine/hdma_phase_tests.rs).
  Copyright 2026 snes-rust contributors; [MIT](../../LICENSE).
- Two original encoded 65C816 instruction streams, direct/indirect tables,
  payloads, result vectors, LoROM header/reset vector and zero-filled 32 KiB
  cartridge construction use no external code, assembler, runtime, font,
  firmware, commercial ROM, game assets or emulator implementation. Generated
  cartridges are not committed.
- Guests configure during VBlank, execute WAI with a programmed H/V IRQ, read
  table state between the old and corrected service phases, then publish later
  reload/pointer and HBlank observations in WRAM before STP. Host code supplies
  only the cartridge and observes bounded WRAM/terminal/deadline results. No
  guest CPU, memory, DMA or raster state is injected. Limits are 100,000 steps
  and fewer than three frame wraps. The
  [phase validation](../../docs/issue-35-validation.md) derives the numeric
  synchronization and distinguishes model latency from hardware facts.
- Decision: original sources and generated instructions/data may be redistributed
  under MIT. Existing fixture pins, notices and licensing decisions remain intact.

## Original HDMA live-NTRL CPU guests: approved

- Sources: [`crates/snes-core/tests/hdma_ntrl.rs`](../../crates/snes-core/tests/hdma_ntrl.rs)
  and [`crates/snes-core/src/machine/hdma_ntrl_tests.rs`](../../crates/snes-core/src/machine/hdma_ntrl_tests.rs).
  Copyright 2026 snes-rust contributors; [MIT](../../LICENSE).
- Two original 65C816 instruction streams construct zero-filled 32 KiB LoROMs
  in memory. All header/vector bytes, direct/indirect tables, payload colors,
  instructions and numeric expected results are original MIT code/data. No
  external assembler, runtime, font, firmware, commercial ROM, game asset,
  emulator implementation or test binary is used. Generated ROMs are not committed.
- Guests configure during VBlank, poll executed `$4212` reads, write live NTRL
  through CPU instructions, and record immediate readback, subsequent counters,
  table/indirect pointers, CGRAM colors and source registers in WRAM. They cover
  preserved transfer and hold decisions, changed repeat bits, live `$80`/`$00`,
  row reload, fetched-zero termination and a subsequent counter-only write to
  the terminated channel. Host code only supplies the cartridge and observes
  results. It injects no CPU, bus, scheduler or guest memory state. Each run
  stops within 100,000 instructions and fewer than three frame wraps.
- Decision: original sources and generated code/data may be redistributed under
  MIT. Existing fixture pins, notices, hashes, gameplay inputs and licensing
  decisions remain intact. The [Issue #37 validation](../../docs/issue-37-validation.md)
  separates explicit source rules, derived expectations and timing conventions.

## Original WRIO counter-latch CPU guest: approved

- Source: [`crates/snes-core/tests/ppu_counter_latch.rs`](../../crates/snes-core/tests/ppu_counter_latch.rs).
  Copyright 2026 snes-rust contributors; [MIT](../../LICENSE).
- The original encoded 65C816 instructions, numerical expected data, LoROM
  header/reset vector and zero-filled 32 KiB image are constructed in memory
  by Rust during the test. No external assembler, code, runtime, font, firmware,
  commercial ROM, game asset or emulator implementation is used. No generated
  cartridge is committed or downloaded.
- The guest initializes WRIO, polls executed HVBJOY reads and programs a masked
  H/V IRQ to wake WAI at V=256/H=256. It exercises enabled SLHV, falling-edge
  capture between low/high counter reads, disabled SLHV, repeated low writes,
  independent selectors, repeated low/high STAT78 and release/re-enable behavior.
  Numerical WRAM results mask floating high-byte/status bits in guest instructions.
  Host code supplies only the generated ROM and observes WRAM, STP and deadlines:
  a success marker within 100,000 instructions and fewer than three frame wraps.
- Decision: original sources and generated code/data may be redistributed under
  MIT. Existing fixture pins, notices, hashes, licensing decisions and replay
  inputs remain unchanged. The [validation record](../../docs/issue-39-validation.md)
  separates register evidence from retained timing and reset conventions.

## Original VRAM read-access CPU guests: approved

- Source: [`crates/snes-core/tests/ppu_vram_access.rs`](../../crates/snes-core/tests/ppu_vram_access.rs).
  Copyright 2026 snes-rust contributors; [MIT](../../LICENSE).
- Two original encoded 65C816 instruction streams build zero-filled 32 KiB
  LoROMs in memory, one per VMAIN increment-port selection. All instructions,
  sentinel VRAM words, header/vector bytes and expected numeric results are
  original MIT code/data. No external assembler, runtime, font, firmware,
  commercial ROM, game asset or emulator implementation is used. No generated
  ROM is committed or downloaded.
- Guests initialize VRAM exclusively through executed register writes, seed the
  buffer under forced blank, synchronize by polling HVBJOY and masked H/V IRQ
  with WAI, and publish 20 result bytes plus `$18F0=$A5` in WRAM before STP.
  They cover active/HBlank and scanline-zero restrictions, both address writes,
  both data ports, restricted increments and forced/VBlank recovery. Host code
  supplies only the ROM and observes results, STP and explicit bounds of 150,000
  steps and fewer than four frame wraps; it patches no CPU, bus, raster, PPU,
  buffer or guest memory state.
- Decision: original sources and generated code/data may be redistributed under
  MIT. Retained invalid-fetch bits are an emulator approximation, distinguished
  from hardware-supported access/order rules in the
  [Issue #41 validation](../../docs/issue-41-validation.md). All existing fixture
  and game pins, ROM hashes, license decisions, notices and replay inputs remain.

## Original OAM access CPU guest: approved

- Source: [`crates/snes-core/tests/ppu_oam_access.rs`](../../crates/snes-core/tests/ppu_oam_access.rs).
  Copyright 2026 snes-rust contributors; [MIT](../../LICENSE).
- Original encoded 65C816 instructions construct a zero-filled 32 KiB LoROM in
  memory, including all OAM sentinels, header/reset vector and numeric results.
  No external assembler, runtime, font, firmware, commercial ROM, game asset,
  emulator implementation or external test binary is used. No generated ROM
  is committed or downloaded.
- The guest initializes OAM through executed writes, polls HVBJOY and uses
  masked H/V IRQ with WAI to synchronize to active display and HBlank. It
  reads/writes OAM, recovers via forced blank and address writes, observes normal
  VBlank reload and frame-wrap scanline zero, and publishes 20 numeric result
  bytes plus `$18F0=$A5` before STP. The host supplies only the ROM and observes
  results with bounds of 150,000 instructions and fewer than four frame wraps;
  it patches no CPU, PPU, raster, bus or result state.
- Decision: original code/data can be redistributed under MIT. Assertions of
  rendering-owned numeric addresses/bytes and downstream latch effects are
  explicit emulator policy, separated from source-supported blanked controls in
  the [Issue #43 validation](../../docs/issue-43-validation.md). Existing fixture
  and game source pins, hashes, notices, licensing decisions and replay inputs
  remain unchanged.

## Original H=40 OAM reload CPU guest: approved

- Source: [`crates/snes-core/tests/ppu_oam_reload.rs`](../../crates/snes-core/tests/ppu_oam_reload.rs).
  Copyright 2026 snes-rust contributors; [MIT](../../LICENSE).
- Original encoded 65C816 instructions, four OAM sentinel bytes, numeric result
  vectors, LoROM header/reset vector and zero-filled 32 KiB cartridge are built
  in memory. No external assembler, runtime, font, firmware, commercial ROM,
  game asset, emulator implementation or external test binary is used. Generated
  ROMs are not committed or downloaded.
- The guest executes SEI, OAM initialization, H/V IRQ programming and WAI. It
  writes the OAM address during early VBlank, reads before H=40, and observes
  one automatic reload afterward through subsequent reads. Expected bytes are
  independent of rendering-corruption substitutes. It publishes four result
  bytes and `$18F0=$A5` before STP. The host supplies only the cartridge and
  observes results/deadlines, injecting no CPU, bus, raster, PPU or memory state.
  Bounds are 100,000 steps and fewer than two frame wraps.
- Decision: original source/generated code and data may be redistributed under
  MIT. The [Issue #45 record](../../docs/issue-45-validation.md) derives access
  windows under the retained scheduler/IRQ conventions and separates those
  assumptions from source-backed address reload and pair behavior. Existing
  fixture/game pins, hashes, notices, licensing decisions and replay inputs remain.

## Original CGRAM access CPU guest: approved

- Source: `crates/snes-core/tests/ppu_cgram_access.rs`, authored for Issue #47.
- License: repository MIT; copyright 2026 snes-rust contributors.
- Original encoded SEI, LDA/STA, AND/branch, WAI and STP instructions construct a
  zero-filled 32 KiB LoROM in memory. Header, reset vector, palette sentinels,
  numeric results and completion marker are original MIT source/data. No external
  assembler, ROM, font, firmware, game asset or emulator code is incorporated.
- Executed CPU instructions initialize CGRAM, program and acknowledge masked
  H/V IRQs, poll HVBJOY, access visible/HBlank/forced/VBlank/scanline-0 states,
  exercise mixed reads/writes and pending-latch readdressing, and publish eighteen
  WRAM bytes plus `$18F0=$A5`. The host supplies only the cartridge and observes
  results/STP/deadlines: at most 150,000 instructions and fewer than four wraps.
  No raster, memory, phase, latch or expected result is injected into the machine.
- Decision: original source and generated code/data may be redistributed under
  MIT. No ROM is committed. Restricted numeric addresses/bytes, scanline-0
  ownership, full HBlank availability and phase/latch retention assert documented
  emulator policy; they do not establish electrical hardware behavior. The
  accompanying PPU and scheduler/DMA/HDMA numerical vectors are also original MIT.
- Evidence and remaining limits: [Issue #47 validation](../../docs/issue-47-validation.md).

## Original Mode 5/6 hires fixtures: approved

- Sources: `crates/snes-core/tests/common/hires.rs`,
  `crates/snes-core/tests/ppu_hires.rs`,
  `crates/snes-core/examples/hires-fixture.rs`, and `web/tests/hires.spec.js`.
  Copyright 2026 snes-rust contributors; repository [MIT](../../LICENSE).
- All encoded 65C816 instructions, coordinate-labelled planar characters,
  palette/map words, LoROM headers/vectors, numeric expectations and generated
  cartridge bytes are original MIT work. No external assembler, ROM, font,
  firmware, game asset, emulator implementation or reference binary is included.
- The shared builder executes LDA/STA initialization through the emulated CPU,
  including forced blank, VRAM, CGRAM, scroll and main/sub enable registers.
  Static Mode 1/5/6 guests publish `$30=$49` and `$31=mode`; interactive variants
  poll VBlank/auto-joypad completion, select Mode 5/6 while A is held (otherwise
  Mode 1), and publish the selected mode in `$31`. Tests supply cartridges and
  controller input only, with no host register/memory/raster patches.
- Core tests limit each batch to 300,000 instructions and a specified number of
  frame wraps (five for initialization, three per switch). The example command
  in README reproducibly emits six 32 KiB `.sfc` files in `.cache/hires`; browser
  tests build these from the same source. Generated ROMs are not committed.
- Decision: original source and generated graphics/code/data may be redistributed
  under MIT. Bounded passing checks establish the scope described in
  [Issue #49 validation](../../docs/issue-49-validation.md). Existing fixture/game
  pins, licenses, notices, hashes, replays and gameplay/audio expectations remain.

## Original Mode 6 offset guests and graphics: approved

- Source: `crates/snes-core/tests/ppu_mode6_offset.rs`, authored for Issue #51.
  Copyright 2026 snes-rust contributors; repository [MIT](../../LICENSE).
- All coordinate images, planar characters, palettes, map/descriptor words,
  literal vectors, encoded 65C816 instructions and LoROM headers/vectors are
  original source/data. No external assembler, emulator implementation, game
  asset, ROM, firmware or reference binary is incorporated or downloaded.
- `guest_rom` reproducibly constructs eight 32 KiB cartridges in memory:
  neither/H/V/both axes enabled, each with ordinary H scroll 0 or 3. SEI/CLD
  and LDA/STA initialize registers, VRAM and CGRAM through CPU writes under
  forced blank, publish completion `$30=$51`, enable display and loop. The
  host supplies only the cartridge and observes completion, 512x224 geometry,
  literal RGBA pixels in both phases and deadlines (three frames, at most
  150,000 instructions). No host PPU, memory, CPU or raster patches occur.
- Reproduce with `cargo test --locked -p snes-core --test ppu_mode6_offset`.
  No generated ROM binary is committed. Original source/generated code and
  data may be redistributed under MIT. The source-derived static addressing
  rules and remaining uncertainty are in
  [Issue #51 validation](../../docs/issue-51-validation.md); these checks do
  not establish physical fetch/raster behavior or full hardware accuracy.

## Original SETINI pseudo-hires guests and graphics: approved

- Sources: `crates/snes-core/tests/common/pseudo_hires.rs`,
  `crates/snes-core/tests/ppu_pseudo_hires.rs`,
  `crates/snes-core/examples/pseudo-hires-fixture.rs` and
  `web/tests/pseudo-hires.spec.js`. Copyright 2026 snes-rust contributors;
  repository [MIT](../../LICENSE).
- All encoded 65C816 instructions, planar characters, coordinate images,
  palettes/map words, literal pixel expectations and LoROM headers/vectors
  are original MIT code/data. No external ROM, firmware, font, game assets,
  assembler or emulator implementation is incorporated. Generated cartridges
  contain only this original code/data and are not committed.
- The builder emits three 32 KiB cartridges: static SETINI off/on and a
  controller-driven variant. BGMODE stays 1. Executed LDA/STA instructions
  initialize registers, VRAM and CGRAM under forced blank, publish `$30=$53`
  and enable display. The interactive guest polls the next VBlank and
  auto-joypad completion, writes SETINI=$08 while A is held (otherwise $00),
  and publishes that value in `$31`. Hosts supply cartridges/controller input
  and observe state only; they do not patch guest CPU, PPU, memory or raster.
- Core checks allow five startup frames and three frames per switch, bounded
  by 300,000 instructions per batch. The runner checks PPM headers, exact
  payload and literal pixels at both edges and the final row. Actual browser
  Wasm/canvas checks cover both static fixtures, transitions, reset, cartridge
  replacement, memory-view reacquisition and display shape/scale.
- Reproduce using the README's `pseudo-hires-fixture` example command. Source
  and generated code/graphics/data may be redistributed under MIT. The exact
  supported register domain, fixture hashes and unresolved interactions are
  recorded in [Issue #53 validation](../../docs/issue-53-validation.md).

## Original rectangular OBJ guests and graphics: approved

- Sources: `crates/snes-core/tests/common/obj_sizes.rs`,
  `crates/snes-core/tests/ppu_obj_sizes.rs`,
  `crates/snes-core/examples/obj-sizes-fixture.rs` and
  `web/tests/obj-sizes.spec.js`. Copyright 2026 snes-rust contributors;
  repository [MIT](../../LICENSE). All encoded 65C816 instructions, planar
  graphics, coordinate images, palettes, literal expectations and LoROM
  headers/vectors are original MIT code/data. No external ROM, firmware,
  font, graphics or assembler is incorporated or acquired for these fixtures.
- The builder emits two 32 KiB cartridges for OBSEL selectors 6/7. Executed
  CPU instructions initialize relevant registers, VRAM, CGRAM and both OAM
  tables under forced blank, publish `$30=$55`, `$31=selector`, and enable
  OBJ display. Each shows small/large and small/large V-flipped objects,
  below evaluation limits. Hosts supply only the cartridges, advance execution
  and observe completion/pixels; they do not patch CPU/PPU/memory state.
- Core execution is bounded by five frames and 300,000 instructions; STP
  fails. Browser tests have a 30-second timeout and five-second pixel-poll
  deadline. Literal vectors check lower rectangular halves, square-half flip
  boundaries, final rows and adjacent backdrop in both actual Wasm framebuffer
  bytes and canvas ImageData. Independent image crops additionally cover all
  sizes/flips, partial horizontal clipping, Y wrap, character-nibble wrapping,
  both name tables and wrapping OBSEL addresses.
- Reproduce: `cargo run --locked -p snes-core --example obj-sizes-fixture -- .cache/obj-sizes`.
  Generated `.sfc` files stay ignored; source and generated code/data may be
  redistributed under MIT. Output SHA-256:
  - `obj-sizes-6.sfc`: `7e199c553feb40a0b8d31219678fc47e18a0f95836c872b754b21854d642a846`
  - `obj-sizes-7.sfc`: `ea0975ba98501585e928a96e1a42288bd891b44d53fd9d2c74cfe2d6a39b11e6`
- Exact browser reproduction, baseline failures, specification derivations,
  toolchain selection, gate results and retained 24-category inventory are in
  [Issue #55 validation](../../docs/issue-55-validation.md). These bounded checks
  establish neither complete OBJ/hardware accuracy nor commercial compatibility.

## Original OBJ-limit and STAT77 guests and graphics: approved

- Sources: `crates/snes-core/tests/common/obj_limits.rs`,
  `crates/snes-core/tests/ppu_obj_limits.rs`,
  `crates/snes-core/examples/obj-limits-fixture.rs` and
  `web/tests/obj-limits.spec.js`. Copyright 2026 snes-rust contributors;
  repository [MIT](../../LICENSE). All encoded 65C816 instructions, planar
  red/green/blue/transparent tiles, palettes, OAM scenes, literal status/pixel
  expectations and LoROM headers/vectors are original MIT code/data.
  No external ROM, firmware, font, graphics or assembler is incorporated or
  acquired. Source and generated code/data may be redistributed under MIT.
- Four 32 KiB cartridges cover neither flag, RANGE OVER alone, TIME OVER alone,
  and both flags. CPU register writes initialize VRAM/CGRAM/OAM under forced
  blank. Ordinary IRQ/WAI and VBlank polling synchronize status observations;
  CPU instructions mask/read $213E, save nine actual values at $1800..$1808,
  compare independent expected constants, and keep a sticky pass byte at $31.
  Results include repeated reads, a non-overflow line, VBlank, forcing blank,
  a frame boundary under forced blank, disabling blank, next-frame clearing,
  and a subsequent non-overflow line. The CPU then restores the original scene
  through registers and publishes `$30=$57`, `$32=mode`.
- The core host supplies only a cartridge, steps and observes. Startup has an
  eight-frame and 300,000-instruction deadline; STP fails. Three further complete
  frames stabilize literal survivor pixels. The browser loads the same original
  guests through the normal ROM picker, observes CPU results and compares
  literal pixels in actual Wasm framebuffer bytes and canvas ImageData. Each
  has a 30-second test deadline and five-second polling limit. No CPU/PPU/WRAM
  state patches, result-flag injection or new debug APIs are used.
- Reproduce without an assembler:
  `cargo run --locked -p snes-core --example obj-limits-fixture -- .cache/obj-limits`.
  Run `cargo test --locked -p snes-core --test ppu_obj_limits` and, after the
  normal web build, `SNES_TEST_PORT=18085 npm --prefix web test -- --grep 'OBJ limits:'`.
  Generated cartridges stay ignored. SHA-256:
  - `obj-limits-0.sfc`: `7e3d8ebe4b928a838910004c945bb3102dca228e9139b5978a6b96b3f5d18919`
  - `obj-limits-1.sfc`: `1d144b53983a17211b8fa238033739d614dd42db3ffba9ea55f78fa6c1006d16`
  - `obj-limits-2.sfc`: `a5a6a6314d54112aba0265ca94393ccb6123d9edaa7991c3c278b4f46fa8720e`
  - `obj-limits-3.sfc`: `357987c2e96ecdee768b9d336e01ca245c0a2ff86d229fc6a17bfd2a865fd3c6`
- Specification qualifications, baseline counterexamples, commands, actual gate
  results and remaining 24-category scopes are in
  [Issue #57 validation](../../docs/issue-57-validation.md). This bounded work
  does not establish physical evaluation/fetch timing, full OBJ/hardware
  accuracy or commercial compatibility and does not complete Issue #3.

## Original Mode 7 scroll and shared-latch guest: approved

- Sources: `crates/snes-core/tests/common/mode7_scroll.rs`,
  `crates/snes-core/tests/ppu_mode7_scroll.rs`,
  `crates/snes-core/src/ppu_mode7_tests.rs`,
  `crates/snes-core/examples/mode7-scroll-fixture.rs`, and
  `web/tests/mode7-scroll.spec.js`. Copyright 2026 snes-rust contributors;
  repository [MIT](../../LICENSE). Encoded 65C816 instructions, sparse colored
  source stripes, palette, map, LoROM header/vectors and literal result/pixel
  expectations are original MIT code/data. No external ROM, firmware, font,
  graphics or assembler is incorporated or acquired. Both sources and generated
  code/data may be redistributed under MIT. No generated cartridge is committed.
- One 32 KiB cartridge initializes graphics/registers using executed CPU bus
  writes under forced blank. Fourteen multiplication cases run outside Mode 7:
  both offsets, C/D and both centers feeding A's low byte, H/V interleaving,
  signed 16-bit A, and all six planar-only scroll ports preserving M7_old.
  Sixteen NOPs precede each readback; latency is not asserted. The guest saves
  42 actual result bytes to `$1800..$1829`, compares independent literal values
  and clears `$31` on any mismatch. It never repairs that sticky pass byte.
  It then initializes an identity matrix/zero centers and writes H=$15,
  BG2H=$42, H=$00 in Mode 1 before switching to Mode 7. Mode 7 H=21 selects
  red; the incorrect planar H=69 selects green. A stable display publishes
  sticky completion `$30=$59`. No host state patches or new public debug API.
- Native startup is bounded to 300,000 instructions/eight frames; STP fails.
  Three further frames stabilize the final scene and preserve completion/pass.
  Browser execution has a 30-second test deadline/five-second polling bound.
  Both hosts assert literal result bytes and pixels; Chromium also checks
  256x224 geometry, 229376 actual Wasm RGBA bytes and canvas ImageData at the
  same coordinates on rows 0/24/223. Hosts only load, advance and observe.
- Reproduce without an assembler:
  `cargo run --locked -p snes-core --example mode7-scroll-fixture -- .cache/mode7-scroll`.
  Run `cargo test --locked -p snes-core --test ppu_mode7_scroll` and, after the
  normal web build,
  `SNES_TEST_PORT=18085 npm --prefix web test -- --grep 'Mode 7 scroll:'`.
  Generated `mode7-scroll.sfc` SHA-256:
  `31f5383fb1b81201dab672e769a52f5450117d5c84cfc4e65755e3ce57c67610`.
- [Issue #59 validation](../../docs/issue-59-validation.md) records the reference
  contract, exact-baseline failures, independently authored pixel vectors,
  actual gates and remaining uncertainty. Mode 7 remains partial; bounded
  identity checks establish neither full hardware accuracy nor commercial
  compatibility and do not complete Issue #3.

## Original Mode 7 EXTBG composition guest (approved)

- Sources: `crates/snes-core/tests/common/mode7_extbg.rs`,
  `crates/snes-core/tests/ppu_mode7_extbg.rs`,
  `crates/snes-core/examples/mode7-extbg-fixture.rs` and
  `web/tests/mode7-extbg.spec.js`. Copyright 2026 snes-rust contributors;
  repository [MIT](../../LICENSE). The encoded 65C816 program, eight-byte
  original graphics strip, palette, map, header/vectors and literal pixel
  expectations are original MIT code/data. Generated code/data may also be
  redistributed under MIT. No external ROM, firmware, font, graphics or assembler
  is used or acquired; no generated cartridge is committed.
- The 32 KiB LoROM initializes VRAM, CGRAM and registers through executed CPU
  writes under forced blank. Its original repeated strip is
  `$00/$01/$7F/$80/$81/$FF/$01/$FF`. BG1's window at x=0..2 exposes low-priority
  BG2. At x=3, BG2's transparent $80 exposes BG1 direct blue (16 rather than
  paletted 31). High-priority BG2 at x=4..5 covers BG1. BG2's window at x=6..7
  exposes direct BG1 (including $FF's 28/28/24 channels). All display enables
  and windows are set by the guest, never a host patch.
- `$30=$61` marks completed guest initialization, not a manufactured success
  result. Native execution has a 300,000-instruction/eight-frame startup bound,
  rejects STP/APU errors, discards the setup frame and asserts literal pixels
  over three subsequent complete frames. Each frame has the existing 400,000
  clock deadline and a checked 100,000-instruction bound. The browser loads the
  same generated bytes through the ordinary ROM picker and polls for at most
  five seconds inside a 30-second test. It checks actual Wasm framebuffer bytes,
  canvas ImageData, 256x224/229376-byte geometry and completion, with no browser
  errors. Both hosts probe columns 0..7 on rows 24/80/160/223; the browser
  repeats observations separated by animation callbacks. Hosts load, advance
  and observe only; no new public inspection API or injected success state.
- Reproduce:
  `cargo run --locked -p snes-core --example mode7-extbg-fixture -- .cache/mode7-extbg`.
  Run `cargo test --locked -p snes-core --test ppu_mode7_extbg` and, after the
  ordinary web build,
  `SNES_TEST_PORT=18085 npm --prefix web test -- --grep 'Mode 7 EXTBG:'`.
  Generated `mode7-extbg.sfc` SHA-256:
  `12e6ff36091c8cc72521d669d5cb69f66a1ac4dbea20d89e4f3663467c57e705`.
  A separate rebuild compared byte for byte with the browser artifact.
- [Issue #61 validation](../../docs/issue-61-validation.md) separates source facts,
  derived literal expectations, exact-baseline failures, mandatory checks and
  the specific remaining Mode 7 limitations. This bounded evidence neither
  verifies a whole inventory category nor completes Issue #3.

## Original Mode 7 fixed-point transform guests (approved)

- Sources: [`mode7_transform.rs`](../../crates/snes-core/tests/common/mode7_transform.rs),
  [`ppu_mode7_transform.rs`](../../crates/snes-core/tests/ppu_mode7_transform.rs),
  [`mode7-transform-fixture.rs`](../../crates/snes-core/examples/mode7-transform-fixture.rs)
  and [`mode7-transform.spec.js`](../../web/tests/mode7-transform.spec.js).
- Authorship/license: original code, map, packed character artwork and CGRAM
  palette by snes-rust contributors, copyright 2026, under this repository's
  MIT license. No external artwork, commercial ROM, firmware, assembler or
  emulator implementation is an input. The Rust builder emits original 65C816
  instructions into a 32 KiB LoROM; only source is committed.
- All PPU initialization is performed by executed CPU writes under forced blank.
  A full 128x128 map selects green character 1 except cell (0,0), which selects
  character 2. Character 2's first pixel is red and its other pixels green.
  Red/green palettes are initialized independently at indices 1/2 and 129/130.
  This asymmetric sparse map distinguishes source columns 0, 1 and 512;
  distant map entries do not repeat a short strip. D=B=C=0 makes every output
  row sample source row 0, independent of physical row origin.
- Fractional variants set A=$0020, H=1: display x=7 must sample source X=0
  (raw $81, red), while the old transform samples X=1 (raw $82, green).
  Clipped variants set A=$0080,H=1024: display x=0 must sample X=0 rather
  than 512. Each has BG1-only and EXTBG BG2-only variants. Completion at
  $30=$63 means the CPU reached the end of setup, not a host-injected pass.
- Native tests load these exact builder bytes, bound startup, reject CPU/APU
  errors, discard setup output and check three complete stable frames per
  variant. Chromium loads the same generated files through the ordinary ROM
  picker and checks completion, 256x224 geometry, real Wasm framebuffer bytes
  and canvas ImageData under the mandatory browser filter.

Reproduction (both builds were performed and compared byte for byte):

```sh
cargo run --locked -p snes-core --example mode7-transform-fixture -- .cache/mode7-transform
cargo run --locked -p snes-core --example mode7-transform-fixture -- .cache/issue-63/rebuilt
for name in fractional-bg1 fractional-bg2 clipped-bg1 clipped-bg2; do
  cmp ".cache/mode7-transform/$name.sfc" ".cache/issue-63/rebuilt/$name.sfc"
done
shasum -a 256 .cache/mode7-transform/*.sfc
```

| Generated file (32768 bytes each) | SHA-256 |
| --- | --- |
| fractional-bg1.sfc | `bf3c6b6a946c644f7ffdcc1284c66bf8810909b7d903f5a6e33f749f6a60413b` |
| fractional-bg2.sfc | `c6da270403f6de845081273f750b4ba3a520ece11c53ce3a607f8cbf50f66434` |
| clipped-bg1.sfc | `099b9eb111833c85d9bb113539aedeecee9b5bc1119d7fbf2a687e74a00e256c` |
| clipped-bg2.sfc | `cfb1acc787a5e73f2ba0560812d47b24b9ef24aafa701abe7398e78f258b6cb7` |

See the [Issue #63 validation record](../../docs/issue-63-validation.md) for
specification reconciliation, literal coordinate derivations, exact-baseline
counterexamples and remaining uncertainty. These software checks do not
establish physical hardware accuracy, commercial compatibility or Issue #3 completion.


## Original Mode 7 screen-row guests (approved)

- Sources: [`mode7_row.rs`](../../crates/snes-core/tests/common/mode7_row.rs),
  [`ppu_mode7_row.rs`](../../crates/snes-core/tests/ppu_mode7_row.rs),
  [`mode7-row-fixture.rs`](../../crates/snes-core/examples/mode7-row-fixture.rs),
  [`ppu_mode7_transform.rs`](../../crates/snes-core/tests/ppu_mode7_transform.rs)
  and [`mode7-row.spec.js`](../../web/tests/mode7-row.spec.js).
- Original authorship: copyright 2026 snes-rust contributors, [MIT](../../LICENSE).
  All 65C816 instructions, sparse asymmetric artwork, palette, map and header
  are original source. There is no external program, asset, font, firmware,
  assembler or compiler runtime input. Only source is committed.
- Four 32768-byte NTSC LoROMs vary BG1/BG2-only routing and normal/V-flipped
  sampling. Executed CPU instructions initialize the complete map and characters
  during forced blank, set A=B=D=256/C=0 with zero offsets/centers, leave blank,
  publish $30=$65 and loop. Six distinct map cells contain one red/blue/yellow
  marker each; all adjacent pixels/rows and the rest of the map are green.
  The same bytes run through native Machine, native runner and ordinary browser
  ROM picker. Hosts never inject registers or raster state.
- Tests bound startup by 300000 instructions/eight frames, reject CPU/APU errors,
  discard setup and inspect three complete stable frames. The browser repeats
  actual Wasm and canvas observations, checks completion/geometry/errors and is
  included by the existing mandatory filter. See [validation](../../docs/issue-65-validation.md).
- Decision: original source and generated ROMs may be redistributed under MIT;
  generated cartridges stay ignored. No commercial ROM or firmware is acquired.

Build twice, compare all four byte streams, and hash:

```sh
cargo run --locked -p snes-core --example mode7-row-fixture -- .cache/mode7-row
cargo run --locked -p snes-core --example mode7-row-fixture -- .cache/issue-65/rebuilt
for name in normal-bg1 normal-bg2 flipped-bg1 flipped-bg2; do
  cmp ".cache/mode7-row/$name.sfc" ".cache/issue-65/rebuilt/$name.sfc"
done
shasum -a 256 .cache/mode7-row/*.sfc
```

Both builds compared byte for byte on 2026-10-24:

| Cartridge | SHA-256 |
| --- | --- |
| `normal-bg1.sfc` | `d81e1b3824bb89358aec8f035ecb5fcf9c693d8ba36f35e9e66b8f4c2c16a8c3` |
| `normal-bg2.sfc` | `78575505f180f37862df4c3c929dd151293838c99f33c4d3a09e9646479a3f7c` |
| `flipped-bg1.sfc` | `a4fd758dbb5993e5705e4aa811d1b058989a45817f7611c9b6b53c3c6c77bed1` |
| `flipped-bg2.sfc` | `12df473a6ec05bffc8efccb5cc25c55bad5f6a2917be8e782c0a9ed8276328cb` |

The #59 scroll, #61 EXTBG and #63 fractional/clipped guest builders and their
hashes remain unchanged. #61's source map ends at row 223: the corrected first
screen row is 1, so framebuffer row 223 samples the uninitialized map row 28
and shows its gray backdrop. Native/browser expectations now assert this,
while also retaining the full composition checks on framebuffer row 222.

## Issue #67: original manual two-pad serial guest

- Source: [`joypad_serial.rs`](../../crates/snes-core/tests/joypad_serial.rs),
  `guest_rom` and `cpu_guest_records_both_manual_sequences_in_wram`.
  Original copyright 2026 snes-rust contributors, [MIT](../../LICENSE).
- All 65C816 instructions and LoROM header bytes are original. No external
  program, graphics, font, sound, firmware, assembler or compiler runtime is
  used. The 32768-byte cartridge is built in memory; only source is committed.
- The guest executes SEI, disables automatic polling through $4200, writes the
  shared latch high then low, and reads port 1 seven times and port 2 three times
  before interleaving their remaining reads. Long loads use $80:4016/$81:4017;
  AND/STA instructions store 24 numeric bits per pad at $1800/$1840. Both
  sixteen-bit sequences and eight trailing ones are checked against literal
  $A590/$5A60 expansions. $18F0=$A5 plus STP marks completion.
- The host supplies only the cartridge and the two button inputs, steps normal
  CPU/scheduler execution, and observes WRAM at completion. Deadlines are 10000
  instructions and fewer than two frames; no serial, raster, or result state is
  injected. These are manual protocol checks, not automatic polling accuracy.
- The existing original MIT [`dma_access.rs`](../../crates/snes-core/tests/dma_access.rs)
  guest also attempts prohibited DMA reads from each controller register, then
  records four CPU-read bits at $1811–$1814. Inputs $8000/$4000 must yield
  `[1,0,1,0]` in port order 1,2,2,1, establishing neither sequence advanced.
  Existing guest observations and assertions are retained. Excluded-read bytes
  retain the documented zero approximation; no new electrical-value claim.
- Decision: source and generated cartridges are redistributable under MIT.
  No commercial ROM or firmware was acquired.

## Issue #69: original automatic two-pad guests and polling corrections

- Source: [`joypad_auto.rs`](../../crates/snes-core/tests/joypad_auto.rs),
  `cartridge`, `cycle`, `guest`, and its two host tests. Original copyright
  2026 snes-rust contributors, [MIT](../../LICENSE). Cargo builds the original
  Rust builder; it encodes the original 65C816 instructions and 32 KiB LoROM
  header directly in memory. No external assembler, runtime, graphics, font,
  sound, firmware, test ROM, or emulator implementation is used.
- The guest executes SEI, forced blank, OUT0 low and automatic enable. It
  explicitly observes busy set and then clear, saves all eight result bytes
  using system-bank-mirrored long loads into $1800–$1807, and saves masked
  start/completion status at $1808/9. The two-cycle guest marks $18F0=1,
  waits for the next automatic cycle, records $1810–$1819, and compares
  all eight second-cycle result bytes with the swapped input expectations.
  $18F1=$EE plus STP reports failure; $18F0=$A5 plus STP reports completion.
- The host supplies only ROM and buttons ($A59F/$5A6F, then $5A60/$A590 after
  observing the first marker); normal CPU/scheduler execution advances time.
  It observes WRAM and post-poll CPU-bus serial reads, without private raster,
  latch, serial or result injection. Deadlines: 150000 instructions, fewer
  than three frames; CPU/APU failures are rejected. Reproduce with
  `cargo test --locked -p snes-core --test joypad_auto`.
- Existing original guests in `dma_access.rs`, `common/hires.rs`,
  `common/pseudo_hires.rs`, and `tools/make-fixture.py` previously assumed busy
  was asserted at VBlank/NMI. They now wait for busy assertion before clearing;
  all numeric, pixel, video, audio and input assertions remain. The smoke
  binary is rebuilt with its Python source, checksum and recorded hash. Static
  hires/pseudo-hires cartridges and both cleared game sources/hashes are unchanged.
- Current interactive fixture SHA-256 values (the historical Issue #49/#53
  validation hashes describe the earlier versions):

| Cartridge | SHA-256 |
| --- | --- |
| hires-1-switch.sfc | `069a7e75c05870413b50d1197e6c370d83ea157e1057138ca1a56ccfe7a665ad` |
| hires-5-switch.sfc | `652b773ff4e4b05ecc5337dab9e2ab51c01fea662467e0b6dc81d0ced468d3b2` |
| hires-6-switch.sfc | `a5f85db717214c4e69766c1a6cc1bdbf91915c6c0f514c193f8a31a384024501` |
| pseudo-hires-switch.sfc | `24f879c0e583474636df8844efb547f86cde708477643b661e30fe1027b8f52f` |

Rebuild using the existing README example commands. Only original source and
our rebuilt smoke binary are committed. Original/generated artifacts remain
redistributable under MIT; no commercial ROM or firmware was acquired.
The [event/evidence record](../../docs/issue-69-validation.md) distinguishes
logical checks from unverified electrical and subcycle timing behavior.

## Issue #71: original FIR/feedback SPC700 guest and echo vectors

- Source: [`dsp_echo.rs`](../../crates/snes-core/tests/dsp_echo.rs), `guest`,
  `echo_data` and the host tests; focused unit vectors in
  [`dsp_echo_tests.rs`](../../crates/snes-core/src/dsp_echo_tests.rs).
  Authored for this repository by snes-rust contributors, copyright 2026,
  [MIT](../../LICENSE). Cargo compiles the Rust instruction builder; it encodes
  original SPC700 instructions directly. No external assembler, executable,
  firmware, music, BRR asset or commercial ROM is acquired or embedded.
- The original data are repeated little-endian stereo words 16386/-8194 in
  `$6000..$67FF`. The host uploads code at `$0200` using the existing firmware-free
  port protocol and supplies initial echo data only. The guest executes DSP
  register writes through `$F2/$F3`, enables nonzero FIR7/EFB, and copies four
  echo RAM bytes per command into `$1000..$100B`. It acknowledges commands 1–4,
  then executes STOP. No private history, echo offset or expected results are
  injected. The host observes RAM to anchor the first write, then checks numeric
  PCM and guest-recorded RAM through delay/history wraps.
- Upload length is below 256 bytes, ready occurs before 100 samples; each
  acknowledgement is bounded by 1,000 calls of 21 master clocks. Each advance
  has a 1,200,000-call bound, completion is before first-write + 1,650 samples,
  STOP and absence of core errors are asserted. The separate partition test
  compares all PCM, DSP registers and echo RAM for fixed EDL 0/1.
- Arithmetic derivation, endpoint attribution and checks are in the
  [Issue #71 record](../../docs/issue-71-validation.md). Only specification facts
  were used; no external emulator implementation code was imported. Original
  source and generated in-memory data are redistributable under MIT.

## Issue #73: original ordered-mixing SPC700 guest and vectors

- Source: [`dsp_mixing.rs`](../../crates/snes-core/tests/dsp_mixing.rs), authored
  for this repository by snes-rust contributors, copyright 2026,
  [MIT](../../LICENSE). Cargo builds the Rust instruction builder, which encodes
  original SPC700 instructions directly. Guest generation uses no external
  assembler, firmware, commercial ROM, emulator implementation, music or BRR asset.
- The host uploads original code at $0200 through the existing firmware-free
  protocol. The guest writes $F2/$F3 to configure held noise, direct GAIN and
  four voices (0,1,2,7), keys them on, produces signed stereo overflow, enables
  late-voice cancellation, then changes EON to select the overflowing subset
  and the cancelling voice independently. It reads echo RAM and EON into four
  five-byte records at $1000..$1013, acknowledges commands 1–5, and executes
  STOP. No expected data, private voices, accumulators or DSP history are injected.
- Source -32768, GAIN=64 and signed volumes -128/+127/+64/-64 are original
  numerical fixtures. Echo starts as zero RAM and is written by the guest's DSP
  configuration. Literal PCM and little-endian expectations are derived in the
  [validation record](../../docs/issue-73-validation.md), including independent
  rounding, saturation and feedback calculations. These data are MIT as well.
- Upload is below 512 bytes with byte-counter wrap; each acknowledgement is
  bounded by 1000 advances of 21 master clocks, readiness before 200 samples and
  standalone completion before 350 samples. STOP, observable ENVX/OUTX, recorded
  EON/echo bytes, steady PCM and no core error are asserted. A second test runs
  the same guest through all mixing phases for fixed EDL=0/1, checking exact
  cadence and every PCM sample, all DSP registers and relevant echo RAM across
  whole and irregular APU tick partitions. Reproduce with
  `cargo test --locked -p snes-core --test dsp_mixing`.

## Issue #79: original terminal-header SPC700 guest and BRR vectors

- Source: [`dsp_brr_end.rs`](../../crates/snes-core/tests/dsp_brr_end.rs), with a
  narrow private history check in [`dsp.rs`](../../crates/snes-core/src/dsp.rs).
  Authored for this repository by snes-rust contributors, copyright 2026,
  [MIT](../../LICENSE). Cargo compiles the original Rust instruction builder;
  no external assembler, firmware, commercial ROM, music, BRR asset or emulator
  output is used. Original instruction bytes, directory entries, BRR payloads
  and numerical expectations are redistributable under MIT.
- Filter-0 range-10 BRR uses original signed nibbles, sequential and terminal
  blocks, and a 64-block chain for a stable ENDX clear/read interval. The guest
  uploads at $0200 using the existing firmware-free protocol and writes/reads
  DSP through $F2/$F3. Commands record ENVX/OUTX/ENDX at $1000..$100e, clear ENDX,
  edit the live loop word, change GAIN/ADSR/SRCN while released, retrigger via
  KON and stop. No private DSP state or expected results are injected.
- Code is below 512 bytes; each acknowledgement has a 1000-by-21-master-clock
  bound, readiness is before sample 100, and STOP is required before sample 1510.
  Independent stereo and register anchors cover terminal silence, clear/reassert,
  no revival without KON and restored audio. Whole versus irregular APU tick
  partitions compare every PCM sample, all DSP registers, all RAM and ports at
  fixed checkpoints. This establishes determinism, not physical timing.
- [Issue #79 validation](../../docs/issue-79-validation.md) records numerical
  derivations, source reconciliation (including conflicting ENDX timing wording),
  behavioral failures in an isolated baseline worktree, final checks and limits.
  The pinned LGPL research source was read only for attributed hardware facts;
  no implementation code, binary or output was copied, imported or executed.
  Existing cleared-game sources, licenses, pins, hashes and replay assertions
  remain unchanged. Reproduce with
  `cargo test --locked -p snes-core --test dsp_brr_end`.

## Issue #81: original zero-pitch terminal-header vectors and SPC700 guest

- `crates/snes-core/tests/dsp_brr_end.rs` adds four tests authored for this
  repository by the snes-rust contributors in 2026, under the root MIT license.
  All new instruction bytes, command sequences, BRR data and numeric vectors
  are original. The guest reuses the existing firmware-free uploader and
  original filter-0 sample/directory builders; no firmware is included.
- The zero/low-pitch matrix crosses all four initial header flags, direct GAIN
  and fast ADSR, and NON on/off with PMON disabled. Held-noise expectations are
  independently calculated from signed source -32768 and envelope arithmetic.
  Release, KOFF, two-voice isolation, silent end chains, stable loop-word edits
  and distinct-start KON controls use original data and explicit scalar counts.
- `zero_pitch_guest` uploads original SPC700 code to $0200, configures DSP state
  through $F2/$F3 and records ENVX/OUTX/ENDX triples at $1100..$1117. Eight bounded
  commands test initial zero-pitch terminal silence, envelope/NON edits, pitch
  resume/hold, ENDX clear/reassert and retrigger into a distinct nonterminal start.
  Independently derived PCM/register anchors accompany whole/irregular APU tick
  comparisons of every PCM sample, all DSP registers, RAM and output ports.
- [Issue #81 validation](../../docs/issue-81-validation.md) attributes the
  hardware semantics and numerical arithmetic. Anomie/jwdonal, Fullsnes and
  Shay Green/blargg's pinned LGPL research implementation were consulted only
  as attributed evidence. No external implementation code, expected emulator
  output, ROM, firmware, audio, graphics or assets were incorporated. Tick
  equality proves determinism only; these vectors do not establish physical
  header-prefetch/publication timing or broad hardware accuracy.

## Issue #83 original SPC700 timer vectors and guest

`crates/snes-core/tests/spc_timers.rs` is original work by the snes-rust
contributors, Copyright 2026, under this repository's MIT license. It constructs
public-API vectors and an SPC700 instruction stream in Rust; no binary fixture
or assembler is needed. The guest is uploaded to RAM through the existing
firmware-free port protocol. It writes timer targets, counts with original
bounded instruction-delay loops, reads timer outputs into RAM, and signals
completion through a port. It also writes two DSP registers as controls.

Expected pulse distances, modulo counts and guest records come from the
attributed hardware contract and independent arithmetic in
[Issue #83 validation](../../docs/issue-83-validation.md). No commercial ROM,
Nintendo firmware, emulator implementation, or captured emulator output was
imported as a fixture or oracle.
Fullsnes and Anomie's attributed hardware research were consulted as
documentation only. Existing source/tool pins, fixtures and game hashes are
unchanged.

## Issue #85: original timer-phase vectors and uploaded SPC700 guest

- Additions to [`spc_timers.rs`](../../crates/snes-core/tests/spc_timers.rs) are
  original work by the snes-rust contributors, copyright 2026, under the root
  [MIT license](../../LICENSE). Cargo compiles the Rust instruction builder;
  only source is committed. The guest encodes original SPC700 instructions
  directly in memory, with no external assembler, executable fixture, firmware,
  commercial ROM, music, graphics or emulator output.
- Public-API matrices reach selected phases and counting/output states on each
  timer through ordinary elapsed time and register operations. `phase_guest` uploads at $0200
  via the existing firmware-free port protocol, writes targets and CONTROL,
  uses bounded instruction-count delays, records 20 output bytes at
  $1100–$1113, and reports port 0=$85 plus STOP. No private timer state or
  expected output is injected. Generated code/data are redistributable as MIT.
- [Issue #85 validation](../../docs/issue-85-validation.md) derives the timer
  expectations from freshly inspected Anomie revision 1126 and Fullsnes
  passages, distinguishes reset stages and documents unresolved physical
  edges. Documentation was consulted for hardware facts only; no external
  implementation was imported or run as an oracle. Whole/irregular runs each
  require numeric records before partition equality is treated as determinism.
- Existing #83 records, cleared-game sources/licenses, source/tool pins, hashes,
  gameplay/freeze, note/effect, Web Audio and queue/drift assertions are retained.
