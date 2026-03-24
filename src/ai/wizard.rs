//! Interactive AI setup wizard
//!
//! Runs on first `?` usage (when no config exists) or when the user invokes
//! `rush --setup-ai`. Guides the user through picking a provider, detecting
//! available backends, and writing `~/.rushrc`.
//!
//! # Flow
//! 1. Ask which provider to use (Ollama / OpenAI / Anthropic / Skip)
//! 2. For Ollama: probe localhost:11434, list models, offer to pull one
//! 3. For OpenAI / Anthropic: ask for API key, pick a model
//! 4. Write config and print confirmation

use crate::ai::config::{LlmConfig, ProviderType};
use nu_ansi_term::Color;
use std::io::{self, BufRead, Write};

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

/// Run the interactive setup wizard.
///
/// Returns `Ok(Some(config))` when the user completes setup, `Ok(None)` when
/// they choose to skip, and `Err` on I/O failure.
pub fn setup_wizard() -> Result<Option<LlmConfig>, String> {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    println!();
    println!(
        "{}",
        Color::Yellow
            .bold()
            .paint("rush: No AI backend configured for ? queries.")
    );
    println!();
    println!("  1) Ollama (local, private, recommended)");
    println!("  2) OpenAI API (requires key)");
    println!("  3) Anthropic API (requires key)");
    println!("  4) Skip for now");
    println!();
    print!("> ");
    stdout.flush().map_err(|e| e.to_string())?;

    let choice = read_line(&stdin)?;
    match choice.trim() {
        "1" => setup_ollama(&stdin),
        "2" => setup_openai(&stdin),
        "3" => setup_anthropic(&stdin),
        "4" | "" => {
            println!("Skipped. Run `rush --setup-ai` to configure later.");
            Ok(None)
        }
        other => {
            eprintln!("Unknown choice '{}'. Skipping.", other);
            Ok(None)
        }
    }
}

// ---------------------------------------------------------------------------
// Provider-specific setup flows
// ---------------------------------------------------------------------------

fn setup_ollama(stdin: &io::Stdin) -> Result<Option<LlmConfig>, String> {
    print!("Checking Ollama... ");
    io::stdout().flush().ok();

    let base_url = "http://localhost:11434";

    match probe_ollama(base_url) {
        Err(e) => {
            println!("{}", Color::Red.paint(format!("not found ({})", e)));
            println!();
            println!("Ollama is not running at {}.", base_url);
            println!("Install it from https://ollama.com, start it, then run `rush --setup-ai`.");
            Ok(None)
        }
        Ok(models) => {
            println!("{}", Color::Green.paint(format!("found at {}", base_url)));

            let model = if models.is_empty() {
                // No models installed — offer to pull the recommended one
                println!("No models installed.");
                offer_pull(stdin, base_url)?
            } else {
                // Show what's available and recommend one
                let display: Vec<String> = models.iter().map(|m| summarise_model(m)).collect();
                println!("Available models: {}", display.join(", "));

                let recommended = recommended_model(&models);
                println!(
                    "Recommended for shell commands: {}",
                    summarise_model(&recommended)
                );
                println!();

                // Ask whether to use recommended or another installed model
                let prompt = format!("Use {}? [Y/n] ", recommended);
                print!("{}", prompt);
                io::stdout().flush().ok();
                let answer = read_line(stdin)?;
                let answer = answer.trim().to_lowercase();

                if answer == "n" || answer == "no" {
                    // Let user type any model name
                    print!("Enter model name (e.g. llama3.2): ");
                    io::stdout().flush().ok();
                    read_line(stdin)?.trim().to_string()
                } else {
                    recommended
                }
            };

            let config = LlmConfig {
                provider: ProviderType::Ollama,
                model,
                api_key: None,
                base_url: None,
                autorun: false,
            };

            save_and_confirm(config)
        }
    }
}

