<!-- Write the title and body in English. -->

## Summary

Describe the source problem and the focused changes that solve it.

## Outcome

Describe the resulting behavior, including before/after behavior when useful.
Explain any necessary scope beyond the source Issue. If this PR is partial, state
what remains and link the parent Issue without automatically closing it. Pending
post-merge-only verification does not make a complete implementation partial.

## Validation

List checks actually performed and their results. Accurately report failures,
skipped checks, and unperformed checks with reasons. Choose checks appropriate to
the changed files and actual repository tooling; do not invent commands or require
unrelated runtime tests for documentation-only changes.

Keep mandatory pre-merge acceptance and optional validation distinct.

## Required post-merge verification

Write "None" unless checks possible only after merge are genuinely required.
Record required checks as pending until performed, including environment blockers;
they do not block pre-merge approval, and closure does not claim they passed.
After merge and Issue closure, record evidence and track actual defects in new
linked Issues under the
[post-merge guidance](https://github.com/takahirox/snes-rust/blob/main/docs/development-flow.md#6-perform-post-merge-verification-only-if-required).

## Related Issues

Link the source Issue and other relevant Issues. Use and preserve `Closes #<number>`
when the implementation fully addresses that Issue and applicable mandatory
pre-merge checks are satisfied, even if post-merge-only verification is pending.
