use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use forge_domain::SkillDefinition;
use forge_ports::{SkillRepositoryError, SkillRepositoryPort};

pub struct ClaudeSkillAdapter {
    claude_home: PathBuf,
}

impl ClaudeSkillAdapter {
    pub fn new(claude_home: PathBuf) -> Self {
        Self { claude_home }
    }
}

fn unquote(value: &str) -> &str {
    for quote in ['"', '\''] {
        if let Some(inner) = value
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
        {
            return inner;
        }
    }
    value
}

fn is_block_scalar_marker(value: &str) -> bool {
    let mut chars = value.chars();
    matches!(chars.next(), Some('>' | '|')) && chars.all(|c| matches!(c, '+' | '-' | '0'..='9'))
}

pub(crate) fn parse_frontmatter(contents: &str) -> Option<HashMap<String, String>> {
    let contents = contents.strip_prefix('\u{feff}').unwrap_or(contents);
    let mut lines = contents.lines();
    if lines.next()?.trim_end() != "---" {
        return None;
    }
    let mut fields = HashMap::new();
    // A key whose value is a YAML block scalar (`>`, `|-`, ...) collects the indented lines that
    // follow it, folded into one line.
    let mut block: Option<(String, Vec<&str>)> = None;
    for line in lines {
        let closing = line.trim_end() == "---";
        let continues_block =
            !closing && (line.trim().is_empty() || line.starts_with(char::is_whitespace));
        if continues_block {
            if let Some((_, parts)) = block.as_mut() {
                if !line.trim().is_empty() {
                    parts.push(line.trim());
                }
            }
            continue;
        }
        if let Some((key, parts)) = block.take() {
            fields.insert(key, parts.join(" "));
        }
        if closing {
            return Some(fields);
        }
        if let Some((key, value)) = line.split_once(':') {
            let value = value.trim();
            if is_block_scalar_marker(value) {
                block = Some((key.trim().to_string(), Vec::new()));
            } else {
                fields.insert(key.trim().to_string(), unquote(value).to_string());
            }
        }
    }
    None
}

pub(crate) async fn read_skill(path: &Path) -> Option<SkillDefinition> {
    let contents = tokio::fs::read_to_string(path).await.ok()?;
    let frontmatter = parse_frontmatter(&contents)?;
    let name = frontmatter.get("name").filter(|n| !n.is_empty())?.clone();
    let description = frontmatter
        .get("description")
        .filter(|d| !d.is_empty())
        .cloned();
    Some(SkillDefinition {
        name,
        path: path.parent()?.to_path_buf(),
        description,
    })
}

pub(crate) async fn collect_skills(
    root: &Path,
    visited: &mut HashSet<PathBuf>,
    skills: &mut Vec<SkillDefinition>,
) -> Result<(), SkillRepositoryError> {
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(canonical) = tokio::fs::canonicalize(&dir).await.map(|path| dunce::simplified(&path).to_path_buf()) else {
            continue;
        };
        if !visited.insert(canonical) {
            continue;
        }
        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(entries) => entries,
            Err(e) if dir == root && e.kind() != std::io::ErrorKind::NotFound => {
                return Err(SkillRepositoryError::ListFailed(format!(
                    "{}: {}",
                    dir.display(),
                    e
                )));
            }
            Err(_) => continue,
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            let Ok(metadata) = tokio::fs::metadata(&path).await else {
                continue;
            };
            if metadata.is_dir() {
                stack.push(path);
            } else if metadata.is_file() && entry.file_name() == "SKILL.md" {
                if let Some(skill) = read_skill(&path).await {
                    skills.push(skill);
                }
            }
        }
    }
    Ok(())
}

