use std::process::Command;

use anyhow::{Context, Result, bail};

use crate::task::{Stage, Task};

/// Get the change_id of the current working copy.
pub fn current_change_id() -> Result<String> {
    let output = Command::new("jj")
        .args([
            "log", "-r", "@", "--no-graph", "-T",
            "change_id",
        ])
        .output()
        .context("running jj log")?;
    if !output.status.success() {
        bail!(
            "jj log failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .trim()
        .to_string())
}

/// Check whether a change_id still exists in the repo.
fn change_exists(change_id: &str) -> bool {
    Command::new("jj")
        .args(["log", "-r", change_id, "--no-graph", "-T", "\"\""])
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Create a new jj change for a stage, parented on the
/// previous stage's change or on main.
///
/// If a previous stage recorded a change_id that no longer
/// exists (user cleanup, abandon, squash), falls back to
/// main and clears the stale id from the task.
pub fn new_change(
    task: &mut Task,
    stage: Stage,
) -> Result<String> {
    let stages = task.task_type.stages();
    let idx = stages
        .iter()
        .position(|&s| s == stage)
        .context("stage not in task type's stage list")?;

    let parent = if idx == 0 {
        "main".to_string()
    } else {
        let prev = stages[idx - 1];
        match task
            .stages
            .get(&prev)
            .and_then(|ss| ss.change_id.as_deref())
        {
            Some(cid) if change_exists(cid) => {
                cid.to_string()
            }
            Some(_) => {
                // Stale change_id — clear it, fall back
                // to main.
                eprintln!(
                    "    warn: {prev} change_id is stale, \
                     falling back to main"
                );
                task.clear_stage_change_id(prev);
                "main".to_string()
            }
            None => "main".to_string(),
        }
    };

    let description =
        format!("task {}: {stage}", task.id);
    let status = Command::new("jj")
        .args(["new", &parent, "-m", &description])
        .status()
        .context("running jj new")?;
    if !status.success() {
        bail!("jj new failed with {status}");
    }

    current_change_id()
}

/// Abandon the current change (on failure).
pub fn abandon() -> Result<()> {
    let status = Command::new("jj")
        .args(["abandon", "@"])
        .status()
        .context("running jj abandon")?;
    if !status.success() {
        bail!("jj abandon failed with {status}");
    }
    Ok(())
}

/// Squash the full stage chain into one commit.
pub fn squash_chain(task: &Task) -> Result<()> {
    let change_ids: Vec<&str> = task
        .task_type
        .stages()
        .iter()
        .filter_map(|s| {
            task.stages
                .get(s)
                .and_then(|ss| ss.change_id.as_deref())
                .filter(|cid| !cid.is_empty())
        })
        .collect();

    if change_ids.len() < 2 {
        return Ok(());
    }

    let first = change_ids[0];
    let last = change_ids[change_ids.len() - 1];
    let msg = format!("task {}: {}", task.id, task.title);

    let status = Command::new("jj")
        .args([
            "squash", "--from", first, "--into", last,
            "-m", &msg,
        ])
        .status()
        .context("running jj squash")?;
    if !status.success() {
        bail!("jj squash failed with {status}");
    }

    Ok(())
}
