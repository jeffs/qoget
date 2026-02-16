# Stage: Implement Feature

**Task**: {{id}} — {{title}}

## Description

{{description}}

## Context Files

{{context_files}}

## Instructions

1. Read the design document at `var/tasks/{{id}}-design.md`.
2. Read the failing tests to understand the expected API and behavior.
3. Implement the feature following the design doc:
   - Modify only the files identified in the design
   - Do NOT modify any test files
   - Follow existing code style (anyhow, no pub(crate), 80-col
     comments, functional style)
   - Keep changes minimal — implement only what the tests require
4. Run `cargo test` — ALL tests (existing + new) must pass.
5. Run `cargo clippy` — no warnings allowed.
