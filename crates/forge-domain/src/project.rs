use std::path::PathBuf;

/// One project directory that has at least one Claude Code transcript.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectSummary {
    /// The real project path, recovered from a transcript's `cwd` (never the escaped directory name).
    pub path: PathBuf,
    /// RFC 3339 modification time of the project's newest transcript.
    pub last_active: String,
    pub session_count: usize,
    /// Whether `path` is a directory on disk right now (a session cannot be started in a missing one).
    pub exists: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(path: &str) -> ProjectSummary {
        ProjectSummary {
            path: PathBuf::from(path),
            last_active: "2026-09-29T00:00:00Z".to_string(),
            session_count: 3,
            exists: true,
        }
    }

    #[test]
    fn project_summary_equality_is_field_based() {
        assert_eq!(summary("/work/a"), summary("/work/a"));
        assert_ne!(summary("/work/a"), summary("/work/b"));
    }

    #[test]
    fn project_summary_clone_keeps_every_field() {
        let original = summary("/work/a");
        assert_eq!(original.clone(), original);
    }
}