fn setup_openai(stdin: &io::Stdin) -> Result<Option<LlmConfig>, String> {
    // Check env first
    let key = if let Ok(k) = std::env::var("OPENAI_API_KEY") {
        println!(
            "{}",
            Color::Green.paint("Found OPENAI_API_KEY in environment.")
        );
        k
    } else {
        print!("Enter OpenAI API key: ");
        io::stdout().flush().ok();
        let k = read_line(stdin)?.trim().to_string();
        if k.is_empty() {
            println!("No key provided. Skipping.");
            return Ok(None);
        }
        k
    };

    println!();
    println!("Available models:");
    println!("  1) gpt-4o (best quality)");
    println!("  2) gpt-4o-mini (fast, cheap)");
    println!("  3) gpt-3.5-turbo (legacy)");
    print!("Choose [1]: ");
    io::stdout().flush().ok();
    let choice = read_line(stdin)?;
    let model = match choice.trim() {
        "2" => "gpt-4o-mini",
        "3" => "gpt-3.5-turbo",
        _ => "gpt-4o",
    }
    .to_string();

    let config = LlmConfig {
        provider: ProviderType::OpenAi,
        model,
        api_key: Some(key),
        base_url: None,
        autorun: false,
    };

    save_and_confirm(config)
}

fn setup_anthropic(stdin: &io::Stdin) -> Result<Option<LlmConfig>, String> {
    let key = if let Ok(k) = std::env::var("ANTHROPIC_API_KEY") {
        println!(
            "{}",
            Color::Green.paint("Found ANTHROPIC_API_KEY in environment.")
        );
        k
    } else {
        print!("Enter Anthropic API key: ");
        io::stdout().flush().ok();
        let k = read_line(stdin)?.trim().to_string();
        if k.is_empty() {
            println!("No key provided. Skipping.");
            return Ok(None);
        }
        k
    };

    println!();
    println!("Available models:");
    println!("  1) claude-3-5-sonnet-20241022 (best quality)");
    println!("  2) claude-3-5-haiku-20241022 (fast, cheap)");
    println!("  3) claude-3-opus-20240229 (most capable, slower)");
    print!("Choose [1]: ");
    io::stdout().flush().ok();
    let choice = read_line(stdin)?;
    let model = match choice.trim() {
        "2" => "claude-3-5-haiku-20241022",
        "3" => "claude-3-opus-20240229",
        _ => "claude-3-5-sonnet-20241022",
    }
    .to_string();

    let config = LlmConfig {
        provider: ProviderType::Anthropic,
        model,
        api_key: Some(key),
        base_url: None,
        autorun: false,
    };

    save_and_confirm(config)
}

// ---------------------------------------------------------------------------
// Ollama detection helpers
// ---------------------------------------------------------------------------

/// Probe Ollama and return the list of installed model names.
///
/// Calls `GET /api/tags` — returns the `name` field from each entry.
fn probe_ollama(base_url: &str) -> Result<Vec<String>, String> {
    let url = format!("{}/api/tags", base_url);
    let response = ureq::get(&url)
        .call()
        .map_err(|e| format!("connection failed: {}", e))?;

    let body: serde_json::Value = response
        .into_body()
        .read_json()
        .map_err(|e| format!("invalid JSON: {}", e))?;

    let models = body["models"]
        .as_array()
        .ok_or("unexpected response format")?
        .iter()
        .filter_map(|m| m["name"].as_str().map(String::from))
        .collect();

    Ok(models)
}

/// When Ollama is running but has no models, offer to pull the recommended one.
fn offer_pull(stdin: &io::Stdin, base_url: &str) -> Result<String, String> {
    let model = "qwen2.5-coder:7b";
    print!("Install {}? [Y/n] ", Color::Cyan.paint(model));
    io::stdout().flush().ok();
    let answer = read_line(stdin)?.trim().to_lowercase();
    if answer == "n" || answer == "no" {
        print!("Enter model name to pull (e.g. llama3.2): ");
        io::stdout().flush().ok();
        let custom = read_line(stdin)?.trim().to_string();
        if custom.is_empty() {
            return Err("No model specified.".to_string());
        }
        pull_model(&custom, base_url)?;
        Ok(custom)
    } else {
        pull_model(model, base_url)?;
        Ok(model.to_string())
    }
}

/// Pull a model via `ollama pull` (spawns subprocess so progress streams to terminal).
fn pull_model(model: &str, _base_url: &str) -> Result<(), String> {
    print!("Pulling {}... ", Color::Cyan.paint(model));
    io::stdout().flush().ok();

    let status = std::process::Command::new("ollama")
        .args(["pull", model])
        .status()
        .map_err(|e| format!("failed to run `ollama pull`: {}", e))?;

    if status.success() {
        println!("{}", Color::Green.paint("done"));
        Ok(())
    } else {
        Err(format!(
            "`ollama pull {}` exited with status {}",
            model, status
        ))
    }
}

