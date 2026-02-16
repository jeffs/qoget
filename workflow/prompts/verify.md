# Stage: Verify

**Task**: {{id}} — {{title}} ({{type}})

## Instructions

Run all verification checks and write a summary.

1. Run `cargo test` — all tests must pass.
2. Run `cargo clippy` — no warnings.
3. Run `cargo build --release` — must compile cleanly.
4. Review the diff (`jj diff`) for:
   - Correctness: does the change address the task?
   - Minimality: are there unnecessary changes?
   - Style: 80-col comments, no `pub(crate)`, functional style?
   - Safety: no real API URLs in test files
     (no `qobuz.com`, `bandcamp.com`, `akamaized.net`)
5. Write a verification summary to `var/tasks/{{id}}-verify.md`:
   - Test results (pass count)
   - Clippy results
   - Release build result
   - Diff review findings
   - Overall verdict: PASS or FAIL with reasons
