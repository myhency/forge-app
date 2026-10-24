use std::path::Path;

/// Where a path asked to open belongs: in the session folder (relative to it) or outside.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OpenPlace {
    Inside(String),
    Outside,
}

pub fn open_place(folder: &Path, absolute_path: &str) -> OpenPlace {
    match forge_entities::relative_to_folder(folder, absolute_path) {
        Some(relative) => OpenPlace::Inside(relative),
        None => OpenPlace::Outside,
    }
}

/// The paths for which `exists` holds, in their order.
pub fn existing_outputs(paths: Vec<String>, exists: impl Fn(&str) -> bool) -> Vec<String> {
    paths.into_iter().filter(|path| exists(path)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_path_outside_the_folder_sets_the_outside_file_not_a_notice() {
        assert_eq!(open_place(Path::new("/w/p"), "/Users/me/report.docx"), OpenPlace::Outside);
    }

    #[test]
    fn a_path_inside_the_folder_still_opens_in_the_files_panel() {
        assert_eq!(open_place(Path::new("/w/p"), "/w/p/src/a.rs"), OpenPlace::Inside("src/a.rs".to_string()));
    }

    #[test]
    fn outputs_drop_paths_that_do_not_exist() {
        let paths = vec!["/a/one.xlsx".to_string(), "/a/gone.xlsx".to_string(), "/a/two.docx".to_string()];
        let kept = existing_outputs(paths, |path| !path.contains("gone"));
        assert_eq!(kept, ["/a/one.xlsx", "/a/two.docx"]);
    }
}