// ---------------------------------------------------------------------------
// Model selection helpers
// ---------------------------------------------------------------------------

/// Pick the best model for shell command generation from an installed list.
///
/// Prefers `qwen2.5-coder` variants, then `codellama`, then the first entry.
fn recommended_model(models: &[String]) -> String {
    // Preference order for coding / shell tasks
    const PREFERRED: &[&str] = &["qwen2.5-coder", "codellama", "llama3"];

    for prefix in PREFERRED {
        if let Some(m) = models.iter().find(|m| m.starts_with(prefix)) {
            return m.clone();
        }
    }

    models[0].clone()
}

/// Human-readable model summary: `qwen2.5-coder:7b` → `qwen2.5-coder (7B)`
fn summarise_model(name: &str) -> String {
    if let Some((base, tag)) = name.split_once(':') {
        let tag_upper = tag.to_uppercase();
        // Only append tag if it looks like a size suffix (e.g. "7b", "13b", "latest")
        if tag_upper.ends_with('B') || tag == "latest" {
            format!("{} ({})", base, tag_upper)
        } else {
            name.to_string()
        }
    } else {
        name.to_string()
    }
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Save config and print confirmation message.
fn save_and_confirm(config: LlmConfig) -> Result<Option<LlmConfig>, String> {
    // Set env vars for the current session
    std::env::set_var("RUSH_AI_PROVIDER", config.provider.to_string());
    std::env::set_var("RUSH_AI_MODEL", &config.model);
    if let Some(ref key) = config.api_key {
        std::env::set_var("RUSH_AI_API_KEY", key);
    }

    // Append to ~/.rushrc for persistence
    let rushrc = dirs::home_dir()
        .map(|h| h.join(".rushrc"))
        .ok_or_else(|| "Could not determine home directory".to_string())?;

    let mut lines = String::from("\n# AI configuration (added by rush --setup-ai)\n");
    lines.push_str(&format!("RUSH_AI_PROVIDER={}\n", config.provider));
    lines.push_str(&format!("RUSH_AI_MODEL={}\n", config.model));
    if let Some(ref key) = config.api_key {
        lines.push_str(&format!("RUSH_AI_API_KEY={}\n", key));
    }

    use std::io::Write;
    let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&rushrc)
        .map_err(|e| format!("Failed to open ~/.rushrc: {}", e))?;
    file.write_all(lines.as_bytes())
        .map_err(|e| format!("Failed to write ~/.rushrc: {}", e))?;

    println!();
    println!(
        "{} AI configured. Try: {} find all rust files modified today",
        Color::Green.paint("✓"),
        Color::Cyan.paint("?")
    );
    println!(
        "Config appended to {}",
        Color::DarkGray.paint(rushrc.display().to_string())
    );

    Ok(Some(config))
}

/// Read a line from stdin, stripping the trailing newline.
fn read_line(stdin: &io::Stdin) -> Result<String, String> {
    let mut line = String::new();
    stdin
        .lock()
        .read_line(&mut line)
        .map_err(|e| format!("Failed to read input: {}", e))?;
    Ok(line)
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_recommended_model_prefers_coder() {
        let models = vec![
            "llama3.2:3b".to_string(),
            "qwen2.5-coder:7b".to_string(),
            "codellama:13b".to_string(),
        ];
        assert_eq!(recommended_model(&models), "qwen2.5-coder:7b");
    }

    #[test]
    fn test_recommended_model_falls_back_to_first() {
        let models = vec!["mistral:7b".to_string(), "phi3:mini".to_string()];
        assert_eq!(recommended_model(&models), "mistral:7b");
    }

    #[test]
    fn test_summarise_model_with_size_tag() {
        assert_eq!(summarise_model("qwen2.5-coder:7b"), "qwen2.5-coder (7B)");
        assert_eq!(summarise_model("codellama:13b"), "codellama (13B)");
    }

    #[test]
    fn test_summarise_model_no_tag() {
        assert_eq!(summarise_model("llama3"), "llama3");
    }

    #[test]
    fn test_summarise_model_latest_tag() {
        assert_eq!(
            summarise_model("qwen2.5-coder:latest"),
            "qwen2.5-coder (latest)"
        );
    }
}
