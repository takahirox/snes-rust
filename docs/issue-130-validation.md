# Issue #130: update the bundled 夜鳴き鐘 / Yonaki Kane ROM

## Change

`demos/yonaki.sfc` is replaced by the current build of the game, in which the
千鶴 (Chizuru) side story is expanded to about 30 minutes per route. The file name,
the `demo-yonaki` button and its label, and the loader are unchanged. Only the
SHA-256 pin changes, in `web/app.js`, the Pages smoke test, the README table and
`demos/NOTICE.md`. Shionari is untouched.

## Artifact

- `demos/yonaki.sfc`: 4,194,304 bytes, headerless LoROM, 8 KiB battery SRAM (same format as before).
- SHA-256 `bb666690e841e878f98fdd8e6562d9d10b60ea20a891dc79aa8c008c03a66688`
  (previously `e8782a44c18a3cd8487d4a42687e5aa975dbbc4b750dbef9cf240e4236bd5925`).

## Validation

- `npm --prefix web run build`: `web/dist` contains the same six files; the published `yonaki.sfc` has the new hash.
- `npx playwright test tests/pages.spec.js tests/hierarchy.spec.js tests/demo-launch.spec.js`: 41 passed.
  This includes the Yonaki title → name entry → prologue test with its own save key.
- After `python3 tools/build-test-tools.py` and `python3 tools/build-cleared-games.py`,
  `npm test -- --grep-invert '301 seconds'`: 105 passed (4.9 min).
- In the game's source repository, the Snes9x real-script test reaches all 20 endings with no text clipping,
  and the bsnes replay passes.

## Saves

Browser saves are keyed by ROM hash. Saves from the previous build stay in storage under the old key and are
not shown for the new build. This is expected and needs no migration.

## Post-merge

After the Pages deployment, `/snes-rust/yonaki.sfc` can be checked against the size and hash above.
