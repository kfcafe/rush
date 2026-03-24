use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::time::{Duration, Instant};
use ureq::{http, ResponseExt};

#[derive(Debug, Serialize, Deserialize)]
struct FetchResponse {
    status: u16,
    status_text: String,
    headers: HashMap<String, String>,
    body: Value,
    response_time_ms: u64,
    url: String,
}

#[derive(Debug)]
struct FetchOptions {
    method: String,
    headers: Vec<(String, String)>,
    body: Option<String>,
    timeout: Option<Duration>,
    follow_redirects: bool,
    output_file: Option<String>,
    json_output: bool,
    verbose: bool,
    include_headers: bool,
}

impl Default for FetchOptions {
    fn default() -> Self {
        Self {
            method: "GET".to_string(),
            headers: Vec::new(),
            body: None,
            timeout: Some(Duration::from_secs(30)),
            follow_redirects: true,
            output_file: None,
            json_output: false,
            verbose: false,
            include_headers: false,
        }
    }
}

/// Parse command line arguments and extract fetch options
fn parse_args(args: &[String]) -> Result<(String, FetchOptions)> {
    let mut opts = FetchOptions::default();
    let mut url: Option<String> = None;
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];

        match arg.as_str() {
            "--json" => {
                opts.json_output = true;
            }
            "-X" | "--request" => {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("Missing method after {}", arg));
                }
                opts.method = args[i].to_uppercase();
            }
            "-H" | "--header" => {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("Missing header value after {}", arg));
                }
                let header = &args[i];
                if let Some((key, value)) = header.split_once(':') {
                    opts.headers
                        .push((key.trim().to_string(), value.trim().to_string()));
                } else {
                    return Err(anyhow!("Invalid header format. Use 'Key: Value'"));
                }
            }
            "-d" | "--data" => {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("Missing data after {}", arg));
                }
                let data = &args[i];
                if data == "@-" {
                    // Read from stdin
                    use std::io::Read;
                    let mut buffer = String::new();
                    std::io::stdin()
                        .read_to_string(&mut buffer)
                        .context("Failed to read data from stdin")?;
                    opts.body = Some(buffer);
                } else if data.starts_with('@') {
                    // Read from file
                    let file_path = &data[1..];
                    let content = fs::read_to_string(file_path)
                        .with_context(|| format!("Failed to read data from file: {}", file_path))?;
                    opts.body = Some(content);
                } else {
                    // Use data directly
                    opts.body = Some(data.to_string());
                }
            }
            "--timeout" => {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("Missing timeout value"));
                }
                let seconds: u64 = args[i]
                    .parse()
                    .context("Invalid timeout value, must be a number")?;
                opts.timeout = Some(Duration::from_secs(seconds));
            }
            "--no-follow" => {
                opts.follow_redirects = false;
            }
            "-o" | "--output" => {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("Missing output file path"));
                }
                opts.output_file = Some(args[i].clone());
            }
            "-v" | "--verbose" => {
                opts.verbose = true;
            }
            "-i" | "--include" => {
                opts.include_headers = true;
            }
            "--method" => {
                i += 1;
                if i >= args.len() {
                    return Err(anyhow!("Missing method after --method"));
                }
                opts.method = args[i].to_uppercase();
            }
            _ => {
                if arg.starts_with('-') {
                    return Err(anyhow!("Unknown option: {}", arg));
                }
                // First non-flag argument is the URL
                if url.is_none() {
                    url = Some(arg.clone());
                } else {
                    return Err(anyhow!(
                        "Multiple URLs provided: {} and {}",
                        url.unwrap(),
                        arg
                    ));
                }
            }
        }

        i += 1;
    }

    let url = url.ok_or_else(|| anyhow!("No URL provided"))?;
    Ok((url, opts))
}

/// Build a configured ureq Agent based on fetch options
fn build_agent(opts: &FetchOptions) -> Result<ureq::Agent> {
    let mut builder = ureq::Agent::config_builder();

    if let Some(timeout) = opts.timeout {
        builder = builder.timeout_global(Some(timeout));
    }

    if !opts.follow_redirects {
        builder = builder.max_redirects(0);
    }

    // Disable treating HTTP error status codes as Err — we handle them ourselves
    builder = builder.http_status_as_error(false);

    let config = builder.build();
    Ok(ureq::Agent::new_with_config(config))
}