#[async_trait]
impl SkillRepositoryPort for ClaudeSkillAdapter {
    async fn list_skills(
        &self,
        project_path: &Path,
    ) -> Result<Vec<SkillDefinition>, SkillRepositoryError> {
        let roots = [
            self.claude_home.join("skills"),
            project_path.join(".claude").join("skills"),
        ];
        let mut visited = HashSet::new();
        let mut skills = Vec::new();
        for root in &roots {
            collect_skills(root, &mut visited, &mut skills).await?;
        }
        skills.sort_by(|a, b| a.name.cmp(&b.name).then_with(|| a.path.cmp(&b.path)));
        Ok(skills)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_skill(root: &Path, relative_dir: &str, contents: &str) {
        let dir = root.join(relative_dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("SKILL.md"), contents).unwrap();
    }

    fn skill(name: &str, description: &str) -> String {
        format!("---\nname: {name}\ndescription: {description}\n---\n\n# Body\n")
    }

    fn names(skills: &[SkillDefinition]) -> Vec<&str> {
        skills.iter().map(|s| s.name.as_str()).collect()
    }

    async fn list(home: &Path, project: &Path) -> Vec<SkillDefinition> {
        ClaudeSkillAdapter::new(home.to_path_buf())
            .list_skills(project)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn list_skills_finds_a_skill_and_reads_name_description_and_directory_path() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        write_skill(
            &home.path().join("skills"),
            "algorithmic-art",
            "---\nname: algorithmic-art\ndescription: Creating art\nlicense: Complete terms in LICENSE.txt\n---\n\nBody text\n",
        );

        let skills = list(home.path(), project.path()).await;

        assert_eq!(
            skills,
            vec![SkillDefinition {
                name: "algorithmic-art".to_string(),
                path: home.path().join("skills").join("algorithmic-art"),
                description: Some("Creating art".to_string()),
            }]
        );
    }

    #[tokio::test]
    async fn list_skills_finds_skills_nested_three_levels_deep() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let root = home.path().join("skills");
        write_skill(&root, "document-skills/xlsx", &skill("xlsx", "spreadsheets"));
        write_skill(&root, "synced/uuid-1/deep-research", &skill("deep-research", "research"));
        write_skill(&root, "top-level", &skill("top-level", "flat"));

        let skills = list(home.path(), project.path()).await;

        assert_eq!(names(&skills), vec!["deep-research", "top-level", "xlsx"]);
    }

    #[tokio::test]
    async fn list_skills_reads_global_and_project_local_skills_sorted_by_name() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        write_skill(&home.path().join("skills"), "zeta", &skill("zeta", "global"));
        write_skill(
            &project.path().join(".claude").join("skills"),
            "alpha",
            &skill("alpha", "project-local"),
        );

        let skills = list(home.path(), project.path()).await;

