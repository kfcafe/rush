use rush::builtins::Builtins;
use rush::runtime::Runtime;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_grep_basic_search() {
    let tmp = TempDir::new().unwrap();
    let file_path = tmp.path().join("test.txt");
    fs::write(&file_path, "hello world\nfoo bar\nhello rust\n").unwrap();

    let mut runtime = Runtime::new();
    runtime.set_cwd(tmp.path().to_path_buf());

    let builtins = Builtins::new();
    let args = vec!["hello".to_string(), file_path.to_string_lossy().to_string()];

    let result = builtins.execute("grep", args, &mut runtime).unwrap();
    assert_eq!(result.exit_code, 0);
    // Output includes line numbers (1:, 3:) and ANSI color codes
    assert!(
        result.stdout().contains("1:")
            && result.stdout().contains("hello")
            && result.stdout().contains("world")
    );
    assert!(
        result.stdout().contains("3:")
            && result.stdout().contains("hello")
            && result.stdout().contains("rust")
    );
    // "foo bar" should not appear in results
    assert!(!result.stdout().contains("foo") || !result.stdout().contains("bar"));
}

#[test]
fn test_grep_case_insensitive() {
    let tmp = TempDir::new().unwrap();
    let file_path = tmp.path().join("test.txt");
    fs::write(&file_path, "HELLO world\nfoo BAR\nhello rust\n").unwrap();

    let mut runtime = Runtime::new();
    runtime.set_cwd(tmp.path().to_path_buf());

    let builtins = Builtins::new();
    let args = vec![
        "-i".to_string(),
        "hello".to_string(),
        file_path.to_string_lossy().to_string(),
    ];

    let result = builtins.execute("grep", args, &mut runtime).unwrap();
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout().contains("HELLO") && result.stdout().contains("world"));
    assert!(result.stdout().contains("hello") && result.stdout().contains("rust"));
}

#[test]
fn test_grep_recursive() {
    let tmp = TempDir::new().unwrap();
    fs::create_dir(tmp.path().join("subdir")).unwrap();

    let file1 = tmp.path().join("file1.txt");
    let file2 = tmp.path().join("subdir").join("file2.txt");

    fs::write(&file1, "hello world\n").unwrap();
    fs::write(&file2, "hello rust\n").unwrap();

    let mut runtime = Runtime::new();
    runtime.set_cwd(tmp.path().to_path_buf());

    let builtins = Builtins::new();
    let args = vec![
        "-r".to_string(),
        "hello".to_string(),
        tmp.path().to_string_lossy().to_string(),
    ];

    let result = builtins.execute("grep", args, &mut runtime).unwrap();
    assert_eq!(result.exit_code, 0);
    assert!(result.stdout().contains("hello") && result.stdout().contains("world"));
    assert!(result.stdout().contains("hello") && result.stdout().contains("rust"));
}

#[test]
fn test_grep_invert_match() {
    let tmp = TempDir::new().unwrap();
    let file_path = tmp.path().join("test.txt");
    fs::write(&file_path, "hello world\nfoo bar\nhello rust\n").unwrap();

    let mut runtime = Runtime::new();
    runtime.set_cwd(tmp.path().to_path_buf());

    let builtins = Builtins::new();
    let args = vec![
        "-v".to_string(),
        "hello".to_string(),
        file_path.to_string_lossy().to_string(),
    ];

    let result = builtins.execute("grep", args, &mut runtime).unwrap();
    assert_eq!(result.exit_code, 0);
    // Invert match: should only show lines WITHOUT "hello"
    assert!(!result.stdout().contains("hello"));
    assert!(result.stdout().contains("foo") && result.stdout().contains("bar"));
}

#[test]
fn test_grep_no_match() {
    let tmp = TempDir::new().unwrap();
    let file_path = tmp.path().join("test.txt");
    fs::write(&file_path, "hello world\nfoo bar\n").unwrap();

    let mut runtime = Runtime::new();
    runtime.set_cwd(tmp.path().to_path_buf());

    let builtins = Builtins::new();
    let args = vec![
        "nonexistent".to_string(),
        file_path.to_string_lossy().to_string(),
    ];

    let result = builtins.execute("grep", args, &mut runtime).unwrap();
    assert_eq!(result.exit_code, 1);
    assert!(result.stdout().is_empty());
}

