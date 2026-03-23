// Project and Git context detection
// Provides smart context awareness for project type detection and Git integration

#[cfg(feature = "git-builtins")]
use crate::git::GitContext;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// All supported project types with their marker files
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ProjectType {
    Rust,
    Node,
    Python,
    Go,
    Ruby,
    Java,
    Elixir,
    Unknown,
}

impl ProjectType {
    /// Returns the marker files that identify this project type
    pub fn marker_files(&self) -> &[&str] {
        match self {
            ProjectType::Rust => &["Cargo.toml"],
            ProjectType::Node => &["package.json"],
            ProjectType::Python => &["pyproject.toml", "setup.py", "requirements.txt", "Pipfile"],
            ProjectType::Go => &["go.mod"],
            ProjectType::Ruby => &["Gemfile"],
            ProjectType::Java => &["pom.xml", "build.gradle", "build.gradle.kts"],
            ProjectType::Elixir => &["mix.exs"],
            ProjectType::Unknown => &[],
        }
    }

    /// Detect project type from a given path by checking for marker files
    /// Returns the first matching project type
    pub fn detect(path: &Path) -> Self {
        let types = [
            ProjectType::Rust,
            ProjectType::Node,
            ProjectType::Python,
            ProjectType::Go,
            ProjectType::Ruby,
            ProjectType::Java,
            ProjectType::Elixir,
        ];

        for project_type in types {
            for marker in project_type.marker_files() {
                if path.join(marker).exists() {
                    return project_type;
                }
            }
        }

        ProjectType::Unknown
    }

    /// Find project root by walking up the directory tree
    /// Returns the closest ancestor directory containing a marker file
    pub fn find_project_root(start_path: &Path) -> Option<(PathBuf, ProjectType)> {
        let mut current = start_path.to_path_buf();

        loop {
            let detected_type = Self::detect(&current);
            if detected_type != ProjectType::Unknown {
                return Some((current, detected_type));
            }

            // Move up to parent directory
            if !current.pop() {
                break;
            }
        }

        None
    }

    /// Get the command mapping for a given generic command
    pub fn route_command(&self, generic_cmd: &str) -> Option<String> {
        match (self, generic_cmd) {
            // Test commands
            (ProjectType::Rust, "test") => Some("cargo test".to_string()),
            (ProjectType::Node, "test") => Some("npm test".to_string()),
            (ProjectType::Python, "test") => Some("pytest".to_string()),
            (ProjectType::Go, "test") => Some("go test ./...".to_string()),
            (ProjectType::Ruby, "test") => Some("bundle exec rake test".to_string()),
            (ProjectType::Java, "test") => Some("mvn test".to_string()),
            (ProjectType::Elixir, "test") => Some("mix test".to_string()),

            // Build commands
            (ProjectType::Rust, "build") => Some("cargo build".to_string()),
            (ProjectType::Node, "build") => Some("npm run build".to_string()),
            (ProjectType::Go, "build") => Some("go build".to_string()),
            (ProjectType::Ruby, "build") => Some("bundle install".to_string()),
            (ProjectType::Java, "build") => Some("mvn package".to_string()),
            (ProjectType::Elixir, "build") => Some("mix compile".to_string()),

            // Run commands
            (ProjectType::Rust, "run") => Some("cargo run".to_string()),
            (ProjectType::Node, "run") => Some("npm start".to_string()),
            (ProjectType::Python, "run") => Some("python -m".to_string()),
            (ProjectType::Go, "run") => Some("go run .".to_string()),
            (ProjectType::Ruby, "run") => Some("bundle exec ruby".to_string()),
            (ProjectType::Java, "run") => Some("mvn exec:java".to_string()),
            (ProjectType::Elixir, "run") => Some("mix run".to_string()),

            // Install commands
            (ProjectType::Rust, "install") => Some("cargo install".to_string()),
            (ProjectType::Node, "install") => Some("npm install".to_string()),
            (ProjectType::Python, "install") => Some("pip install".to_string()),
            (ProjectType::Go, "install") => Some("go install".to_string()),
            (ProjectType::Ruby, "install") => Some("bundle install".to_string()),
            (ProjectType::Java, "install") => Some("mvn install".to_string()),
            (ProjectType::Elixir, "install") => Some("mix deps.get".to_string()),

            // Format commands
            (ProjectType::Rust, "format") => Some("cargo fmt".to_string()),
            (ProjectType::Node, "format") => Some("npm run format".to_string()),
            (ProjectType::Python, "format") => Some("black .".to_string()),
            (ProjectType::Go, "format") => Some("go fmt ./...".to_string()),
            (ProjectType::Ruby, "format") => Some("rubocop -a".to_string()),
            (ProjectType::Elixir, "format") => Some("mix format".to_string()),

            // Lint commands
            (ProjectType::Rust, "lint") => Some("cargo clippy".to_string()),
            (ProjectType::Node, "lint") => Some("npm run lint".to_string()),
            (ProjectType::Python, "lint") => Some("pylint".to_string()),
            (ProjectType::Go, "lint") => Some("golangci-lint run".to_string()),
            (ProjectType::Ruby, "lint") => Some("rubocop".to_string()),
            (ProjectType::Elixir, "lint") => Some("mix credo".to_string()),

            _ => None,
        }
    }
}

