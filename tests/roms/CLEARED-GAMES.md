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
| `.cache/cleared-games/memory/memory.sfc` | 131072 | `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1` |

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

Upstream Memory Game is silent. This build adds original looping gameplay music
and sound effects for card reveals, matching pairs, winning and choosing a wrong
pair. These are audible feedback for real game events, not an emulator-side tone or the diagnostic ROM.
The game uploads an original SPC700 program through the IPL protocol, then sends
pitch/sequence and music start/stop commands through the APU ports when its game
routines execute. Timer 0 keys the effect voice off after roughly 125 ms; timer 1
advances a separate BRR music voice every 250 ms. An original 16-beat, four-second
C-major tune loops during play, including idle card selection. Its two answering
phrases, pitches and rests are written directly in `tools/memory-audio.py`; it is
not an arrangement of an external composition. Music starts when Start opens the
cards and stops on win/loss/title. Final win/loss effects can finish on the title;
boot/reset titles are silent. Music is centered, effects are left-panned, allowing
tests to distinguish them from stereo waveform output. This does not demonstrate
compatibility with an upstream third-party music driver.
The Rust core, Wasm adapter and audio playback path have no game-specific changes.

All score data, driver instructions, waveform data, upload code and event integration are
original MIT work, Copyright (c) 2026 snes-rust contributors. The applicable full
notice is [the repository LICENSE](../../LICENSE), also copied to each generated
`NOTICE.txt`. No sound recording, external instrument, firmware, external sound
compiler or additional submodule is used. The 16-sample triangular waveform is
specified directly as signed nibbles with BRR filter 0, range 9 and loop flags.
The existing pinned ca65/ld65 tools assemble the main CPU extension; Python emits
the annotated SPC instruction bytes without another assembler.

| Exact original build input / derived artifact | SHA-256 |
| --- | --- |
| `tools/memory-audio.py` (tracked original source; hash enforced by builder) | `9374c75f76f226d2ca6f9da7cd8ebcb01e01179d357fb93c268eb1bcddfffb65` |
| `.cache/cleared-games/memory/memory-spc.bin` (537 bytes, program/score/directory/BRR) | `8824c7fb3e9bd776d1ee6c21d1fbbffc53ca24e25d57e8eb1c6dd938b2c809e4` |
| `.cache/cleared-games/memory/src/audio.s` (generated 65C816 upload/command code) | `69f4294df9269ccb2a0e7feb0ccbebcc0781c48f526d99ea2ff33492768f908d` |

The tracked generator also inserts calls in the pinned `src/main.s` and
`src/gameloop.s`. It preserves the existing game decisions and adds audio at
`EnterState_SelectSecond`, `EnterState_WaitForDoors` and the match/loss/win
branches of `CheckMatch`. Music starts at `EnterState_OpenAllDoors` and stops
on each assignment of `PRESS_START`. Redistribution of this exact modified build with both
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
run must win and restart more than once. Every window separately requires music
and effect RMS above 0.001. A separate idle-gameplay waveform test records 600
frames (about ten seconds), recovers the notes from positive zero crossings and
checks their order, rests, quarter-second durations and looping. It then reveals
a card and verifies a finite effect while music continues. Its JSON evidence and
32 kHz stereo `music-and-card-effect.wav` are generated in `web/test-results`.
The first approximately ten seconds contain music alone, followed by a card
reveal and two seconds of continuing music. A separate real-time interactive
test uses the normal ROM picker, Play gesture and keyboard input to verify
music and effect buffers are scheduled through Web Audio. It checks title
silence, more than one full music loop while idle, a card sound, the effect ending
while music continues, pause and reset returning to a silent title. Input
selection is automated play, not a claim that an unaided human completed the game.

Castle remains silent; Memory produces original gameplay music and sound effects.
The [#5 validation](../../docs/issue-5-validation.md) supersedes the historical
partial-delivery status of #3. The selected first-playable behavior is covered by
numerical timing/PPU/DSP and browser synchronization checks. These results do not
clear previous release ROMs or establish full hardware/catalog accuracy.
No commercial ROM is shipped or fetched.

## Independent-game selection decision for #3 / #5

Accept this pair for the original requirement of two independently developed
SNES programs. The author is shared, as are generic routines in separately pinned
versions of `1gam-common`. The game programs themselves are separate upstream
repositories: Castle's entities, collision, map/chain/bridge progression and
credits are not Memory's shuffled card grid, door transitions, selection,
matching, guesses and win/loss state machine. Neither imports the other's game
logic or assets. The manifest lists each game's own files and linked library
modules, making this distinction reproducible. They are not reskins of the same
game or variants of the diagnostic. The local alterations retain those gameplay
programs and add audited presentation/audio, without emulator-side exceptions.

#3 names candidate *independent SNES programs* and requires two distinct actual
games; it does not require different authors or unrelated libraries. #5 explicitly
says author identity alone is not a new gate. This decision therefore accepts
independent gameplay development, while making no claim of two unrelated engine
implementations. Gothicvania was replaced for the concrete runtime/audio/build
licensing blockers recorded in [PROVENANCE.md](PROVENANCE.md), not to avoid a
failing gameplay test. The original Castle image was replaced by an audited
source build to remove its unidentified credits font and release-build mismatch.
Both exact ROM hashes and all notices above remain unchanged by #5.