#[test]
fn test_grep_json_output() {
    let tmp = TempDir::new().unwrap();
    let file_path = tmp.path().join("test.txt");
    fs::write(&file_path, "hello world\nfoo bar\nhello rust\n").unwrap();

    let mut runtime = Runtime::new();
    runtime.set_cwd(tmp.path().to_path_buf());

    let builtins = Builtins::new();
    let args = vec![
        "--json".to_string(),
        "hello".to_string(),
        file_path.to_string_lossy().to_string(),
    ];

    let result = builtins.execute("grep", args, &mut runtime).unwrap();
    assert_eq!(result.exit_code, 0);

    let output = result.stdout();
    let json: serde_json::Value = serde_json::from_str(&output).expect("Valid JSON");
    let matches = json.as_array().expect("JSON array");

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0]["line_number"], 1);
    assert!(matches[0]["full_line"].as_str().unwrap().contains("hello"));
    assert_eq!(matches[1]["line_number"], 3);
}

#[test]
fn test_grep_json_with_context() {
    let tmp = TempDir::new().unwrap();
    let file_path = tmp.path().join("test.txt");
    fs::write(
        &file_path,
        "line 1\nline 2 match\nline 3\nline 4\nline 5 match\nline 6\n",
    )
    .unwrap();

    let mut runtime = Runtime::new();
    runtime.set_cwd(tmp.path().to_path_buf());

    let builtins = Builtins::new();
    let args = vec![
        "--json".to_string(),
        "-C".to_string(),
        "1".to_string(),
        "match".to_string(),
        file_path.to_string_lossy().to_string(),
    ];

    let result = builtins.execute("grep", args, &mut runtime).unwrap();
    assert_eq!(result.exit_code, 0);

    let output = result.stdout();
    let json: serde_json::Value = serde_json::from_str(&output).expect("Valid JSON");
    let matches = json.as_array().expect("JSON array");

    assert_eq!(matches.len(), 2);

    // Check first match has context
    let context_before = matches[0]["context_before"].as_array().unwrap();
    assert_eq!(context_before.len(), 1);
    assert!(context_before[0].as_str().unwrap().contains("line 1"));

    let context_after = matches[0]["context_after"].as_array().unwrap();
    assert_eq!(context_after.len(), 1);
    assert!(context_after[0].as_str().unwrap().contains("line 3"));
}

#[test]
fn test_grep_json_case_insensitive() {
    let tmp = TempDir::new().unwrap();
    let file_path = tmp.path().join("test.txt");
    fs::write(&file_path, "HELLO world\nhello rust\n").unwrap();

    let mut runtime = Runtime::new();
    runtime.set_cwd(tmp.path().to_path_buf());

    let builtins = Builtins::new();
    let args = vec![
        "--json".to_string(),
        "-i".to_string(),
        "hello".to_string(),
        file_path.to_string_lossy().to_string(),
    ];

    let result = builtins.execute("grep", args, &mut runtime).unwrap();
    assert_eq!(result.exit_code, 0);

    let output = result.stdout();
    let json: serde_json::Value = serde_json::from_str(&output).expect("Valid JSON");
    let matches = json.as_array().expect("JSON array");

    assert_eq!(matches.len(), 2);
}

#[test]
fn test_grep_json_no_match() {
    let tmp = TempDir::new().unwrap();
    let file_path = tmp.path().join("test.txt");
    fs::write(&file_path, "hello world\nfoo bar\n").unwrap();

    let mut runtime = Runtime::new();
    runtime.set_cwd(tmp.path().to_path_buf());

    let builtins = Builtins::new();
    let args = vec![
        "--json".to_string(),
        "nonexistent".to_string(),
        file_path.to_string_lossy().to_string(),
    ];

    let result = builtins.execute("grep", args, &mut runtime).unwrap();
    assert_eq!(result.exit_code, 1);

    let output = result.stdout();
    let json: serde_json::Value = serde_json::from_str(&output).expect("Valid JSON");
    let matches = json.as_array().expect("JSON array");
    assert_eq!(matches.len(), 0);
}
