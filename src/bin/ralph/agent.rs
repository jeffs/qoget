use std::fs;
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};

use crate::task::{Stage, Task, TaskType};

const PROMPT_DIR: &str = "workflow/prompts";
const LOG_DIR: &str = "var/agent-logs";

/// URL-like patterns that indicate real API endpoints.
/// Bare domain mentions (e.g. in HTML fixtures) are fine;
/// we only flag strings that look like fetchable URLs.
const FORBIDDEN_PATTERNS: &[&str] = &[
    "://qobuz.com",
    "://bandcamp.com",
    "://akamaized.net",
    "://popplers5",
    "://bcbits.com",
    ".qobuz.com/",
    ".bandcamp.com/",
    ".akamaized.net/",
    ".bcbits.com/",
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
        .args(["--max-budget-usd", "25.00"])
        .args(["--allowedTools", &allowed_tools])
        .arg("--dangerously-skip-permissions")
        .arg(&prompt)
        .env_remove("CLAUDECODE");

    if !network {
        cmd.env("HTTP_PROXY", "http://127.0.0.1:1")
            .env("HTTPS_PROXY", "http://127.0.0.1:1")
            .env("NO_PROXY", "anthropic.com");
    }

    // Stream stdout to the log file; inherit stderr so
    // the operator sees claude's progress on the terminal.
    let log_out = fs::File::create(&log_file)
        .with_context(|| format!("creating {log_file}"))?;

    let mut child = cmd
        .stdout(Stdio::from(log_out))
        .stderr(Stdio::inherit())
        .spawn()
        .context("spawning claude")?;

    // Heartbeat so the operator can distinguish "working"
    // from "stuck".
    let start = Instant::now();
    let status = loop {
        match child.try_wait().context("waiting for claude")? {
            Some(s) => break s,
            None => {
                let secs = start.elapsed().as_secs();
                eprintln!("    ... {secs}s");
                thread::sleep(Duration::from_secs(30));
            }
        }
    };

    Ok(AgentResult {
        exit_code: status.code().unwrap_or(1),
        log_file,
        model,
    })
}

/// Scan *added lines* in the current jj change for
/// forbidden API URLs. Only checks test/var files, and
/// only the lines the agent actually added.
pub fn safety_check() -> Result<Vec<String>> {
    let output = Command::new("jj")
        .args(["diff", "--git"])
        .output()
        .context("running jj diff --git")?;

    let diff = String::from_utf8_lossy(&output.stdout);
    let mut violations = Vec::new();
    let mut current_file: Option<String> = None;
    let mut in_guarded_file = false;

    for line in diff.lines() {
        if let Some(path) = line.strip_prefix("+++ b/") {
            in_guarded_file = path.starts_with("tests/")
                || path.starts_with("var/");
            current_file = if in_guarded_file {
                Some(path.to_string())
            } else {
                None
            };
            continue;
        }
        if !in_guarded_file {
            continue;
        }
        if let Some(added) = line.strip_prefix('+') {
            for pattern in FORBIDDEN_PATTERNS {
                if added.contains(pattern) {
                    let file = current_file
                        .as_deref()
                        .unwrap_or("?");
                    violations.push(format!(
                        "{file}: added line contains \
                         '{pattern}'"
                    ));
                }
            }
        }
    }

    Ok(violations)
}
