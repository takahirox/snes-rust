# Issue #9 Pages deployment verification

On 2026-10-22, [Issue #9](https://github.com/takahirox/snes-rust/issues/9) was
resolved operationally by enabling the repository's GitHub Actions Pages source.
No application or workflow change was needed. This record and the
[machine-readable evidence](../tests/results/issue-9.json) preserve the actual
production results.

## Configuration and deployment

Before remediation, an authenticated administrator's `GET /repos/takahirox/snes-rust/pages`
returned HTTP 404, while the repository API reported `has_pages: false`. The
`github-pages` environment initially had no protection rules or branch policy.
This independently confirmed the missing setting reported by the original
failed deployment, rather than assuming that every Pages 404 proves disablement.

The authorized remediation used the existing authenticated GitHub CLI:

```sh
gh api --method POST repos/takahirox/snes-rust/pages -f build_type=workflow
gh api repos/takahirox/snes-rust/pages
gh api repos/takahirox/snes-rust/environments/github-pages
gh api repos/takahirox/snes-rust/environments/github-pages/deployment-branch-policies
```

The response and subsequent reads report `build_type: workflow`,
`has_pages: true`, `https_enforced: true`, `public: true`, no custom domain, and
`html_url: https://takahirox.github.io/snes-rust/`. GitHub added an environment
branch policy as part of enablement; the final policy permits `main`.
The [GitHub Pages REST documentation](https://docs.github.com/en/rest/pages/pages#create-a-github-pages-site)
defines the site-creation endpoint and `workflow` build type. No workflow token
permissions or build gates were changed.

Only the failed jobs of existing run `39764299327` were requested to rerun through
the GitHub connector. The existing `main` commit was still
`64a53034c512d72d09003adb68d0437a5d3d1a01`; no source push was used to deploy.

- [Attempt 2](https://github.com/takahirox/snes-rust/actions/runs/39764299327/attempts/2)
  completed successfully at 12:33:00 UTC. The core and browser check results are
  successful, including artifact upload; the failed-job retry reused the
  previously validated artifact.
- [Deploy job](https://github.com/takahirox/snes-rust/actions/runs/39764299327/job/119825596323)
  reports success for both `actions/configure-pages@v5` and
  `actions/deploy-pages@v4`, and reports the public URL.
- Deployment `7252254021` has a successful status and environment URL
  <https://takahirox.github.io/snes-rust/>.

## Artifact and live HTTPS verification

Downloaded `github-pages` artifact `12419797684` from that run and inspected every
tar member, including hidden files. It contains exactly four nonempty regular
files, with no symlinks or extra directories other than the archive root. There
are no ROMs, SRAM saves, build caches, test output or generated game artifacts.
Each live HTTPS response was HTTP 200 without redirects, and its bytes and
SHA-256 matched the downloaded artifact. Exact hashes are in the JSON evidence.

| Path under `/snes-rust/` | Bytes | Content-Type |
| --- | ---: | --- |
| `/` (`index.html`) | 1832 | `text/html; charset=utf-8` |
| `app.js` | 8886 | `application/javascript; charset=utf-8` |
| `style.css` | 1278 | `text/css; charset=utf-8` |
| `snes_wasm.wasm` | 95213 | `application/wasm` |

At 12:33:24 UTC, Playwright 1.61.1 / Chromium 149.0.7827.55 opened the real HTTPS
site in a fresh browser context. Observations and assertions passed:

- Real Wasm instantiation completed; status became **Open a local SNES ROM to begin**.
- The local ROM chooser was enabled and the canvas used pixelated rendering.
- The original MIT [diagnostic](../tests/roms/PROVENANCE.md) was selected from
  `tests/roms/smoke.bin` using the normal file input. Play became enabled, the
  emulated controller register at `0x11` responded to ArrowRight, the framebuffer
  contained nonblank pixels, and Pause succeeded with no app error.
- No page exceptions, error-level console messages, CSP violations or failed
  requests occurred. Network capture contained exactly four same-origin GETs
  for the app assets, each returning 200, including throughout local ROM play.
  No ROM was uploaded or fetched remotely.

Before deployment completed, an early HTTPS probe returned 404. All final checks
above ran after the successful deployment; the intermediate response is not
counted as verification success.

The existing local Pages smoke test also passed against those exact downloaded
production assets placed in ignored `web/dist`, rather than a different rebuild:

```sh
npm --prefix web ci
cd web && npx playwright install chromium
# From the repository root, after placing the inspected artifact in web/dist:
SNES_TEST_PORT=18085 npm --prefix web test -- tests/pages.spec.js
```

Result: **1 passed**. The successful Actions core/browser gates provide workspace
validation for the unchanged deployed commit; unrelated native suites were not
repeated locally for this documentation-only checkpoint. Markdown links, JSON
syntax and staged whitespace were checked separately.

## Reporting and remaining scope

The configuration fix and production smoke check are complete. This execution
was explicitly prohibited from commenting on Issues, pushing, opening PRs or
merging. Evidence is committed here for a later authorized Issue-reporting step;
Issue #9 was not edited or closed by this execution.

No corrective code PR or additional merge-triggered smoke check is needed for
this setting-only fix. A later `main` push has not been independently exercised;
the unchanged push trigger remains configured. A second browser and physical
gamepad were optional and were not tested. No commercial or third-party game
ROM was acquired or published.
