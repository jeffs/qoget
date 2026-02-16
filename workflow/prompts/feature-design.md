# Stage: Design Feature

**Task**: {{id}} — {{title}}

## Description

{{description}}

## Context Files

{{context_files}}

## Instructions

1. Read the feature description and all context files carefully.
2. Read the relevant source files to understand the current
   architecture and where the feature fits.
3. Write a design document to `var/tasks/{{id}}-design.md` containing:
   - Files to modify and why
   - New types, functions, or modules needed
   - How the feature integrates with existing code
   - Edge cases and error handling strategy
   - Test cases to write (with expected inputs/outputs)
4. Run `cargo test` to verify the existing test suite passes
   (establishing baseline).
5. Do NOT implement anything. Design only.
