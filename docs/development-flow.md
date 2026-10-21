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

For templates and documentation, inspect Markdown structure, template syntax,
links (including anchors), whitespace, and consistency. Use available tooling
where applicable; `git diff --check` checks whitespace in tracked changes, so
include newly added files in the staged diff when validating them. Review the
diff for accidental scope changes. No runtime tests, hardware/device checks, or
deployed-site verification are required for a documentation-only change like this
collaboration setup. If tooling is added for a future Issue, use the actual
documented commands appropriate to that change.

## 3. Create a Pull Request for the Issue

Use the [PR template](../.github/pull_request_template.md). Explain the problem,
resulting behavior, focused changes, related Issues, and validation actually
performed. Report results, failures, skipped checks, and unperformed checks
accurately, including reasons. Choose checks appropriate to the changed files and
actual repository tooling; unrelated runtime suites are not required for
templates or documentation alone.

Only claim to close an Issue when the implementation fully addresses it. Identify
partial work explicitly and leave its Issue open. Report mandatory pre-merge
validation, optional checks, and required post-merge verification separately.
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

## 6. Perform post-merge verification only if required

Perform any separately recorded required post-merge checks, then report actual
results, failures, and needed follow-up. Pre-merge validation does not prove that
deployment or publication succeeded. No post-merge verification is required for
this templates/documentation-only collaboration setup.

This flow and the templates adapt the
[GitWeave development flow](https://github.com/takahirox/gitweave/blob/main/docs/development-flow.md)
and its linked collaboration guidance to this repository.
