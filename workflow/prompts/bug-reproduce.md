# Stage: Reproduce Bug

**Task**: {{id}} — {{title}}

## Description

{{description}}

## Context Files

{{context_files}}

## Instructions

1. Read the bug description and all context files carefully.
2. Read the relevant source files to trace the code path where the
   bug occurs. Follow the call chain from entry point to the failure.
3. Write a detailed reproduction analysis to
   `var/tasks/{{id}}-reproduce.md` containing:
   - The exact code path involved
   - Where the logic goes wrong and why
   - What the correct behavior should be
   - If the bug is in a pure function, a minimal demonstration
4. Run `cargo test` to verify the existing test suite passes
   (establishing baseline).
5. Do NOT fix the bug. Do NOT write new tests. Analysis only.
