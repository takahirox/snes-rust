# Issue #138: Yonaki Kane ROM with translucent figures and no text dim

## Change

`demos/yonaki.sfc` is replaced by the build from
[snes-roms](https://github.com/takahirox/snes-roms) commit 6da6766:

- Characters are translucent silhouettes with no outline, so the photo shows through them. Ghosts are white.
- Text no longer dims the background. The menu dim and scripted darkness are unchanged.

The SHA-256 pin is updated in `web/app.js`, the Pages smoke test, the README table and `demos/NOTICE.md`.
The file name, button, loader and Shionari are unchanged.

## Artifact

- `demos/yonaki.sfc`: 4,194,304 bytes, headerless LoROM, 8 KiB battery SRAM.
- SHA-256 `b0b90ecf9d853f8c975f9c0045712966f114b1463e973a7b91434a97a141c3c7`
  (previously `cd5b5f2560291ed8c50e9433a9eacbb33491032c09c9b7356c14ddf80cca7d47`).

## Validation

- `npm --prefix web run build`: the published `yonaki.sfc` has the new hash.
- After `python3 tools/build-test-tools.py` and `python3 tools/build-cleared-games.py`,
  `npm test -- --grep-invert '301 seconds'`: 122 passed (5.4 min). This includes the Yonaki
  title → name entry → prologue test and the Pages hash check.
- Source repository:
  - Snes9x real-script test: all 20 endings.
  - bsnes replay: passes.
  - In-game screenshots of the lobby, kitchen and dawn scenes show translucent figures on the undimmed photo.

## Post-merge

After the Pages deployment, `/snes-rust/yonaki.sfc` can be checked against the size and hash above.
