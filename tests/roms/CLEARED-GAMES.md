# Audited source-built games

The cleared artifacts are **modified builds**, not the previously investigated
upstream releases. [cleared-games.json](cleared-games.json) is the complete
allowlist of fetched paths, their SHA-256 values, repositories and immutable
commits, selected linked assembly modules, and expected final ROM hashes.
Every URL is `https://raw.githubusercontent.com/REPOSITORY/COMMIT/PATH`.
Nothing fetches a prebuilt ROM. Game binaries and build products remain ignored.

| Artifact | Bytes | SHA-256 |
| --- | --- | --- |
| `.cache/cleared-games/castle/castle.sfc` | 131072 | `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869` |
| `.cache/cleared-games/memory/memory.sfc` | 131072 | `08bc46ab6ac44adee7b593cdbc6d30a2dadaefd37b5040296b952b8893d630c7` |

Decision: allow automatic acquisition of **only the listed sources**, building
and redistribution of these modified images with their generated `NOTICE.txt`.
This decision does not clear either original release image. All game/library
code is assembly; no C runtime, emulator, Nintendo firmware, third-party audio
driver, soundbank, recorded sample or external font is linked into either image.
Memory includes the original sound extension described below.

## Castle Platformer

- Game: <https://github.com/undisbeliever/castle_platformer>,
  `847e75261c394e33deff21232cd1c051dca04e30` (v1.04).
- Game runtime: <https://github.com/undisbeliever/1gam-common>,
  `02f661ca902d6663cf43d9aacb4b43acf83a34ff`, the exact release submodule.
  Only the manifest's includes, headers, linker configuration and selected
  routines are fetched; examples, fonts and SNESGSS/audio are excluded.
- Game code, maps, entity/animation definitions, Python map converters and
  runtime: MIT, Copyright (c) 2015 The UnDisbeliever
  <undisbeliever@gmail.com>. Both LICENSE files have SHA-256
  `b30f671b26adb2f5dd3b272296a39465c72eab7d5a42b72a4592adc5d118daeb`;
  the full notice is retained in [castle-MIT.txt](licenses/castle-MIT.txt).
