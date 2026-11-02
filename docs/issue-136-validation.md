# Issue 136 Pages timeout recovery

The browser job now allows 15 minutes, including setup, and uses Playwright's
list reporter so future logs identify each completed test and its duration.
The production build, two fresh audited game builds, full applicable browser
suite, single worker, individual test timeouts and required core/browser gates
are unchanged. Pages still uploads only the validated `web/dist` artifact on a
main push and deploys only after both checks succeed. No deployment bypass or
test retry was added.

## Diagnosis from actual GitHub logs

Compared the successful [PR run 41415029694](https://github.com/takahirox/snes-rust/actions/runs/41415029694/job/125176681256)
with the cancelled [main Pages run 41416164519](https://github.com/takahirox/snes-rust/actions/runs/41416164519/job/125180416896).
The reviewed head `54b031be3a7dcf2317c61ba44d14ecd462666ec1` and merge
`b22d34d8126bfc6aee12c91c0ca9cc2bce3b32a8` have identical `.github`, `web`,
`tools`, `crates` and Cargo sources (`git diff` between these revisions is empty
for those paths). Both runs selected 122 tests with one worker.

These elapsed times come from timestamped step starts to the next step start;
Playwright ends at its passing summary or cancellation. They include small
between-step overheads, rather than claiming unavailable profiler measurements.

| Phase | Successful PR | Cancelled Pages |
| --- | ---: | ---: |
| Checkout | 1.24 s | 1.48 s |
| Rust toolchain setup | 8.98 s | 9.90 s |
| Node setup | 5.73 s | 4.47 s |
| Python setup | 0.33 s | 0.23 s |
| Pinned tools and both audited game builds | 38.65 s | 47.96 s |
| npm ci | 1.05 s | 1.69 s |
| Production Wasm/site build | 0.88 s | 1.20 s |
| Chromium and OS dependencies | 27.97 s | 36.21 s |
| Playwright step | 367.68 s, passed | 507.85 s, cancelled |

Playwright's test-start banner appeared at 08:44:05.781 UTC on the PR runner
and 08:53:52.215 UTC on Pages. The first line of 80 success dots appeared at
08:49:35.936 and 09:01:51.053 respectively: 330.16 versus 478.84 seconds for
the same progress, about 45% slower. The PR then completed the remaining 42
tests in 35.02 seconds, reporting **122 passed (6.1m)**. Pages continued serving
assets until 09:02:15.535, then reported `The operation was canceled.` at
09:02:17.136. No failed-test marker, assertion failure or final suite total
was reported on Pages. The core job passed; artifact upload and deploy were
skipped.

The PR used an AWS us-west-2 runner with image `20261018.50.1`; Pages used
Azure westus with image `20261025.179.1`. Tool builds, Wasm compilation and
browser installation were also slower. Together with identical tested sources,
this supports runner variability and inadequate job headroom, rather than a
demonstrated repeatable application regression. The old dot reporter does not
provide individual test durations, so attributing the extra time to a particular
test, host resource or browser defect would be speculation.

Scaling the passing suite by the observed 80-test slowdown gives about 533
seconds of Playwright time; adding the slower setup (about 105 seconds) already
exceeds ten minutes. Fifteen minutes leaves roughly four minutes for further
variation, upload and cleanup while keeping a finite job limit. This is a
budget choice, not a measured guarantee for every hosted runner. Per-test list
output makes subsequent slowdowns diagnosable without adding coverage or cost.

## Local validation on 2026-11-02

Environment: macOS ARM64, Node 26.12.1, Python 3.13.13, native Rust 1.94.0 and
Wasm Rust 1.99.0 (selected with `RUSTC="$(rustup which rustc)"`). Local timings
do not establish Linux hosted-runner performance or prove that the updated
remote workflow passed.

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --workspace --all-targets --locked -- -D warnings`: passed.
- `cargo test --workspace --locked`: 519 passed, zero failures.
- `python3 tools/make-fixture.py .cache/smoke.bin` and `cmp` against the tracked
  fixture: identical.
- `npm --prefix web ci`, `python3 tools/build-test-tools.py`, and two consecutive
  `python3 tools/build-cleared-games.py` builds: passed. Both builds retained
  Castle SHA-256 `197a176355e42f7984ab090baf7b2ac895c59409a9ffeec6c6017d7c5c28c869`
  and Memory SHA-256 `6b57b34416676d2dfddfef6bb450ffacc7336900adb916648bc1dea0317494b1`.
- `RUSTC="$(rustup which rustc)" npm --prefix web run build`: passed; the
  production asset set is unchanged.
- `SNES_TEST_PORT=18086 PLAYWRIGHT_JSON_OUTPUT_FILE=../.cache/issue-136/browser.json
  npm --prefix web test -- --grep-invert '301 seconds' --reporter=list,json`:
  **122 passed in 302.27 seconds**, zero failures, retries, flakes or skips.
  The additional JSON reporter records timing evidence; CI uses the same list
  reporter and test selection. The suite includes all 17 volume checks,
  cleared-game playthroughs, real Web Audio queues/drift/attenuation, both demos,
  SRAM protection, input/layout/safe areas/fullscreen, rendering, and production
  asset allowlist/hashes, CSP, MIME and `/snes-rust/` checks.
- YAML parsing and a structural comparison against the original workflow:
  passed; only the browser job limit and reporter differ. Documentation links
  and staged whitespace checks passed. `actionlint` is not installed locally.

Individual timings from this run's Playwright JSON results:

| Test | Duration |
| --- | ---: |
| Memory five-minute controller playthrough | 67.589 s |
| Castle five-minute gameplay replay | 58.912 s |
| Castle all maps and credits | 35.242 s |
| Yonaki title/name/prologue and save | 16.182 s |
| Memory real Web Audio queues/drift/stalls | 12.898 s |
| Shionari opening/title/warehouse | 12.223 s |

All cleared-game tests together took 192.911 seconds; Pages tests took 41.022
seconds; the 17 volume tests took 12.713 seconds. These are sums of test
durations, not whole-job times. The full 302.27-second local result is close
to the 301.5 seconds recorded for Issue #134. No persistent slowdown reproduced
locally, and the measured long tests are retained. Initial native workspace
validation and a brief public-site inspection overlapped this run, so this is
an observed development run rather than an isolated performance benchmark.

The unchanged native CPU/SPC700 source-ROM and scanline HDMA gates were not
rerun locally for this workflow-only fix; both referenced GitHub core jobs
passed them. They remain mandatory in CI. The updated Linux Actions job has
not been run by this node. Optional user-supplied game replays remain excluded
by the existing `--grep-invert '301 seconds'` selection.

## Required post-merge verification

At 09:12 UTC on 2026-11-02, fresh Chromium contexts at all three required
viewport sizes reached Ready on the public URL with one `#sound` checkbox,
zero `#mute` buttons and zero `#volume` sliders. No console/page errors,
failed requests or HTTP errors occurred on initial load. Public HTML, JS and
CSS returned HTTP 200 but each differed byte-for-byte from the reviewed sources.
This confirms the old app is still published; these checks do not validate the
new audio controls.

Pending. This implementation node is authorized to commit locally, with no
push, PR, merge or GitHub comment. It cannot produce a main deployment of this
workflow change before a later authorized merge. No successful deployment of
the new controls, live mute/attenuation result or deployed revision match is
claimed here. Checks of the old published Sound checkbox do not satisfy these
requirements.

After merge:

1. Record the successful main Pages run URL, deployed commit and passing
   `checks / core`, `checks / browser`, artifact upload and `deploy`. Inspect
   the browser total and individual timings; do not bypass a failed gate.
2. In fresh Chromium contexts at desktop 1280×900, portrait 320×740 and
   landscape 667×320, verify Ready and Playing, always-visible `#mute` and
   `#volume`, mute/restore, live attenuation, and no console, page or HTTP errors.
   Save screenshots of the actual public app at each size.
3. Measure real Web Audio output at 100%, 50%, 25%, zero and restored volume
   using matched analyser taps before/after the master gain, as in
   `web/tests/volume.spec.js`. Confirm nonzero input, output/input ratios near
   1/.5/.25, silence at zero, continuing frames, and attenuation of already
   queued sound while the main thread is blocked. Record measurements rather
   than relying on slider values or a listening claim.
4. Compare downloaded public HTML/JS/CSS and both ROMs with the intended
   revision, and Wasm with that run's validated Pages artifact (not a build
   from a different compiler). Record SHA-256 hashes and HTTP results. Confirm
   the app is the new implementation before treating any live check as passed.

Physical iOS Safari, Android Chrome and headphone listening remain optional.
ROM licensing, source provenance, production asset allowlist and all ROM hashes
remain unchanged; no external game is added or executed.
