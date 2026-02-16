use std::fs;
use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result};

use crate::task::{Stage, Task, TaskType};

const PROMPT_DIR: &str = "workflow/prompts";
const LOG_DIR: &str = "var/agent-logs";

const FORBIDDEN_PATTERNS: &[&str] = &[
    "qobuz.com",
    "bandcamp.com",
    "akamaized.net",
    "popplers5",
    "bcbits.com",
];

pub struct AgentResult {
    pub exit_code: i32,
    pub log_file: String,
    pub model: &'static str,
}

impl Stage {
    fn model(self) -> &'static str {
        match self {
            Stage::Reproduce
            | Stage::Design
            | Stage::Verify => "sonnet",
            Stage::Test | Stage::Fix | Stage::Impl => "opus",
        }
    }

    fn template(self, task_type: TaskType) -> &'static str {
        match (task_type, self) {
            (_, Stage::Verify) => "verify.md",
            (TaskType::Bug, Stage::Reproduce) => {
                "bug-reproduce.md"
            }
            (TaskType::Bug, Stage::Test) => "bug-test.md",
            (TaskType::Bug, Stage::Fix) => "bug-fix.md",
            (TaskType::Feature, Stage::Design) => {
                "feature-design.md"
            }
            (TaskType::Feature, Stage::Test) => {
                "feature-test.md"
            }
            (TaskType::Feature, Stage::Impl) => {
                "feature-impl.md"
            }
            (t, s) => unreachable!(
                "invalid stage {s} for task type {t:?}"
            ),
        }
    }
}

fn compose_prompt(
    task: &Task,
    stage: Stage,
) -> Result<String> {
    let preamble = fs::read_to_string(
        Path::new(PROMPT_DIR).join("preamble.md"),
    )
    .context("reading preamble.md")?;

    let template_file = stage.template(task.task_type);
    let template = fs::read_to_string(
        Path::new(PROMPT_DIR).join(template_file),
    )
    .with_context(|| format!("reading {template_file}"))?;

    let context_files = task.context_files.join(", ");
    let task_type_str = match task.task_type {
        TaskType::Bug => "bug",
        TaskType::Feature => "feature",
    };

    let body = template
        .replace("{{id}}", &task.id)
        .replace("{{title}}", &task.title)
        .replace("{{description}}", &task.description)
        .replace("{{context_files}}", &context_files)
        .replace("{{type}}", task_type_str);

    Ok(format!("{preamble}\n\n---\n\n{body}"))
}

pub fn run(task: &Task, stage: Stage) -> Result<AgentResult> {
    let prompt = compose_prompt(task, stage)?;
    let model = stage.model();
    let log_file =
        format!("{LOG_DIR}/{}-{stage}.log", task.id);
    let prompt_file =
        format!("{LOG_DIR}/{}-{stage}.prompt.md", task.id);

    fs::create_dir_all(LOG_DIR)?;
    fs::write(&prompt_file, &prompt)?;

    let allowed_tools = [
        "Read", "Grep", "Glob", "Write", "Edit",
        "Bash(cargo:*)", "Bash(jj:*)", "Bash(ls:*)",
    ]
    .join(",");

    // Allow network for Reproduce/Test stages when the
    // task opts in. All other stages stay air-gapped.
    let network = task.allow_network
        && matches!(stage, Stage::Reproduce | Stage::Test);

    let mut cmd = Command::new("claude");
    cmd.arg("-p")
        .args(["--model", model])
        .args(["--max-budget-usd", "2.00"])
        .args(["--allowedTools", &allowed_tools])
        .arg("--dangerously-skip-permissions")
        .arg(&prompt)
        .env_remove("CLAUDECODE");

    if !network {
        cmd.env("HTTP_PROXY", "http://127.0.0.1:1")
            .env("HTTPS_PROXY", "http://127.0.0.1:1")
            .env("NO_PROXY", "anthropic.com");
    }

    let output = cmd.output().context("spawning claude")?;

    let mut log =
        String::from_utf8_lossy(&output.stdout).into_owned();
    if !output.stderr.is_empty() {
        log.push_str("\n--- stderr ---\n");
        log.push_str(&String::from_utf8_lossy(
            &output.stderr,
        ));
    }
    fs::write(&log_file, &log)?;

    Ok(AgentResult {
        exit_code: output.status.code().unwrap_or(1),
        log_file,
        model,
    })
}

/// Scan files changed in the current jj change for
/// forbidden API URLs. Returns a list of violations.
pub fn safety_check() -> Result<Vec<String>> {
    let output = Command::new("jj")
        .args(["diff", "--name-only"])
        .output()
        .context("running jj diff --name-only")?;

    let changed = String::from_utf8_lossy(&output.stdout);
    let mut violations = Vec::new();

    for line in changed.lines() {
        let file = line.trim();
        if file.is_empty() {
            continue;
        }
        // Only check test files and var/ docs
        if !file.starts_with("tests/")
            && !file.starts_with("var/")
        {
            continue;
        }
        let path = Path::new(file);
        if !path.exists() {
            continue;
        }
        let content = fs::read_to_string(path)
            .with_context(|| format!("reading {file}"))?;
        for pattern in FORBIDDEN_PATTERNS {
            if content.contains(pattern) {
                violations.push(format!(
                    "{file}: contains '{pattern}'"
                ));
            }
        }
    }

    Ok(violations)
}