        assert_eq!(names(&skills), vec!["alpha", "zeta"]);
        assert_eq!(
            skills[0].path,
            project.path().join(".claude").join("skills").join("alpha")
        );
    }

    #[tokio::test]
    async fn list_skills_skips_a_skill_without_a_name_but_keeps_the_rest() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let root = home.path().join("skills");
        write_skill(&root, "nameless", "---\ndescription: no name here\n---\n");
        write_skill(&root, "ok", &skill("ok", "fine"));

        let skills = list(home.path(), project.path()).await;

        assert_eq!(names(&skills), vec!["ok"]);
    }

    #[tokio::test]
    async fn list_skills_keeps_a_skill_without_a_description_with_none() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        write_skill(&home.path().join("skills"), "bare", "---\nname: bare\n---\n");

        let skills = list(home.path(), project.path()).await;

        assert_eq!(names(&skills), vec!["bare"]);
        assert_eq!(skills[0].description, None);
    }

    #[tokio::test]
    async fn list_skills_skips_files_without_frontmatter_or_with_unterminated_frontmatter() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let root = home.path().join("skills");
        write_skill(&root, "no-frontmatter", "# Just a heading\n");
        write_skill(&root, "unterminated", "---\nname: unterminated\ndescription: never closed\n");
        write_skill(&root, "ok", &skill("ok", "fine"));

        let skills = list(home.path(), project.path()).await;

        assert_eq!(names(&skills), vec!["ok"]);
    }

    #[tokio::test]
    async fn list_skills_handles_crlf_bom_and_quoted_values() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let root = home.path().join("skills");
        write_skill(
            &root,
            "crlf",
            "\u{feff}---\r\nname: crlf-skill\r\ndescription: Windows authored\r\n---\r\nBody\r\n",
        );
        write_skill(
            &root,
            "quoted",
            "---\nname: 'quoted-skill'\ndescription: \"Use when: the user asks\"\n---\n",
        );

        let skills = list(home.path(), project.path()).await;

        assert_eq!(names(&skills), vec!["crlf-skill", "quoted-skill"]);
        assert_eq!(skills[0].description.as_deref(), Some("Windows authored"));
        assert_eq!(skills[1].description.as_deref(), Some("Use when: the user asks"));
    }

    #[tokio::test]
    async fn list_skills_keeps_indented_block_scalar_lines_out_of_the_top_level_keys() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        write_skill(
            &home.path().join("skills"),
            "folded",
            "---\nname: real-name\ndescription: >-\n  name: fake-name\n  continued text\n---\n",
        );

        let skills = list(home.path(), project.path()).await;

        assert_eq!(names(&skills), vec!["real-name"]);
        assert_eq!(skills[0].description.as_deref(), Some("name: fake-name continued text"));
    }

    #[tokio::test]
    async fn list_skills_folds_a_block_scalar_description_into_one_line() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        write_skill(
            &home.path().join("skills"),
            "folded",
            "---\nname: folded\ndescription: >\n  First line of the description\n  and its continuation.\nlicense: MIT\n---\n",
        );

        let skills = list(home.path(), project.path()).await;

        assert_eq!(names(&skills), vec!["folded"]);
        assert_eq!(
            skills[0].description.as_deref(),
            Some("First line of the description and its continuation.")
        );
    }

    #[tokio::test]
    async fn list_skills_folds_a_literal_block_scalar_that_ends_at_the_closing_delimiter() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        write_skill(
            &home.path().join("skills"),
            "literal",
            "---\nname: literal\ndescription: |-\n  Line one\n\n  Line two\n---\n",
        );

        let skills = list(home.path(), project.path()).await;

        assert_eq!(skills[0].description.as_deref(), Some("Line one Line two"));
    }

    #[tokio::test]
    async fn list_skills_returns_empty_when_no_skill_directories_exist() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();

        let skills = list(home.path(), project.path()).await;

        assert!(skills.is_empty());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn list_skills_follows_a_symlinked_skill_directory() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let checkout = tempfile::tempdir().unwrap();
        write_skill(checkout.path(), "my-skill", &skill("my-skill", "from a checkout"));
        let skills_root = home.path().join("skills");
        std::fs::create_dir_all(&skills_root).unwrap();
        std::os::unix::fs::symlink(checkout.path().join("my-skill"), skills_root.join("linked")).unwrap();

        let skills = list(home.path(), project.path()).await;

        assert_eq!(names(&skills), vec!["my-skill"]);
        assert_eq!(skills[0].path, skills_root.join("linked"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn list_skills_terminates_on_a_symlink_loop() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        let skills_root = home.path().join("skills");
        write_skill(&skills_root, "a", &skill("looper", "d"));
        std::os::unix::fs::symlink(&skills_root, skills_root.join("a").join("back-to-root")).unwrap();

        let skills = list(home.path(), project.path()).await;

        assert_eq!(names(&skills), vec!["looper"]);
    }

    #[tokio::test]
    async fn list_skills_does_not_duplicate_when_the_project_root_is_the_claude_home_parent() {
        let base = tempfile::tempdir().unwrap();
        let claude_home = base.path().join(".claude");
        write_skill(&claude_home.join("skills"), "shared", &skill("shared", "d"));

        let skills = ClaudeSkillAdapter::new(claude_home)
            .list_skills(base.path())
            .await
            .unwrap();

        assert_eq!(names(&skills), vec!["shared"]);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn list_skills_fails_when_the_skills_root_is_not_a_directory() {
        let home = tempfile::tempdir().unwrap();
        let project = tempfile::tempdir().unwrap();
        std::fs::write(home.path().join("skills"), "i am a file").unwrap();

        let result = ClaudeSkillAdapter::new(home.path().to_path_buf())
            .list_skills(project.path())
            .await;

        assert!(matches!(result, Err(SkillRepositoryError::ListFailed(_))));
    }
}