/// Execute the HTTP request, returning the response and elapsed time in ms
fn execute_request(url: &str, opts: &FetchOptions) -> Result<(http::Response<ureq::Body>, u64)> {
    let agent = build_agent(opts)?;

    // Build request using the http crate's builder (ureq 3.x accepts http::Request)
    let mut req_builder = http::Request::builder()
        .method(opts.method.as_str())
        .uri(url);

    for (key, value) in &opts.headers {
        req_builder = req_builder.header(key.as_str(), value.as_str());
    }

    let start = Instant::now();
    let response = match &opts.body {
        Some(body) => {
            let request = req_builder
                .body(body.as_str())
                .context("Failed to build HTTP request")?;
            agent.run(request).context("Failed to send HTTP request")?
        }
        None => {
            let request = req_builder
                .body(())
                .context("Failed to build HTTP request")?;
            agent.run(request).context("Failed to send HTTP request")?
        }
    };
    let elapsed = start.elapsed();

    Ok((response, elapsed.as_millis() as u64))
}

/// Parse a response body as JSON, falling back to a plain string
fn parse_body_as_json(body_text: &str, content_type: &str) -> Value {
    let is_json =
        content_type.contains("application/json") || content_type.contains("application/ld+json");
    let looks_like_json = body_text.trim().starts_with('{') || body_text.trim().starts_with('[');

    if is_json || looks_like_json {
        serde_json::from_str::<Value>(body_text)
            .unwrap_or_else(|_| Value::String(body_text.to_string()))
    } else {
        Value::String(body_text.to_string())
    }
}

/// Format a response as the structured FetchResponse JSON type
fn format_json_response(
    response: http::Response<ureq::Body>,
    response_time_ms: u64,
) -> Result<FetchResponse> {
    let status = response.status();
    let final_url = response.get_uri().to_string();

    let mut headers = HashMap::new();
    for (key, value) in response.headers() {
        if let Ok(value_str) = value.to_str() {
            headers.insert(key.to_string(), value_str.to_string());
        }
    }

    let content_type = headers.get("content-type").cloned().unwrap_or_default();

    let body_text = response
        .into_body()
        .read_to_string()
        .context("Failed to read response body")?;

    let body = parse_body_as_json(&body_text, &content_type);

    Ok(FetchResponse {
        status: status.as_u16(),
        status_text: status.canonical_reason().unwrap_or("Unknown").to_string(),
        headers,
        body,
        response_time_ms,
        url: final_url,
    })
}

