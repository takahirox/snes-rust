---
name: Issue
about: Report a problem or propose a change
title: ""
labels: ""
---

<!-- Write the title and body in English. Remove prompts that do not apply. -->

## Problem

Describe the problem and why it matters.

## Expected outcome

Describe what should be true when the Issue is resolved.

## Context and reproduction

Add relevant context, examples, logs, and related Issues. For a reproducible
problem, include steps, expected results, and actual results where applicable.

## Pre-merge acceptance

List concrete mandatory criteria that can be achieved and verified before merge.
Default to criteria an AI agent can execute and verify. Require human confirmation,
physical-device testing, subjective evaluation, or external approval only when
necessary for this Issue; explain why and what result is expected. No assignee or
owner field is required.

- [ ] Describe a verifiable completion criterion.

Checks possible only after merge must not be prerequisites for PR approval. Keep
all implementation requirements and applicable pre-merge checks. See the
[development guidance](https://github.com/takahirox/snes-rust/blob/main/docs/development-flow.md#1-start-with-an-issue).

## Optional validation

List useful additional checks separately from mandatory acceptance, or write
"None." These checks do not block completion.

## Required post-merge verification

Record checks possible only after merge only when this Issue needs them; otherwise
write "None." For example, a merge-triggered deployment can only be verified after
merge. Report required checks as pending until performed; pre-merge validation
does not prove publication succeeded. See the
[review guidance](https://github.com/takahirox/snes-rust/blob/main/docs/review-guidelines.md#check-pre-merge-acceptance-and-post-merge-verification).
