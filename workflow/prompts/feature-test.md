# Stage: Write Feature Tests

**Task**: {{id}} — {{title}}

## Description

{{description}}

## Context Files

{{context_files}}

## Instructions

1. Read the design document at `var/tasks/{{id}}-design.md`.
2. Read existing test files in `tests/` to understand patterns.
3. Run `cargo test` first to confirm all existing tests pass.
4. Write tests for the feature in the appropriate `tests/*_test.rs`
   file(s). Follow existing conventions:
   - Use `#[test]` (not async)
   - Construct test data inline (no HTTP, no external resources)
   - Use `serde_json::from_str()` for JSON fixtures
   - Use helper functions where they exist
   - Cover the main path and key edge cases from the design doc
5. The new tests will fail (feature not yet implemented). Confirm
   they fail for the right reason (missing function/type, not panic).
6. Verify existing tests still PASS.
7. Do NOT implement the feature. Tests only.