- Exact art inputs: five `resources/entities/*.pcx` sprite sheets and
  `resources/metatilesets/castle_platformer.pcx`. Each accompanying `.source`
  record is fetched, hash-checked and retained. The author records the
  adaptations (including resizing the player) and each source's CC0 terms.
  These match the artists' own distribution pages:
  [Jetrel](https://opengameart.org/content/castle-platformer),
  [qubodup](https://opengameart.org/content/simple-svg-ice-platformer-tiles-16x16-16x96-96x16),
  [Jason-Em / GrafxKid](https://opengameart.org/content/classic-hero-and-baddies-pack).
  Applicable art dedication: [CC0 1.0](https://creativecommons.org/publicdomain/zero/1.0/).
  No external art archive or editor is needed; the pinned adapted PCXs are
  the build inputs, not undocumented intermediate products.
- The original `credits.pcx` and `credits.xcfgz` are **not acquired or used**.
  Their GIMP Sans/Sans Bold font uncertainty therefore cannot affect this ROM.
  Our original 3x5 lettering replaces that entire screen, retains creator/art
  attribution and identifies the build as modified. Gameplay art is retained.
- Two assembly compatibility alterations are explicit in the build script:
  declare `.A16` after reset's REP, and translate two old bare,
  destination-first MVN bank operands to ca65 2.19's immediate, source-first
  notation. They express the intended reset and entity-copy operations;
  neither changes the emulator. No source game logic is replaced.
- `utils/map2metatile1x16.py` and `utils/tmx2metamap1x16.py` remain pinned MIT
  upstream code. An original adapter supplies their XML data through Python's
  standard library, eliminating the old python3-tmx/Tiled build dependency.

## Memory Game: replacement for the blocked Gothicvania release

- Game: <https://github.com/undisbeliever/memory-game>,
  `0a43ebc04dbe243c328b914cbbeb661008907372`.
- Runtime submodule: <https://github.com/undisbeliever/1gam-common>,
  `0be45f5dbfe69e03c25e91c62186afa138ea1924`. No other runtime is linked.
- Code and game artwork: MIT, Copyright (c) 2015 The UnDisbeliever,
  same notice/hash as above. The pinned README explicitly licenses **the
  game**, not just its code. The title's source also identifies it as MIT.
- Asset inventory: `cards.pcx`, its editable `cards.svgz`, and `door.pcx`.
  The SVG contains geometric card symbols and no text, embedded raster image,
  font reference or external image reference. The door is a geometric shutter
  animation. No third-party art, audio, font or asset submodule is referenced
  by these inputs. The exact files and hashes are in the allowlist.
- The upstream `sprites.pcx` is excluded, including its raster lettering.
  Our original corner cursor and hand-authored font replace it, preserving
  the game's Press Start, Game Over and You Win messages. No host font is used.
- Change only the configuration's PAL region declaration to NTSC. The game's
  own region-aware timer then selects 60 Hz. The core still rejects PAL images;
  this modified NTSC build does not establish PAL support.
- This is a distinct 30-card matching puzzle with selection, matching,
  win/loss and restart behavior, not our diagnostic ROM or a variant of Castle
  gameplay. Both games are by the same author and share a game library; they
  do not establish coverage of two unrelated game-runtime implementations.
- Gothicvania is replaced because its linked WDC/Linux/Flipcode runtime terms
  and complete audio/build provenance remain unresolved. Its original image
  stays user-supplied and excluded from CI. Its prior optional audio checks
  remain investigation evidence, not cleared-game audio acceptance.

## Original sound extension in the modified Memory Game

Upstream Memory Game is silent. This build adds original sound effects for card
reveals, matching pairs, winning and choosing a wrong pair. These are audible
feedback for real game events, not an emulator-side tone or the diagnostic ROM.
The game uploads an original SPC700 program through the IPL protocol, then sends
pitch/sequence commands through the APU ports when its game routines execute.
The SPC program programs a BRR voice and uses timer 0 to key it off after roughly
125 ms. The title and idle gameplay are silent. No background music is added;
this does not demonstrate compatibility with an upstream third-party music driver.
The Rust core, Wasm adapter and audio playback path have no game-specific changes.

All driver instructions, waveform data, upload code and event integration are
original MIT work, Copyright (c) 2026 snes-rust contributors. The applicable full
notice is [the repository LICENSE](../../LICENSE), also copied to each generated
`NOTICE.txt`. No sound recording, external instrument, firmware, external sound
compiler or additional submodule is used. The 16-sample triangular waveform is
specified directly as signed nibbles with BRR filter 0, range 9 and loop flags.
The existing pinned ca65/ld65 tools assemble the main CPU extension; Python emits
the annotated SPC instruction bytes without another assembler.

| Exact original build input / derived artifact | SHA-256 |
| --- | --- |
| `tools/memory-audio.py` (tracked original source; hash enforced by builder) | `302ba82e4a5da5ef0dcf1613bd4c0cbedb4b7994d2b59e44aaa33b6302ac00d4` |
| `.cache/cleared-games/memory/memory-spc.bin` (281 bytes, program/directory/BRR) | `54dbf2fe274b14bc8ff7e0c63a4468d2030cf53656ffe39f922ccfe433ff33a1` |
| `.cache/cleared-games/memory/src/audio.s` (generated 65C816 upload/command code) | `975b5e6213dd6a43b3a2838a2f3a6107b9503a80d776eddaf94cd831b30827eb` |

The tracked generator also inserts calls in the pinned `src/main.s` and
`src/gameloop.s`. It preserves the existing game decisions and adds audio at
`EnterState_SelectSecond`, `EnterState_WaitForDoors` and the match/loss/win
branches of `CheckMatch`. Redistribution of this exact modified build with both
MIT notices remains allowed; it does not clear any original upstream release.

## Build and validation

```sh
python3 tools/build-test-tools.py
python3 tools/build-cleared-games.py
python3 tools/build-cleared-games.py
npm --prefix web run build
npm --prefix web test -- --grep 'cleared '
```

ca65/ld65 come from the exact cc65 archive pinned in
[toolchains.json](toolchains.json), zlib, cc65 contributors. These are assembly
tools and link no library/runtime into either game. Python 3.12+ and the
standard library are the only conversion dependencies. No pcx2snes, ucon64,
ImageMagick, GIMP, font installation or platform graphics library is needed.
Original PCX RLE decoding, palette allocation, SNES planar packing, checksum
calculation and replacement glyph data are in `tools/build-cleared-games.py`,
MIT, Copyright (c) 2026 snes-rust contributors, [full notice](../../LICENSE).
All game data is rendered by the same Rust core. No game identifier or fix
was added to the core or product UI.

Each build starts from verified original sources in a fresh build directory.
It verifies the final ROM SHA-256. Repeating the build checks reproducibility;
CI does both builds before browser tests. `NOTICE.txt` accompanies each ROM
and contains both MIT notices, art attribution, the CC0 URL and alterations.
If redistributing an image, include that file; the MIT notices are required.

The Chromium tests require these exact ROM hashes. Castle's 18,100-frame replay
asserts movement, grounded jumps, active gameplay, changing video, bounded audio
cadence and death/restart transitions in every complete 900-frame window.
Its separate fixed joypad recording in
[`castle-route.json`](../../web/tests/castle-route.json) starts from reset,
operates the first map's three chain switches and the third map's bridge switch,
traverses all five maps and reaches the modified build's scrolling credits.
Assertions verify changed map tiles, ordered map transitions, movement/jumps,
changing game-derived frames, checkpoint hashes and the completed credits scroll.
Unexpected death fails the recording. No RAM writes, save-state restores, ROM
patches or emulator shortcuts are used by this test. This is an automated route,
not a claim of an unaided human playthrough or exhaustive gameplay coverage.
Memory's equally long run uses reads of the shuffled card grid to choose
matches, then performs all actions through joypad input, without changing RAM.
Every window asserts cursor movement, card selection, active play, changing
video and more than 10,000 nonzero finite audio samples with RMS above 0.001; the
run must win and restart more than once. A separate interactive test uses the
normal ROM picker, Play gesture and keyboard input to verify non-silent game
buffers are scheduled through Web Audio. It checks title silence, a card sound,
the effect ending while idle, pause and reset returning to a silent title. Input
selection is automated play, not a claim that an unaided human completed the game.

Castle remains silent; Memory now produces original game sound effects.
**Issue #3 remains incomplete:** the remaining timing/PPU/audio fidelity checks
are still required. Castle's finish/credits progression is now covered. License-cleared
gameplay here does not certify the previous release ROMs, full hardware accuracy or commercial
cartridge compatibility. No commercial ROM is shipped or fetched.
