mod agent;
mod jj;
mod task;

use std::process::Command;
use std::thread;
use std::time::Duration;

use anyhow::{Context, Result, bail};

use task::{Stage, Status, Task};

const MAX_RETRIES: u32 = 2;

fn main() -> Result<()> {
    eprintln!("Ralph Wiggum reporting for duty!");
    eprintln!();

    loop {
        let tasks = Task::load_all()?;

        if tasks.is_empty() {
            eprintln!("No tasks in var/tasks/. Exiting.");
            return Ok(());
        }

        if tasks.iter().all(|t| t.status == Status::Done) {
            eprintln!("All tasks done! Ralph helped!");
            return Ok(());
        }

        let runnable_id = tasks
            .iter()
            .filter(|t| t.is_runnable(&tasks))
            .min_by_key(|t| t.priority)
            .map(|t| t.id.clone());

        let Some(id) = runnable_id else {
            if tasks
                .iter()
                .any(|t| t.status == Status::InProgress)
            {
                eprintln!("Waiting for in-progress tasks...");
                thread::sleep(Duration::from_secs(5));
                continue;
            }
            eprintln!(
                "Deadlock: nothing runnable, nothing \
                 in-progress."
            );
            for t in &tasks {
                if t.status == Status::Failed {
                    eprintln!(
                        "  FAILED: {} — {} [{}]",
                        t.id,
                        t.title,
                        t.error.as_deref().unwrap_or("?")
                    );
                }
            }
            bail!("deadlock — all remaining tasks blocked or failed");
        };

        // Owned mutable copy from disk
        let mut task =
            Task::load(&Task::path_for_id(&id))?;

        let stage = match task.next_stage() {
            Some(s) => s,
            None => {
                task.status = Status::Done;
                task.save()?;
                continue;
            }
        };

        eprintln!(
            "=== Task {}: {} ===",
            task.id, task.title
        );
        eprintln!("    Stage: {stage}");
        if task.allow_network {
            let live = matches!(
                stage,
                Stage::Reproduce | Stage::Test
            );
            eprintln!(
                "    Network: {}",
                if live { "LIVE" } else { "blocked" }
            );
        }

        // Mark in-progress
        task.status = Status::InProgress;
        task.set_stage_status(stage, Status::InProgress);
        task.save()?;

        // Prepare jj change
        let change_id = match jj::new_change(&task, stage) {
            Ok(cid) => {
                eprintln!("    JJ change: {cid}");
                cid
            }
            Err(e) => {
                eprintln!("    FAILED jj new: {e}");
                handle_failure(
                    &mut task,
                    stage,
                    &format!("jj new: {e}"),
                )?;
                continue;
            }
        };
        let _ = change_id; // used implicitly via jj @

        // Run agent
        eprintln!("    Running agent...");
        let result = match agent::run(&task, stage) {
            Ok(r) => r,
            Err(e) => {
                eprintln!("    FAILED agent: {e}");
                handle_failure(
                    &mut task,
                    stage,
                    &format!("agent: {e}"),
                )?;
                continue;
            }
        };
        eprintln!(
            "    Agent exited: {} [{}]",
            result.exit_code, result.model,
        );
        eprintln!("    Log: {}", result.log_file);

        if result.exit_code != 0 {
            eprintln!("    FAILED: non-zero exit");
            handle_failure(
                &mut task,
                stage,
                "agent exited non-zero",
            )?;
            continue;
        }

        // Safety check
        let violations = agent::safety_check()?;
        if !violations.is_empty() {
            eprintln!("    FAILED: safety check");
            for v in &violations {
                eprintln!("      - {v}");
            }
            handle_failure(
                &mut task,
                stage,
                "safety check failed",
            )?;
            continue;
        }

        // Reload task — agent may have modified it
        // (e.g. added blockers, created subtasks)
        task = Task::load(&Task::path_for_id(&task.id))?;

        // Stage-specific verification
        if stage == Stage::Test {
            // Test stage: new test is expected to fail.
            // Don't run cargo test.
            eprintln!(
                "    Test stage: skip cargo test \
                 (expected failure)"
            );
        } else {
            eprintln!("    Running cargo test...");
            let cargo = Command::new("cargo")
                .arg("test")
                .output()
                .context("running cargo test")?;

            if !cargo.status.success() {
                eprintln!("    FAILED: cargo test");
                let stderr =
                    String::from_utf8_lossy(&cargo.stderr);
                for line in stderr.lines().take(20) {
                    eprintln!("      {line}");
                }
                handle_failure(
                    &mut task,
                    stage,
                    "cargo test failed",
                )?;
                continue;
            }
            eprintln!("    cargo test: PASS");
        }

        // Record success
        let cid = jj::current_change_id()?;
        task.set_stage_status(stage, Status::Done);
        task.set_stage_change_id(stage, cid);
        task.save()?;

        // Check if all stages done
        if task.all_stages_done() {
            eprintln!("    All stages done — squashing...");
            jj::squash_chain(&task)?;
            task.status = Status::Done;
            task.save()?;
            eprintln!("=== Task {}: DONE ===", task.id);
        }

        eprintln!();
    }
}

fn handle_failure(
    task: &mut Task,
    stage: Stage,
    reason: &str,
) -> Result<()> {
    let _ = jj::abandon(); // best-effort

    task.retries += 1;

    if task.retries > MAX_RETRIES {
        eprintln!(
            "    Max retries exceeded — marking FAILED"
        );
        task.status = Status::Failed;
        task.error = Some(reason.to_string());
        task.set_stage_status(stage, Status::Failed);
    } else {
        eprintln!(
            "    Retry {}/{}",
            task.retries, MAX_RETRIES
        );
        task.set_stage_status(stage, Status::Pending);
        task.status = Status::Pending;
    }

    task.save()
}
