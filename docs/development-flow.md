# Development Flow

This is the default contribution flow for `takahirox/snes-rust`, including AI
agents: Issue → implementation → PR → review → revision as needed → merge.
The current default branch is `main`.

## Collaboration language

Use **English** for newly authored templates, development/review/merge
documentation, AI instructions, Issue and PR titles and bodies, review comments,
and commit messages. This collaboration convention does not require translating
historical content or changing product localization.

## 1. Start with an Issue

Use the [Issue template](../.github/ISSUE_TEMPLATE/issue.md) to state the problem,
expected outcome, relevant context, reproduction steps where applicable, and
concrete completion criteria. No assignee or owner field is required.

Default to criteria an AI agent can execute and verify. Require human confirmation,
physical-device testing, subjective evaluation, or external approval only when
necessary for the actual Issue. Explain why it is necessary and what result is
expected. Separate optional validation from mandatory completion criteria.

Mandatory pre-merge acceptance must be achievable and verifiable before merge.
Checks possible only after merge must not be pre-merge approval conditions. Record
genuinely necessary post-merge work separately; do not add it routinely. For
example, verification of a merge-triggered deployment or published site belongs
after merge. This distinction preserves implementation requirements and applicable
pre-merge checks.

## 2. Implement the smallest complete change

Read the source Issue and inspect the current repository before choosing scope.
Clarify genuinely blocking ambiguity before dependent implementation. Preserve
useful existing content and justified conventions. Prefer the smallest coherent
design that fully satisfies the Issue; simplicity does not excuse missing
requirements or necessary validation.

Avoid unnecessary abstractions, speculative extensibility, unrelated refactoring,
new frameworks or subsystems, and extra policies or configuration without a
demonstrated need tied to the Issue. Added complexity is not automatically useful.

### Available validation

