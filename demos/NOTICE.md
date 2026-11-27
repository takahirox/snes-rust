# Shionari no Misshitsu (潮鳴りの密室)

Created by takahirox with AI assistance, 2026.

Copyright © 2026 takahirox, to the extent applicable law recognizes copyright
or other protectable rights. **All rights reserved.**

The maintainer authorizes inclusion and distribution of the unmodified
`demos/shionari.sfc` ROM in the `takahirox/snes-rust` GitHub repository and through
its official, publicly accessible GitHub Pages emulator demo. Visitors may access
and play the bundled demo. No broader license to separately redistribute, modify,
sublicense, or commercially exploit the game/embedded assets is granted by this
notice. This does not limit rights provided by applicable law or GitHub's
applicable terms.

The repository's MIT license for emulator source code **does not apply to this
game ROM or its embedded content**. This notice retains whatever rights legally
exist; it does not assert that every AI-generated element qualifies for copyright
protection in every jurisdiction.

## Provenance and exact artifact

- File: `demos/shionari.sfc` (published unchanged as `shionari.sfc`).
- Size: 524,288 bytes; headerless ordinary LoROM, 8 KiB battery-backed SRAM.
- SHA-256: `1132c59cfba4cc7b0c1a7d7957b4b5de84c6038e0e061ca2e6372f8b8b4a01ca`.
- Added in [PR #109](https://github.com/takahirox/snes-rust/pull/109).
- Authorship and distribution authorization: the maintainer's
  [Issue #110 clarification](https://github.com/takahirox/snes-rust/issues/110#issuecomment-6502461333).
  The code, story, graphics, audio and other embedded assets were created for this
  game with AI assistance under the maintainer's direction. The maintainer reports
  no knowingly incorporated third-party licensed assets and no known required
  third-party asset notices. Any later identified third-party component requires
  explicit license/attribution handling before redistribution.

# Yonaki Kane (夜鳴き鐘)

Created by takahirox with AI assistance, 2026.

Copyright © 2026 takahirox, to the extent applicable law recognizes copyright
or other protectable rights. **All rights reserved.**

The same authorization and limits as above apply to the unmodified
`demos/yonaki.sfc` ROM: the maintainer authorizes its inclusion and distribution
in the `takahirox/snes-rust` GitHub repository and through its official, publicly
accessible GitHub Pages emulator demo, and visitors may access and play it. No
broader license is granted. The repository's MIT license for emulator source code
**does not apply to this game ROM or its embedded content**.

## Provenance and exact artifact

- File: `demos/yonaki.sfc` (published unchanged as `yonaki.sfc`).
- Size: 3,145,728 bytes; headerless ordinary LoROM, 8 KiB battery-backed SRAM.
- SHA-256: `5bf8d0e2d46c7421cecc85bb825b1ce1e02ae329f118ba48c938b9747a51da25`.
- Added for [Issue #126](https://github.com/takahirox/snes-rust/issues/126) and updated to the current build for
  [Issue #130](https://github.com/takahirox/snes-rust/issues/130), [Issue #132](https://github.com/takahirox/snes-rust/issues/132) and
  [Issue #138](https://github.com/takahirox/snes-rust/issues/138). Their bodies record
  the maintainer's authorship and distribution authorization.
- Updated for [Issue #149](https://github.com/takahirox/snes-rust/issues/149): the maintainer's
  validated snes-roms working-tree build (base commit `6da6766`) sets `DIM_TEXT` to 2 out of 31
  for subtly dimmed backgrounds while reading. White text is unchanged.
- Updated for [Issue #152](https://github.com/takahirox/snes-rust/issues/152): the same working tree
  removes full-background dimming during figure changes, prepares the new composition while the old
  VRAM image stays visible, then transfers the affected columns during V-blank.
- Updated for [Issue #155](https://github.com/takahirox/snes-rust/issues/155): the same working tree
  adds eight gradual figure-only dissolve stages, with at least 32 animation frames. It compares
  the settled/target pictures and transfers only changed tiles while retaining the photo palette,
  reading dim and text. Larger multi-figure updates take longer. See the
  [validation and captures](../docs/issue-155-validation.md). Public SFC Kamaitachi no Yoru gameplay
  informed the pacing; no artwork, ROM data or code from that game is included.
- Updated for [Issue #168](https://github.com/takahirox/snes-rust/issues/168) from the same
  maintainer working tree. Ordinary scenes keep their brightness when narration begins.
  Holding R applies the previous reading dim and holding L hides the text layer, including
  when both buttons are held. Chapter names label bookmarks and the story flows directly
  into each scene.
- Updated for [Issue #172](https://github.com/takahirox/snes-rust/issues/172): the
  maintainer's validated snes-roms working-tree build (base commit `6da6766`) adds
  a one-pixel dark outline to narration and choices using the same Shinonome glyphs.
- Updated for [Issue #178](https://github.com/takahirox/snes-rust/issues/178) from the
  committed [snes-roms release](https://github.com/takahirox/snes-roms/commit/10fb57baa5ff425c1b11f2cd58ff82fbae095ea3).
  Its `dist/yonaki.sfc` combines the 3 MiB packed assets, frame scheduler, and save recovery with
  the reading controls, outlined text, continuous chapter transitions, and silhouette dissolves.
- Updated from the maintainer's validated snes-roms commit `1334304`:
  the culprit is typed on the name entry grid, most choices continue the sentence above them, the
  photo dims under text by its own brightness, a guest register precedes a new game, and the
  scenario's clues and pacing are revised. Saves from earlier builds start fresh.
- Updated from the maintainer's validated snes-roms commit `abd348e`: the night photographs and
  figures from the first bell to the end of the main story are shown (an unlifted darkness had
  hidden them), story lines are 24 px apart with seven lines a page, reveals open a new page, and
  close-ups inside a scene change the picture under the text. Saves from earlier builds start fresh.
- The code, story, music, sound effects and SPC700 driver were created for this
  game with AI assistance under the maintainer's direction; instruments are
  synthesised, not sampled. Background photographs and character silhouettes were
  generated for it with Codex CLI's built-in image generation.
- Known third-party font notices: the title lettering images were rendered from
  Noto Serif CJK JP (Google/Adobe, SIL Open Font License 1.1); body text glyphs come
  from the public-domain Shinonome 12-dot bitmap font. No font software is
  embedded. The maintainer reports no other knowingly incorporated third-party
  licensed assets. Any later identified third-party component requires explicit
  license/attribution handling before redistribution.

## Other ROMs

These are the site's only bundled games. Castle Platformer, Memory Game and the
original diagnostics are test-only fixtures governed by their existing
[cleared-game audit](../tests/roms/CLEARED-GAMES.md) and test notices. They were
not shipped by the previous browser app and remain outside the production
artifact. User-supplied local ROMs have their own rights and are never uploaded;
battery SRAM stays in the visitor's browser storage.
