# Stage: Write Failing Test

**Task**: {{id}} — {{title}}

## Description

{{description}}

## Context Files

{{context_files}}

## Instructions

1. Read the reproduction analysis at `var/tasks/{{id}}-reproduce.md`.
2. Read the existing test file(s) in `tests/` that cover the affected
   module. Understand the testing patterns used.
3. Run `cargo test` first to confirm all existing tests pass.
4. Write one or more failing test(s) that demonstrate the bug. Add
   them to the appropriate `tests/*_test.rs` file following existing
   conventions:
   - Use `#[test]` (not async)
   - Construct test data inline (no HTTP, no external resources)
   - **NEVER** use real service domains in test data — no
     `bandcamp.com`, `qobuz.com`, `akamaized.net`, `bcbits.com`,
     or `popplers5`. Use `example.com` for any URLs. This applies
     to HTML fixtures, JSON fixtures, and string literals alike.
     An automated safety check will reject your changes otherwise.
   - Use `serde_json::from_str()` for JSON fixtures
   - Use helper functions like `make_item()` if they exist
   - Name tests descriptively: `fn bug_{{id}}_short_description()`
5. Verify the new test(s) FAIL — this confirms they catch the bug.
6. Verify the existing tests still PASS — run `cargo test` excluding
   your new test name to confirm you haven't broken anything.
7. Do NOT fix the bug. The test must fail.
