# Review and Merge Guidelines

Use these steps for human and AI reviews. Follow the
[development flow](development-flow.md), including its English collaboration
convention. Review checks correctness and whether the change is the right response
to the source Issue.

## Review against the Issue

Read the source Issue, its acceptance criteria, the PR description, and the full
diff. The central question is:

> **Does the PR completely solve the source Issue, without unnecessary scope or complexity?**

## Check for missing work

Verify that all required behavior and acceptance criteria the PR claims to resolve
are implemented and appropriately validated. Check documentation when behavior
changes. Do not approve a PR as closing an Issue while required work is missing.
If a PR is intentionally partial, ensure it says what remains and leaves the
Issue open.

## Check for unnecessary work

Reject unnecessary abstractions, speculative extensibility, unrelated refactoring,
new frameworks or subsystems, and extra policies or configuration without a
demonstrated need tied to the Issue. Require a concrete justification for scope
beyond the Issue.

Prefer the smallest coherent design that fully satisfies the Issue. Added
complexity is not automatically beneficial. Preserve justified existing
conventions, and do not use simplicity to excuse omitted requirements or
necessary validation.

## Check the result and validation

Verify that the result matches the expected outcome and fits the actual repository
structure and conventions. Inspect the evidence for checks actually performed;
validation claims must accurately distinguish passes, failures, skipped checks,
and unperformed checks, with reasons.

Review applicable interface changes against the [UI/UX principles](../README.md#uiux).

Apply the [test and CI performance principles](development-flow.md#test-and-ci-performance-principles):
review additional defect coverage, overlap with existing checks, and execution
and maintenance cost together. Ask:

> Does this change provide sufficient confidence with the lightest appropriate
> validation, without redundant execution?

Choose checks appropriate to the changed files and available tooling. Runtime and browser checks are documented in the [README](../README.md#validate). For documentation and
templates, check syntax, links, whitespace, and consistency as described in
[available validation](development-flow.md#available-validation). Do not demand
invented commands, unrelated runtime suites, new tooling, hardware/device tests,
or deployment checks for this documentation-only collaboration setup.

## Check pre-merge acceptance and post-merge verification

Require all mandatory pre-merge acceptance criteria to be achievable and verifiable
before merge. Check that necessary human-only criteria explain why they are needed
and what result is expected. Keep optional validation separate from mandatory
acceptance.

Checks possible only after merge, such as merge-triggered deployment or
published-site verification, must not be prerequisites for pre-merge approval.
Record genuinely necessary post-merge checks separately and report them as pending
until performed. Pre-merge validation does not prove deployment or publication
succeeded. This distinction does not waive implementation requirements or
applicable pre-merge checks. Do not add post-merge work routinely; none is required
for this templates/documentation-only collaboration setup.

## Review outcome and merge

- **Agent-fixable findings:** request specific fixes for missing requirements,
  unjustified scope or complexity, correctness problems, or insufficient
  validation. Tie each blocking finding to the Issue or a concrete defect. Review
  the updated PR again after revision.
- **Waiting for necessary human input:** record the specific unanswered question,
  why the answer is necessary, and the expected confirmation or decision. Use the
  workflow's wait path and wait for the answer; resume review when it arrives.
  Clearly distinguish this state from agent-fixable findings. Do not repeatedly
  request code fixes for missing human input.
- **Ready to merge:** approve and merge when the implementation fully addresses
  the Issue it claims to resolve, mandatory pre-merge checks are sufficient and
  pass, and no blocking findings or necessary unanswered human questions remain.
  Follow the repository's merge settings and the workflow's authorization.

Pending checks possible only after merge do not block approval of an otherwise
ready PR. After merge, perform any recorded required verification and report actual
results, including failures and follow-up; do not claim completion before it runs.

Adapted from the
[GitWeave review guidelines](https://github.com/takahirox/gitweave/blob/main/docs/review-guidelines.md).