/// Cache for project type detection to avoid repeated filesystem checks
#[derive(Debug, Clone)]
struct ProjectCache {
    cache: Arc<Mutex<HashMap<PathBuf, (ProjectType, PathBuf)>>>,
}

impl ProjectCache {
    fn new() -> Self {
        Self {
            cache: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    fn get(&self, path: &Path) -> Option<(ProjectType, PathBuf)> {
        let cache = self.cache.lock().ok()?;
        cache.get(path).cloned()
    }

    fn insert(&self, path: PathBuf, project_type: ProjectType, root: PathBuf) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(path, (project_type, root));
        }
    }

    fn clear(&self) {
        if let Ok(mut cache) = self.cache.lock() {
            cache.clear();
        }
    }
}

/// Combined project and Git context
pub struct Context {
    project_type: ProjectType,
    project_root: Option<PathBuf>,
    #[cfg(feature = "git-builtins")]
    git_context: Option<GitContext>,
    cache: ProjectCache,
}

impl Context {
    /// Create a new context without detection
    pub fn new() -> Self {
        Self {
            project_type: ProjectType::Unknown,
            project_root: None,
            #[cfg(feature = "git-builtins")]
            git_context: None,
            cache: ProjectCache::new(),
        }
    }

    /// Create a context with automatic detection for the given path
    pub fn detect(path: &Path) -> Self {
        let mut ctx = Self::new();
        ctx.detect_all(path);
        ctx
    }

    /// Detect both project type and Git context
    pub fn detect_all(&mut self, path: &Path) {
        self.detect_project(path);
        #[cfg(feature = "git-builtins")]
        self.detect_git(path);
    }

    /// Detect project type from the given path, using cache if available
    pub fn detect_project(&mut self, path: &Path) -> ProjectType {
        // Check cache first
        if let Some((cached_type, cached_root)) = self.cache.get(path) {
            self.project_type = cached_type.clone();
            self.project_root = Some(cached_root);
            return cached_type;
        }

        // Perform detection
        if let Some((root, project_type)) = ProjectType::find_project_root(path) {
            self.project_type = project_type.clone();
            self.project_root = Some(root.clone());
            self.cache
                .insert(path.to_path_buf(), project_type.clone(), root);
            project_type
        } else {
            self.project_type = ProjectType::Unknown;
            self.project_root = None;
            ProjectType::Unknown
        }
    }

    /// Detect Git context for the given path
    #[cfg(feature = "git-builtins")]
    pub fn detect_git(&mut self, path: &Path) {
        self.git_context = Some(GitContext::new(path));
    }

    /// Get the detected project type
    pub fn get_project_type(&self) -> &ProjectType {
        &self.project_type
    }

    /// Get the project root directory if detected
    pub fn get_project_root(&self) -> Option<&Path> {
        self.project_root.as_deref()
    }

    /// Get the Git context if available
    #[cfg(feature = "git-builtins")]
    pub fn get_git_context(&self) -> Option<&GitContext> {
        self.git_context.as_ref()
    }

    /// Route a generic command to a project-specific command
    pub fn route_command(&self, generic_cmd: &str) -> Option<String> {
        self.project_type.route_command(generic_cmd)
    }

    /// Clear the detection cache
    pub fn clear_cache(&self) {
        self.cache.clear();
    }