Runtime and browser tooling is now configured. Follow the [README validation
commands](../README.md#validate), including source-built hardware fixtures when
changing emulation behavior. Do not run incompletely audited external games in
CI or include their binaries. Report optional game checks separately.
Use the [test and CI performance principles](#test-and-ci-performance-principles)
when selecting or changing validation.

For templates and documentation, inspect Markdown structure, template syntax,
links (including anchors), whitespace, and consistency. Use available tooling
where applicable; `git diff --check` checks whitespace in tracked changes, so
include newly added files in the staged diff when validating them. Review the
diff for accidental scope changes. No runtime tests, hardware/device checks, or
deployed-site verification are required for a documentation-only change like this
collaboration setup. If tooling is added for a future Issue, use the actual
documented commands appropriate to that change.

### Test and CI performance principles

**Detect the required failures at the lightest appropriate layer, without
redundant execution.** Preserve required correctness, regressions and meaningful
integration coverage when redistributing checks. These principles guide future
test/CI work; they do not change current checks or introduce a new merge gate.

1. **Prefer the lightest meaningful layer.** Use Rust unit tests for calculations,
   branches and broad data combinations. Use native CPU-executed guests and
   source-built ROM suites for machine integration, and Wasm/Playwright checks
   for browser boundaries such as canvas output, Web Audio, controls and saves.
   Add assertions that detect concrete defects, rather than mirror implementation.
2. **Choose combinations deliberately.** Cover distinct cartridge mappings,
   PPU/DSP paths and browser states with representative cases. Exhaust combinations
   where interactions require it, rather than multiplying every ROM, mode,
   viewport and input path. Retain required regressions and integration evidence.
3. **Match validation to the change.** Documentation-only changes need Markdown,
   link/anchor, consistency and whitespace checks. UI changes need relevant
   production-build and browser checks; core/runtime changes need applicable
   Rust, source-built ROM and Wasm/browser checks from [README](../README.md#validate).
   CI/build changes need validation of the affected jobs and produced artifacts.
   Use focused checks for fast feedback and full applicable regression checks
   before merge; focused runs do not replace mandatory pre-merge validation.
   Keep optional user-supplied game investigations separate. Pages calls
   [Emulator checks](../.github/workflows/ci.yml) before deploying on `main`;
   local subpath smoke tests run before merge, while genuinely required live-site
   verification belongs in separately recorded post-merge work.
4. **Reuse work and avoid duplicate execution.** Reuse builds, verified downloads
   and fixtures with keys covering toolchain, dependencies and source inputs,
   preserving isolation and hash/provenance checks. Keep the deliberate repeated
   fresh cleared-game builds that verify reproducibility. CI currently caches npm
   downloads, and [Pages](../.github/workflows/pages.yml) reuses the checks workflow
   and validated site artifact. Non-`main` push and PR triggers can still overlap:
   future CI work should avoid equivalent runs for the same revision and cancel
   superseded checks when safe, preserving Pages deployment ordering. Measure
   resource use before adding parallelism; Playwright currently uses one worker.
5. **Review coverage and cost together.** Ask which additional defect a test
   catches, whether existing coverage already provides that confidence, and what
   execution and maintenance cost it adds. Watch repeated compiler/browser setup,
   large ROM fixtures, long emulation loops and Cartesian-product growth. Remove
   duplication without weakening meaningful assertions or required coverage.
6. **Maintain an execution-time budget.** The [Issue #136 timing record](issue-136-validation.md)
   provides initial browser measurements and runner variability; no performance
   target is defined yet. Use actual runs during test/CI performance work to
   monitor growth and investigate overruns. Track elapsed
   feedback time, total runner/computation time and queue time separately. The
   current core/browser job timeouts (20/15 minutes) are limits, not measured
   baselines or targets. Revisit validation structure and duplication as cost
   grows while retaining required confidence; this documentation change requires
   no new benchmark, hardware purchase or timing gate.

## 3. Create a Pull Request for the Issue

Use the [PR template](../.github/pull_request_template.md). Explain the problem,
resulting behavior, focused changes, related Issues, and validation actually
performed. Report results, failures, skipped checks, and unperformed checks
accurately, including reasons. Choose checks appropriate to the changed files and
actual repository tooling; unrelated runtime suites are not required for
templates or documentation alone.

Use and preserve `Closes #N` when the implementation fully addresses the Issue
and its applicable mandatory pre-merge checks are satisfied. Identify intentional
partial work and its remaining scope explicitly; link its parent Issue without
automatically closing it. Report mandatory pre-merge validation, optional checks,
and required post-merge verification separately.
Pending post-merge verification does not make an otherwise complete implementation
partial, but must remain reported as pending until performed.

## 4. Review and revise as needed

Follow the [review guidelines](review-guidelines.md). Ask: **Does the PR completely
solve the source Issue, without unnecessary scope or complexity?** Check both
missing work and unnecessary work.

Request fixes for agent-fixable findings, then review the updated PR again. When
genuinely necessary human input is missing, record the specific question and use
the workflow's wait path until an answer arrives. Distinguish that waiting state
from fixable findings instead of repeatedly requesting code fixes.

## 5. Merge

Approve and merge into `main` when the implementation and mandatory pre-merge
checks are sufficient and no blocking review findings remain. Respect the
repository's current merge settings and the workflow's authorization.
Missing implementation, mandatory pre-merge tests, required protections and
genuinely necessary pre-merge human acceptance remain blockers.

After successful merge, close the original implemented Issue, normally through
`Closes #N`. Do not remove that reference, keep the Issue open, or withhold
approval solely because post-merge-only verification is pending. ProjectWeave
publication adds the closing reference and its merge/close stages close the
Issue; asking review to undo this solely for pending post-merge checks creates
repeated Review → Fix → Publish → Review loops.

## 6. Perform post-merge verification only if required

After merging and closing the original Issue, perform any separately recorded
required post-merge checks. Record the affected revision, environment, commands
or steps, and actual results with supporting links/logs in the merged PR, closed
Issue, or a linked validation report. Report unperformed, pending or
environment-blocked checks honestly, with reasons; Issue closure does not claim
they passed. Pre-merge validation does not prove deployment or publication
succeeded. An unavailable test environment alone is not evidence of a product
defect.

If verification finds an actual defect, first check for an existing equivalent
Issue. Reuse it or create a new linked Issue with reproduction steps, affected
revision, expected and actual behavior, evidence, and links to the original
Issue and PR. Do not reopen the original Issue solely for post-merge follow-up.

Require these checks only when the actual change needs them. Documentation-only
work requires no routine manual approval, physical-device testing, heavyweight
model downloads or deployment gates, and this documentation/template change
requires no post-merge verification. Existing verification requirements remain
in force.

This lifecycle supersedes older instructions to keep an implemented Issue open
solely for post-merge verification. Preserve historical test outcomes and evidence
as recorded; this guidance does not turn pending or failed results into passes.

This flow and the templates adapt the
[GitWeave development flow](https://github.com/takahirox/gitweave/blob/main/docs/development-flow.md)
and its linked collaboration guidance to this repository.
