use crate::executor::{ExecutionResult, Output};
use crate::runtime::Runtime;
use anyhow::{anyhow, Context, Result};
use reqwest::blocking::{Client, RequestBuilder, Response};
use reqwest::header::{HeaderName, HeaderValue};
use reqwest::Method;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::str::FromStr;
use std::time::{Duration, Instant};

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
    method: Method,
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
            method: Method::GET,
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
                opts.method =
                    Method::from_str(&args[i].to_uppercase()).context("Invalid HTTP method")?;
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
                opts.method =
                    Method::from_str(&args[i].to_uppercase()).context("Invalid HTTP method")?;
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

/// Execute the HTTP request
fn execute_request(url: &str, opts: &FetchOptions) -> Result<(Response, u64)> {
    let client = Client::builder()
        .redirect(if opts.follow_redirects {
            reqwest::redirect::Policy::default()
        } else {
            reqwest::redirect::Policy::none()
        })
        .build()
        .context("Failed to create HTTP client")?;

    let mut request: RequestBuilder = client.request(opts.method.clone(), url);

    // Add custom headers
    for (key, value) in &opts.headers {
        let header_name =
            HeaderName::from_str(key).with_context(|| format!("Invalid header name: {}", key))?;
        let header_value = HeaderValue::from_str(value)
            .with_context(|| format!("Invalid header value: {}", value))?;
        request = request.header(header_name, header_value);
    }

    // Add body if present
    if let Some(body) = &opts.body {
        request = request.body(body.clone());
    }

    // Add timeout if specified
    if let Some(timeout) = opts.timeout {
        request = request.timeout(timeout);
    }

    // Execute request and measure time
    let start = Instant::now();
    let response = request.send().context("Failed to send HTTP request")?;
    let elapsed = start.elapsed();

    Ok((response, elapsed.as_millis() as u64))
}

/// Parse response body as JSON or return as string
fn parse_response_body(response: Response) -> Result<Value> {
    let content_type = response
        .headers()
        .get("content-type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string();

    let body_text = response.text().context("Failed to read response body")?;

    // Try to parse as JSON if content type suggests it, or if it looks like JSON
    if content_type.contains("application/json") || content_type.contains("application/ld+json") {
        match serde_json::from_str::<Value>(&body_text) {
            Ok(json_value) => Ok(json_value),
            Err(_) => Ok(Value::String(body_text)),
        }
    } else if body_text.trim().starts_with('{') || body_text.trim().starts_with('[') {
        // Looks like JSON, try to parse
        match serde_json::from_str::<Value>(&body_text) {
            Ok(json_value) => Ok(json_value),
            Err(_) => Ok(Value::String(body_text)),
        }
    } else {
        Ok(Value::String(body_text))
    }
}

/// Format response as JSON structure
fn format_json_response(response: Response, response_time_ms: u64) -> Result<FetchResponse> {
    let status = response.status();
    let final_url = response.url().to_string();

    // Extract headers
    let mut headers = HashMap::new();
    for (key, value) in response.headers() {
        if let Ok(value_str) = value.to_str() {
            headers.insert(key.to_string(), value_str.to_string());
        }
    }

    // Parse body
    let body = parse_response_body(response)?;

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

    // Execute the request
    let (response, response_time_ms) =
        execute_request(&url, &opts).context("HTTP request failed")?;

    let status = response.status();

    if opts.json_output {
        // Format as JSON
        let fetch_response = format_json_response(response, response_time_ms)?;
        let json_output = serde_json::to_string_pretty(&fetch_response)
            .context("Failed to serialize JSON response")?;

        // Check for HTTP errors
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
        // Text output mode
        let mut output = String::new();

        // Include headers if requested
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

        // Get response body
        let body_text = response.text().context("Failed to read response body")?;

        // Save to file if requested
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

        // Check for HTTP errors
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
        assert_eq!(opts.method, Method::GET);
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
        assert_eq!(opts.method, Method::POST);
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
        assert_eq!(opts.method, Method::POST);
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
