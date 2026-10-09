//! What the head of a workflow script says about the workflow. A script is JavaScript; this reads
//! only its `meta` block (`name`, `description`, the `title` of each phase) and never runs or
//! parses the rest.

/// The facts a script's `meta` block gives.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkflowMeta {
    pub name: String,
    pub description: Option<String>,
    /// The titles of the phases, in the order the script lists them.
    pub phases: Vec<String>,
}

/// The `meta` block of a script, or `None` when it has no readable `name`. The block comes first and
/// ends at the first line that starts with `}`, so a string in a later agent prompt is never taken
/// for the workflow's name.
pub fn workflow_meta(script: &str) -> Option<WorkflowMeta> {
    let meta = script.split("\n}").next().unwrap_or(script);
    let name = literals(meta, "name").into_iter().next()?;
    let description = literals(meta, "description").into_iter().next();
    let phases = match meta.find("phases") {
        Some(at) => {
            let list = &meta[at..];
            let list = &list[..list.find(']').unwrap_or(list.len())];
            literals(list, "title")
        }
        None => Vec::new(),
    };
    Some(WorkflowMeta { name, description, phases })
}

/// Every non-empty string literal that follows `key:`, where `key` stands alone as a word. Reads
/// `'...'`, `"..."` and backtick strings with backslash escapes; a value that is not a plain string
/// is skipped.
fn literals(text: &str, key: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut from = 0;
    while let Some(at) = text[from..].find(key) {
        let start = from + at;
        let end = start + key.len();
        from = end;
        if text[..start].chars().next_back().is_some_and(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        let Some(rest) = text[end..].trim_start().strip_prefix(':') else { continue };
        let mut chars = rest.trim_start().chars();
        let Some(quote) = chars.next().filter(|c| matches!(c, '\'' | '"' | '`')) else { continue };
        let mut value = String::new();
        let mut closed = false;
        while let Some(c) = chars.next() {
            match c {
                '\\' => match chars.next() {
                    Some('n') => value.push('\n'),
                    Some('t') => value.push('\t'),
                    Some(other) => value.push(other),
                    None => break,
                },
                c if c == quote => {
                    closed = true;
                    break;
                }
                c => value.push(c),
            }
        }
        if closed && !value.is_empty() {
            found.push(value);
        }
    }
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    const SCRIPT: &str = "export const meta = {\n  name: 'sample-risk-review',\n  description: \"Three agents review; a fourth merges\",\n  phases: [{ title: 'Analyze' }, { title: \"Merge\" }],\n}\nconst x = await agent('name: not this one', { phase: 'Analyze', title: 'nor this' })";

    #[test]
    fn the_meta_block_gives_the_name_the_description_and_the_phase_titles() {
        assert_eq!(
            workflow_meta(SCRIPT),
            Some(WorkflowMeta {
                name: "sample-risk-review".to_string(),
                description: Some("Three agents review; a fourth merges".to_string()),
                phases: vec!["Analyze".to_string(), "Merge".to_string()],
            })
        );
    }

    #[test]
    fn a_script_without_phases_or_a_description_still_has_a_name() {
        let meta = workflow_meta("export const meta = { name: 'quick' }").unwrap();
        assert_eq!(meta, WorkflowMeta { name: "quick".to_string(), description: None, phases: Vec::new() });
    }

    #[test]
    fn a_script_without_a_readable_name_has_no_meta() {
        for script in ["", "const nothing = 1", "export const meta = { name: someVariable }", "export const meta = { name: '' }"] {
            assert_eq!(workflow_meta(script), None, "{script}");
        }
    }

    #[test]
    fn a_key_inside_another_word_is_not_the_key() {
        let meta = workflow_meta("export const meta = { rename: 'no', name: 'yes', subtitle: 'no', phases: [{ subtitle: 'no', title: 'Yes' }] }").unwrap();
        assert_eq!(meta.name, "yes");
        assert_eq!(meta.phases, vec!["Yes".to_string()]);
    }

    #[test]
    fn escaped_quotes_and_newlines_read_whole_and_an_unclosed_string_is_dropped() {
        assert_eq!(workflow_meta("export const meta = { name: 'it\\'s fine' }").unwrap().name, "it's fine");
        assert_eq!(workflow_meta("export const meta = { name: 'a', description: 'line one\\nline two' }").unwrap().description.as_deref(), Some("line one\nline two"));
        assert_eq!(workflow_meta("export const meta = { name: 'a', description: 'never closed }").unwrap().description, None);
    }
}
