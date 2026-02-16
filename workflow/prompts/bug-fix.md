# Stage: Fix Bug

**Task**: {{id}} — {{title}}

## Description

{{description}}

## Context Files

{{context_files}}

## Instructions

1. Read the reproduction analysis at `var/tasks/{{id}}-reproduce.md`.
2. Read the failing test(s) to understand exactly what behavior is
   expected.
3. Implement the minimal fix in `src/` to make the failing test pass.
   - Change as few lines as possible
   - Do NOT modify any test files
   - Preserve existing public API signatures where possible
   - Follow the existing code style (anyhow, no pub(crate), 80-col
     comments)
4. Run `cargo test` — ALL tests (existing + new) must pass.
5. Run `cargo clippy` — no warnings allowed.
6. If the fix requires new types or functions, keep them minimal and
   well-documented.
