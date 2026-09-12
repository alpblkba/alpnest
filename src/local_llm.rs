use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::bootstrap;
use crate::paths::AlpnestPaths;

pub const DEFAULT_MODEL: &str = "qwen3:8b";

pub fn configured_model() -> String {
    std::env::var("ALPNEST_OLLAMA_MODEL")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| DEFAULT_MODEL.to_string())
}

pub fn draft_path(body_path: &Path) -> PathBuf {
    let stem = body_path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("section");

    body_path.with_file_name(format!(".{stem}.llm-draft.md"))
}

pub fn draft_command(
    body_path: &Path,
    context_path: Option<&Path>,
    panel_prompt_path: Option<&Path>,
) -> io::Result<Vec<String>> {
    let paths = AlpnestPaths::resolve()?;
    bootstrap::ensure_initialized(&paths)?;

    let script = bootstrap::runtime_script_path(&paths, "suggest_section_local.py")
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "local LLM helper is missing"))?;

    let output = draft_path(body_path);
    let mut argv = vec![
        "python3".to_string(),
        script.display().to_string(),
        "--body".to_string(),
        body_path.display().to_string(),
        "--output".to_string(),
        output.display().to_string(),
        "--model".to_string(),
        configured_model(),
    ];

    if let Some(path) = context_path.filter(|path| path.is_file()) {
        argv.push("--context".to_string());
        argv.push(path.display().to_string());
    }

    if let Some(path) = panel_prompt_path.filter(|path| path.is_file()) {
        argv.push("--panel-prompt".to_string());
        argv.push(path.display().to_string());
    }

    Ok(argv)
}

pub fn print_doctor() -> io::Result<bool> {
    let paths = AlpnestPaths::resolve()?;
    let seeded = bootstrap::ensure_initialized(&paths)?;
    let model = configured_model();

    println!("Alpnest doctor\n");
    println!("[ok] runtime home: {}", paths.home.display());
    println!(
        "[ok] content: {}{}",
        paths.contents_dir.display(),
        if seeded {
            " (starter content created)"
        } else {
            ""
        }
    );

    let helper_ready = bootstrap::runtime_script_path(&paths, "suggest_section_local.py").is_some();
    println!(
        "[{}] bundled helpers: {}",
        if helper_ready { "ok" } else { "missing" },
        bootstrap::runtime_dir(&paths).display()
    );

    let python_ready = print_command_status("python3", &["--version"]);
    let ollama_ready = print_command_status("ollama", &["--version"]);

    let model_ready = if ollama_ready {
        match Command::new("ollama").arg("list").output() {
            Ok(output) if output.status.success() => {
                let listed = model_is_listed(&String::from_utf8_lossy(&output.stdout), &model);
                println!(
                    "[{}] local model: {model}",
                    if listed { "ok" } else { "missing" }
                );

                if !listed {
                    println!("      install with: ollama pull {model}");
                }

                listed
            }
            Ok(output) => {
                println!("[missing] Ollama service is not responding");
                print_output_hint(&output);
                println!("          start it with: ollama serve");
                false
            }
            Err(error) => {
                println!("[missing] Ollama service check failed: {error}");
                false
            }
        }
    } else {
        println!("[missing] local model: {model} (Ollama is unavailable)");
        false
    };

    let ready = helper_ready && python_ready && ollama_ready && model_ready;
    println!(
        "\n{}",
        if ready {
            "Ready: local section drafts can run inside the workbench."
        } else {
            "Not ready yet: fix the missing items above, then run `alpnest doctor` again."
        }
    );

    Ok(ready)
}

fn print_command_status(command: &str, args: &[&str]) -> bool {
    match Command::new(command).args(args).output() {
        Ok(output) if output.status.success() => {
            let version = first_output_line(&output);
            println!("[ok] {command}: {version}");
            true
        }
        Ok(output) => {
            println!("[missing] {command} returned an error");
            print_output_hint(&output);
            false
        }
        Err(_) => {
            println!("[missing] {command} is not installed or not on PATH");
            false
        }
    }
}

fn first_output_line(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);

    stdout
        .lines()
        .chain(stderr.lines())
        .find(|line| !line.trim().is_empty())
        .unwrap_or("available")
        .trim()
        .to_string()
}

fn print_output_hint(output: &Output) {
    let hint = first_output_line(output);
    if hint != "available" {
        println!("          {hint}");
    }
}

fn model_is_listed(output: &str, model: &str) -> bool {
    output
        .lines()
        .filter_map(|line| line.split_whitespace().next())
        .any(|name| name == model)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_only_the_exact_ollama_model() {
        let output =
            "NAME             ID              SIZE\nqwen3:8b         abc123          5.2 GB\n";

        assert!(model_is_listed(output, "qwen3:8b"));
        assert!(!model_is_listed(output, "qwen3:4b"));
    }

    #[test]
    fn draft_is_hidden_beside_the_source() {
        let path = Path::new("/tmp/project/next-steps.md");
        assert_eq!(
            draft_path(path),
            PathBuf::from("/tmp/project/.next-steps.llm-draft.md")
        );
    }
}
