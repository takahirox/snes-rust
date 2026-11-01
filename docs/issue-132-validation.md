# Issue #132: Yonaki Kane ROM with silhouette fixes

## Change

`demos/yonaki.sfc` is replaced by a build that corrects six silhouettes:

- Two malformed staged figures were regenerated.
- Two new poses match their text: a standing proprietress, and a tired woman leaning at a window.
- Two script lines now use a matching existing pose.

The SHA-256 pin is updated in `web/app.js`, the Pages smoke test, the README table and `demos/NOTICE.md`.
The file name, button, loader and Shionari are unchanged.

## Artifact

- `demos/yonaki.sfc`: 4,194,304 bytes, headerless LoROM, 8 KiB battery SRAM.
- SHA-256 `cd5b5f2560291ed8c50e9433a9eacbb33491032c09c9b7356c14ddf80cca7d47`
  (previously `bb666690e841e878f98fdd8e6562d9d10b60ea20a891dc79aa8c008c03a66688`).

## Validation

- `npm --prefix web run build`: the published `yonaki.sfc` has the new hash.
- After `python3 tools/build-test-tools.py` and `python3 tools/build-cleared-games.py`,
  `npm test -- --grep-invert '301 seconds'`: 105 passed (6.6 min). This includes the Yonaki
  title → name entry → prologue test and the Pages hash check.
- Source repository:
  - Build: 0 warnings.
  - Snes9x real-script test: all 20 endings.
  - bsnes replay: passes.
  - In-game screenshots of the two changed bad-ending scenes show the new poses.

## Post-merge

After the Pages deployment, `/snes-rust/yonaki.sfc` can be checked against the size and hash above.
