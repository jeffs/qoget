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

/// Create a new jj change for a stage, parented on the
/// previous stage's change or on main.
pub fn new_change(
    task: &Task,
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
        task.stages
            .get(&prev)
            .and_then(|ss| ss.change_id.as_deref())
            .with_context(|| {
                format!(
                    "previous stage '{prev}' has no \
                     change_id"
                )
            })?
            .to_string()
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
        })
        .collect();

    if change_ids.len() < 2 {
        return Ok(());
    }

    let first = change_ids[0];
    let last = change_ids[change_ids.len() - 1];

    let status = Command::new("jj")
        .args([
            "squash", "--from", first, "--into", last,
        ])
        .status()
        .context("running jj squash")?;
    if !status.success() {
        bail!("jj squash failed with {status}");
    }

    Ok(())
}
