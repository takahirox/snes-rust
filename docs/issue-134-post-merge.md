# Issue 134 merge and publication result

## Merge

PR [#135](https://github.com/takahirox/snes-rust/pull/135) merged on
2026-11-02 as `b22d34d8126bfc6aee12c91c0ca9cc2bce3b32a8`.
Issue [#134](https://github.com/takahirox/snes-rust/issues/134) was freshly read
and confirmed open immediately before the merge attempt. GitHub accepted the
merge with method `merge` and the expected head
`54b031be3a7dcf2317c61ba44d14ecd462666ec1`. The merge's parents are the previous
main `244f72f3c3a0aa47d92eaa4fedf7087fac922e7b` and that reviewed PR head.
The input PR reference was preserved unchanged.

The upstream review approved the change without blocking findings. The PR's
[Emulator checks run](https://github.com/takahirox/snes-rust/actions/runs/41415029694)
had successful core and browser jobs before merge. A duplicate branch-push browser
job was still running at merge time. GitHub reported no main branch protection
or applicable branch rules; no checks or reviews were bypassed.

## Deployment failed

The merge-triggered [Pages run](https://github.com/takahirox/snes-rust/actions/runs/41416164519)
concluded `cancelled`:

| Job | Result |
| --- | --- |
| checks / core | Success |
| checks / browser | Cancelled after exceeding the existing 10-minute job limit |
| deploy | Skipped |

The [browser job](https://github.com/takahirox/snes-rust/actions/runs/41416164519/job/125180416896)
annotation states `The job has exceeded the maximum execution time of 10m0s`.
The final error at 09:02:17 UTC was `The operation was canceled.` The production
Playwright step was active at cancellation. No final browser pass count or
assertion-failure summary was reported, so this run is not counted as passing.
[Recorded workflow metadata](issue-134/live/deployment.json) and
[failure annotations](issue-134/live/failure-annotations.json) preserve the job
results and timeout evidence.

## Public-site inspection

Real Playwright/Chromium inspection of
[the public app](https://takahirox.github.io/snes-rust/) after the failed run found
the previous Sound checkbox and no mute button or volume slider. All three fresh
browser contexts reached Ready without console/page errors or HTTP failures.
Published HTML, JavaScript and CSS returned HTTP 200 but did not match the reviewed
source files. [The live evidence](issue-134/live/deployment-blocked.json) records
timestamps, asset hashes/MIME types, control counts and diagnostics.

| Viewport | Actual public-site screenshot |
| --- | --- |
| Desktop 1280×900 | [Screenshot](issue-134/live/1280-deployment-blocked.png) |
| Portrait 320×740 | [Screenshot](issue-134/live/320-deployment-blocked.png) |
| Short landscape 667×320 | [Screenshot](issue-134/live/667-deployment-blocked.png) |

These checks establish that the new app was not published. They do not establish
working live mute, restoration, smooth gain changes or output attenuation for
the merged revision. Those checks remain pending until successful deployment;
the pre-merge audio measurements do not replace them.

## Follow-up

Opened task [#136](https://github.com/takahirox/snes-rust/issues/136) with the
failed run, logs, actual/expected behavior and required deployment/live checks,
following the repository's post-merge failure flow. Issue #134 remains closed by
its implementation merge. This node did not modify the workflow, rerun jobs,
change the public app or start background monitoring.

The merge succeeded, so the GitWeave result is `merged=true`, `retry=false` with
the actual merge SHA above. Publication failure is tracked separately in #136.
