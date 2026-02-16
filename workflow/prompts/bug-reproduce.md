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
3. If network access is available, you may run `cargo run` with
   appropriate arguments to observe the actual API behavior. Keep
   API calls to the minimum needed — one or two targeted requests,
   not a full sync. Log or capture the response for your analysis.
4. Write a detailed reproduction analysis to
   `var/tasks/{{id}}-reproduce.md` containing:
   - The exact code path involved
   - Where the logic goes wrong and why
   - What the correct behavior should be
   - If the bug is in a pure function, a minimal demonstration
   - If you made API calls, the relevant response data observed
5. Run `cargo test` to verify the existing test suite passes
   (establishing baseline).
6. Do NOT fix the bug. Do NOT write new tests. Analysis only.