pub fn builtin_fetch(args: &[String], _runtime: &mut Runtime) -> Result<ExecutionResult> {
    if args.is_empty() {
        return Ok(ExecutionResult::error(
            "fetch: usage: fetch [OPTIONS] URL\n\nOptions:\n  --json              Output structured JSON response\n  -X, --request METHOD    HTTP method (GET, POST, PUT, DELETE, etc.)\n  -H, --header HEADER     Custom header (format: 'Key: Value')\n  -d, --data DATA         Request body (use @file or @- for stdin)\n  --timeout SECONDS       Request timeout in seconds (default: 30)\n  --no-follow             Don't follow redirects\n  -o, --output FILE       Save response to file\n  -v, --verbose           Verbose output\n  -i, --include           Include headers in text output\n".to_string(),
        ));
    }

    let (url, opts) = parse_args(args)?;
    let (response, response_time_ms) =
        execute_request(&url, &opts).context("HTTP request failed")?;

    let status = response.status();

    if opts.json_output {
        let fetch_response = format_json_response(response, response_time_ms)?;
        let json_output = serde_json::to_string_pretty(&fetch_response)
            .context("Failed to serialize JSON response")?;

        let exit_code = if status.is_success() {
            0
        } else {
            status.as_u16() as i32
        };

        Ok(ExecutionResult {
            output: Output::Text(json_output + "\n"),
            stderr: String::new(),
            exit_code,
            error: None,
        })
    } else {
        let mut output = String::new();

        if opts.include_headers || opts.verbose {
            output.push_str(&format!(
                "HTTP/1.1 {} {}\n",
                status.as_u16(),
                status.canonical_reason().unwrap_or("Unknown")
            ));
            for (key, value) in response.headers() {
                if let Ok(value_str) = value.to_str() {
                    output.push_str(&format!("{}: {}\n", key, value_str));
                }
            }
            output.push('\n');
        }

        let body_text = response
            .into_body()
            .read_to_string()
            .context("Failed to read response body")?;

        if let Some(output_file) = &opts.output_file {
            fs::write(output_file, &body_text)
                .with_context(|| format!("Failed to write to file: {}", output_file))?;
            output.push_str(&format!("Response saved to {}\n", output_file));
        } else {
            output.push_str(&body_text);
            if !body_text.ends_with('\n') {
                output.push('\n');
            }
        }

        let exit_code = if status.is_success() {
            0
        } else {
            status.as_u16() as i32
        };

        Ok(ExecutionResult {
            output: Output::Text(output),
            stderr: String::new(),
            exit_code,
            error: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_args_basic() {
        let args = vec!["https://example.com".to_string()];
        let (url, opts) = parse_args(&args).unwrap();
        assert_eq!(url, "https://example.com");
        assert_eq!(opts.method, "GET");
        assert!(!opts.json_output);
    }

    #[test]
    fn test_parse_args_with_json_flag() {
        let args = vec!["--json".to_string(), "https://api.example.com".to_string()];
        let (url, opts) = parse_args(&args).unwrap();
        assert_eq!(url, "https://api.example.com");
        assert!(opts.json_output);
    }

    #[test]
    fn test_parse_args_with_method() {
        let args = vec![
            "-X".to_string(),
            "POST".to_string(),
            "https://api.example.com".to_string(),
        ];
        let (url, opts) = parse_args(&args).unwrap();
        assert_eq!(url, "https://api.example.com");
        assert_eq!(opts.method, "POST");
    }

    #[test]
    fn test_parse_args_with_headers() {
        let args = vec![
            "-H".to_string(),
            "Content-Type: application/json".to_string(),
            "-H".to_string(),
            "Authorization: Bearer token123".to_string(),
            "https://api.example.com".to_string(),
        ];
        let (url, opts) = parse_args(&args).unwrap();
        assert_eq!(url, "https://api.example.com");
        assert_eq!(opts.headers.len(), 2);
        assert_eq!(
            opts.headers[0],
            ("Content-Type".to_string(), "application/json".to_string())
        );
        assert_eq!(
            opts.headers[1],
            ("Authorization".to_string(), "Bearer token123".to_string())
        );
    }

    #[test]
    fn test_parse_args_with_data() {
        let args = vec![
            "-d".to_string(),
            r#"{"key":"value"}"#.to_string(),
            "https://api.example.com".to_string(),
        ];
        let (url, opts) = parse_args(&args).unwrap();
        assert_eq!(url, "https://api.example.com");
        assert_eq!(opts.body, Some(r#"{"key":"value"}"#.to_string()));
    }

    #[test]
    fn test_parse_args_with_timeout() {
        let args = vec![
            "--timeout".to_string(),
            "60".to_string(),
            "https://example.com".to_string(),
        ];
        let (url, opts) = parse_args(&args).unwrap();
        assert_eq!(url, "https://example.com");
        assert_eq!(opts.timeout, Some(Duration::from_secs(60)));
    }

    #[test]
    fn test_parse_args_no_follow() {
        let args = vec!["--no-follow".to_string(), "https://example.com".to_string()];
        let (url, opts) = parse_args(&args).unwrap();
        assert_eq!(url, "https://example.com");
        assert!(!opts.follow_redirects);
    }

    #[test]
    fn test_parse_args_output_file() {
        let args = vec![
            "-o".to_string(),
            "output.json".to_string(),
            "https://example.com".to_string(),
        ];
        let (url, opts) = parse_args(&args).unwrap();
        assert_eq!(url, "https://example.com");
        assert_eq!(opts.output_file, Some("output.json".to_string()));
    }

    #[test]
    fn test_parse_args_complex() {
        let args = vec![
            "--json".to_string(),
            "-X".to_string(),
            "POST".to_string(),
            "-H".to_string(),
            "Content-Type: application/json".to_string(),
            "-d".to_string(),
            r#"{"test":"data"}"#.to_string(),
            "--timeout".to_string(),
            "10".to_string(),
            "https://api.example.com/endpoint".to_string(),
        ];
        let (url, opts) = parse_args(&args).unwrap();
        assert_eq!(url, "https://api.example.com/endpoint");
        assert!(opts.json_output);
        assert_eq!(opts.method, "POST");
        assert_eq!(opts.headers.len(), 1);
        assert_eq!(opts.body, Some(r#"{"test":"data"}"#.to_string()));
        assert_eq!(opts.timeout, Some(Duration::from_secs(10)));
    }

    #[test]
    fn test_parse_args_missing_url() {
        let args = vec!["--json".to_string()];
        let result = parse_args(&args);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("No URL provided"));
    }

    #[test]
    fn test_parse_args_invalid_header() {
        let args = vec![
            "-H".to_string(),
            "InvalidHeader".to_string(),
            "https://example.com".to_string(),
        ];
        let result = parse_args(&args);
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid header format"));
    }

    // Integration tests that hit a real HTTP endpoint
    // These tests use httpbin.org which is a public HTTP testing service

    #[test]
    #[ignore] // Ignore by default to avoid network calls in CI
    fn test_fetch_basic_get() {
        let mut runtime = Runtime::new();
        let args = vec!["https://httpbin.org/get".to_string()];
        let result = builtin_fetch(&args, &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(!result.stdout().is_empty());
    }

    #[test]
    #[ignore]
    fn test_fetch_json_output() {
        let mut runtime = Runtime::new();
        let args = vec!["--json".to_string(), "https://httpbin.org/get".to_string()];
        let result = builtin_fetch(&args, &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);

        // Verify it's valid JSON
        let json: serde_json::Value = serde_json::from_str(&result.stdout()).unwrap();
        assert!(json.get("status").is_some());
        assert!(json.get("body").is_some());
        assert!(json.get("headers").is_some());
        assert!(json.get("response_time_ms").is_some());
    }

    #[test]
    #[ignore]
    fn test_fetch_post_with_data() {
        let mut runtime = Runtime::new();
        let args = vec![
            "-X".to_string(),
            "POST".to_string(),
            "-d".to_string(),
            r#"{"test":"data"}"#.to_string(),
            "--json".to_string(),
            "https://httpbin.org/post".to_string(),
        ];
        let result = builtin_fetch(&args, &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);

        let json: serde_json::Value = serde_json::from_str(&result.stdout()).unwrap();
        assert_eq!(json["status"], 200);
    }

    #[test]
    #[ignore]
    fn test_fetch_custom_headers() {
        let mut runtime = Runtime::new();
        let args = vec![
            "--json".to_string(),
            "-H".to_string(),
            "X-Custom-Header: test-value".to_string(),
            "https://httpbin.org/headers".to_string(),
        ];
        let result = builtin_fetch(&args, &mut runtime).unwrap();
        assert_eq!(result.exit_code, 0);

        let json: serde_json::Value = serde_json::from_str(&result.stdout()).unwrap();
        assert_eq!(json["status"], 200);
    }

    #[test]
    #[ignore]
    fn test_fetch_timeout() {
        let mut runtime = Runtime::new();
        // httpbin has a /delay endpoint
        let args = vec![
            "--timeout".to_string(),
            "1".to_string(),
            "https://httpbin.org/delay/5".to_string(),
        ];
        let result = builtin_fetch(&args, &mut runtime);
        // Should timeout and fail
        assert!(result.is_err() || result.unwrap().exit_code != 0);
    }

    #[test]
    #[ignore]
    fn test_fetch_404_error() {
        let mut runtime = Runtime::new();
        let args = vec![
            "--json".to_string(),
            "https://httpbin.org/status/404".to_string(),
        ];
        let result = builtin_fetch(&args, &mut runtime).unwrap();
        assert_eq!(result.exit_code, 404);

        let json: serde_json::Value = serde_json::from_str(&result.stdout()).unwrap();
        assert_eq!(json["status"], 404);
    }
}