    /// Check if we're in a Git repository
    pub fn is_git_repo(&self) -> bool {
        #[cfg(feature = "git-builtins")]
        {
            self.git_context
                .as_ref()
                .map(|ctx| ctx.is_git_repo())
                .unwrap_or(false)
        }
        #[cfg(not(feature = "git-builtins"))]
        false
    }

    /// Get a combined context string for display (project type + git status)
    pub fn status_string(&self) -> String {
        let mut parts = Vec::new();

        // Add project type if known
        if self.project_type != ProjectType::Unknown {
            parts.push(format!("{:?}", self.project_type));
        }

        // Add git status if available
        #[cfg(feature = "git-builtins")]
        if let Some(git_ctx) = &self.git_context {
            let git_status = git_ctx.status_summary();
            let git_str = git_status.prompt_string();
            if !git_str.is_empty() {
                parts.push(git_str);
            }
        }

        parts.join(" ")
    }
}

impl Default for Context {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_project(project_type: ProjectType) -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let markers = project_type.marker_files();
        if !markers.is_empty() {
            let marker_path = temp_dir.path().join(markers[0]);
            fs::write(&marker_path, "").unwrap();
        }
        temp_dir
    }

    #[test]
    fn test_detect_rust_project() {
        let temp_dir = create_test_project(ProjectType::Rust);
        let detected = ProjectType::detect(temp_dir.path());
        assert_eq!(detected, ProjectType::Rust);
    }

    #[test]
    fn test_detect_node_project() {
        let temp_dir = create_test_project(ProjectType::Node);
        let detected = ProjectType::detect(temp_dir.path());
        assert_eq!(detected, ProjectType::Node);
    }

    #[test]
    fn test_detect_python_project() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("pyproject.toml"), "").unwrap();
        let detected = ProjectType::detect(temp_dir.path());
        assert_eq!(detected, ProjectType::Python);
    }

    #[test]
    fn test_detect_python_requirements() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("requirements.txt"), "").unwrap();
        let detected = ProjectType::detect(temp_dir.path());
        assert_eq!(detected, ProjectType::Python);
    }

    #[test]
    fn test_detect_go_project() {
        let temp_dir = create_test_project(ProjectType::Go);
        let detected = ProjectType::detect(temp_dir.path());
        assert_eq!(detected, ProjectType::Go);
    }

    #[test]
    fn test_detect_ruby_project() {
        let temp_dir = create_test_project(ProjectType::Ruby);
        let detected = ProjectType::detect(temp_dir.path());
        assert_eq!(detected, ProjectType::Ruby);
    }

    #[test]
    fn test_detect_java_maven_project() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("pom.xml"), "").unwrap();
        let detected = ProjectType::detect(temp_dir.path());
        assert_eq!(detected, ProjectType::Java);
    }

    #[test]
    fn test_detect_java_gradle_project() {
        let temp_dir = TempDir::new().unwrap();
        fs::write(temp_dir.path().join("build.gradle"), "").unwrap();
        let detected = ProjectType::detect(temp_dir.path());
        assert_eq!(detected, ProjectType::Java);
    }

    #[test]
    fn test_detect_elixir_project() {
        let temp_dir = create_test_project(ProjectType::Elixir);
        let detected = ProjectType::detect(temp_dir.path());
        assert_eq!(detected, ProjectType::Elixir);
    }

    #[test]
    fn test_detect_unknown_project() {
        let temp_dir = TempDir::new().unwrap();
        let detected = ProjectType::detect(temp_dir.path());
        assert_eq!(detected, ProjectType::Unknown);
    }

    #[test]
    fn test_find_project_root_nested() {
        let temp_dir = TempDir::new().unwrap();
        let root = temp_dir.path();

        // Create Cargo.toml at root
        fs::write(root.join("Cargo.toml"), "").unwrap();

        // Create nested directory
        let nested = root.join("src").join("module");
        fs::create_dir_all(&nested).unwrap();

        // Find project root from nested directory
        let (found_root, project_type) = ProjectType::find_project_root(&nested).unwrap();
        assert_eq!(found_root, root);
        assert_eq!(project_type, ProjectType::Rust);
    }

    #[test]
    fn test_find_project_root_closest_ancestor() {
        let temp_dir = TempDir::new().unwrap();
        let outer_root = temp_dir.path();

        // Create outer Rust project
        fs::write(outer_root.join("Cargo.toml"), "").unwrap();

        // Create inner Node project
        let inner_root = outer_root.join("frontend");
        fs::create_dir_all(&inner_root).unwrap();
        fs::write(inner_root.join("package.json"), "").unwrap();

        // Create nested directory in inner project
        let nested = inner_root.join("src");
        fs::create_dir_all(&nested).unwrap();

        // Should find the closest ancestor (Node project, not Rust)
        let (found_root, project_type) = ProjectType::find_project_root(&nested).unwrap();
        assert_eq!(found_root, inner_root);
        assert_eq!(project_type, ProjectType::Node);
    }

    #[test]
    fn test_route_command_rust() {
        let project = ProjectType::Rust;
        assert_eq!(
            project.route_command("test"),
            Some("cargo test".to_string())
        );
        assert_eq!(
            project.route_command("build"),
            Some("cargo build".to_string())
        );
        assert_eq!(project.route_command("run"), Some("cargo run".to_string()));
        assert_eq!(
            project.route_command("install"),
            Some("cargo install".to_string())
        );
        assert_eq!(
            project.route_command("format"),
            Some("cargo fmt".to_string())
        );
        assert_eq!(
            project.route_command("lint"),
            Some("cargo clippy".to_string())
        );
    }

    #[test]
    fn test_route_command_node() {
        let project = ProjectType::Node;
        assert_eq!(project.route_command("test"), Some("npm test".to_string()));
        assert_eq!(
            project.route_command("build"),
            Some("npm run build".to_string())
        );
        assert_eq!(
            project.route_command("install"),
            Some("npm install".to_string())
        );
    }

    #[test]
    fn test_route_command_python() {
        let project = ProjectType::Python;
        assert_eq!(project.route_command("test"), Some("pytest".to_string()));
        assert_eq!(
            project.route_command("install"),
            Some("pip install".to_string())
        );
        assert_eq!(project.route_command("format"), Some("black .".to_string()));
    }

    #[test]
    fn test_route_command_unknown() {
        let project = ProjectType::Unknown;
        assert_eq!(project.route_command("test"), None);
        assert_eq!(project.route_command("build"), None);
    }

    #[test]
    fn test_context_caching() {
        let temp_dir = create_test_project(ProjectType::Rust);
        let path = temp_dir.path();

        let mut ctx = Context::new();

        // First detection should populate cache
        let type1 = ctx.detect_project(path);
        assert_eq!(type1, ProjectType::Rust);

        // Second detection should use cache (same result)
        let type2 = ctx.detect_project(path);
        assert_eq!(type2, ProjectType::Rust);
    }

    #[test]
    #[cfg(feature = "git-builtins")]
    fn test_context_with_git() {
        let temp_dir = create_test_project(ProjectType::Rust);
        let ctx = Context::detect(temp_dir.path());

        assert_eq!(ctx.get_project_type(), &ProjectType::Rust);
        assert!(ctx.get_git_context().is_some());
    }

    #[test]
    fn test_context_route_command() {
        let temp_dir = create_test_project(ProjectType::Node);
        let ctx = Context::detect(temp_dir.path());

        assert_eq!(ctx.route_command("test"), Some("npm test".to_string()));
        assert_eq!(
            ctx.route_command("build"),
            Some("npm run build".to_string())
        );
    }

    #[test]
    fn test_context_status_string() {
        let temp_dir = create_test_project(ProjectType::Rust);
        let ctx = Context::detect(temp_dir.path());

        let status = ctx.status_string();
        assert!(status.contains("Rust"));
    }

    #[test]
    fn test_cache_clear() {
        let temp_dir = create_test_project(ProjectType::Go);
        let mut ctx = Context::new();

        ctx.detect_project(temp_dir.path());
        assert_eq!(ctx.get_project_type(), &ProjectType::Go);

        ctx.clear_cache();
        // Cache is cleared but context still holds the last detected type
        assert_eq!(ctx.get_project_type(), &ProjectType::Go);
    }

    #[test]
    fn test_all_project_types_have_markers() {
        // Ensure all project types (except Unknown) have at least one marker
        let types = [
            ProjectType::Rust,
            ProjectType::Node,
            ProjectType::Python,
            ProjectType::Go,
            ProjectType::Ruby,
            ProjectType::Java,
            ProjectType::Elixir,
        ];

        for project_type in types {
            assert!(
                !project_type.marker_files().is_empty(),
                "{:?} should have at least one marker file",
                project_type
            );
        }
    }
}
