# ROM and build provenance

No commercial ROM, Nintendo firmware, external game ROM, or third-party emulator
implementation is distributed. `.sfc`, `.smc`, saves, downloads, compiler sources,
build products and screenshots are ignored. A file being available to download
is not a redistribution decision. The only committed binary is our original
`smoke.bin` diagnostic. The game audits below remain incomplete.

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
- Build: ca65/ld65, pcx2snes, ucon64, Tiled, python3-tmx and ImageMagick are listed
  upstream. Release tool versions, generated graphics, fonts and exact submodule
  pin must be matched to `v1.04`; this release has not been reproduced locally.
- Decision: user-supplied development image only. Primary code/art rights are
  documented; full binary/build/font clearance is pending. No audio is expected
  from this selected demo; silence here is not the audio acceptance case.

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
