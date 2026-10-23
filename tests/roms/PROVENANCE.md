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
- SHA-256: `73c0bad128a030ced5ef160a4e8e3df46de545c3e1c166a44e9d5968798b82fb`.
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
- Decision: original source and generated guest/data may be redistributed under
  MIT. This is a bounded envelope diagnostic, not full DSP or game acceptance.

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
