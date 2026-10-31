# Issue #126: second bundled demo, 夜鳴き鐘 / Yonaki Kane

## Change

**Change cartridge** now lists two bundled demos above **Open local ROM…**:
潮鳴りの密室 (unchanged button id `demo`) and 夜鳴き鐘 (`demo-yonaki`). `web/app.js`
keeps one table of demos, keyed by button id. Each entry has a file name, display
name, exact size and SHA-256. Both buttons share the existing `loadCartridge` path:
same-origin fetch, size/hash check, save restoration, then automatic start.
The inserted demo is identified by its file name, so reselecting it is still a
no-op. Choosing the other demo switches cartridges like any other source.
Saves remain keyed by ROM hash, so the two games never share SRAM.

`tools/build-web.mjs` copies both ROMs into `web/dist`, and the Pages allowlist
gains `yonaki.sfc`. Shionari's file, hash, save key and tests are unchanged.

## Artifact

- `demos/yonaki.sfc`: 4,194,304 bytes, headerless LoROM, 8 KiB battery SRAM.
- SHA-256 `e8782a44c18a3cd8487d4a42687e5aa975dbbc4b750dbef9cf240e4236bd5925`.
- 4 MiB is the existing core and loader maximum; no core change was needed.
- Authorship, authorization and font notices are in the
  [ROM notice](../demos/NOTICE.md) and the source Issue.

## Validation

- `npm --prefix web run build`: `web/dist` contains exactly the five previous files
  plus `yonaki.sfc`.
- `npx playwright test tests/pages.spec.js tests/hierarchy.spec.js tests/demo-launch.spec.js`:
  41 passed, including the new test:
  - **Yonaki plays from the title through name entry into the prologue, with its own save.**
    It first loads Shionari and writes an SRAM marker, then switches to 夜鳴き鐘. It
    waits for a stable title, presses A to open name entry and accepts both
    default names with Start. It then checks that the story's opening chapter
    card differs from both earlier scenes. Reselecting 夜鳴き鐘 does not
    download it again. Switching back restores Shionari's marker, and exactly
    two save keys exist.
  - The picker test now expects three buttons and checks the new accessible name
    and Tab order. The Pages smoke test checks the new ROM's size and hash and
    rejects `/snes-rust/demos/yonaki.sfc`.
  - The diagnostic test expects every published file to be requested except the
    unselected `yonaki.sfc`.
- After `python3 tools/build-test-tools.py` and `python3 tools/build-cleared-games.py`,
  `npm test -- --grep-invert '301 seconds'` passed all 94 browser checks (5.6 min).
- Picker screenshots at 1280×900, 320×740 and 844×390 show three full-width buttons
  without overlap.
- Optional native check with `snes-run` and a controller replay: the title,
  name entry, bus scene (colour-math dimming and a seated silhouette) and an inn
  room scene rendered correctly. About 290 emulated frames per second on the
  development machine; audio is non-silent.

Not performed: physical SNES checks. The game's own Snes9x/bsnes playthroughs
of all 20 endings ran in its source repository, not here.

## Post-merge

None required. After the Pages deployment, `/snes-rust/yonaki.sfc` can be checked
against the size and hash above.
