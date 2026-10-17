//! Which of a changed file's three groups it belongs to, for the Changes panel's `Separate test,
//! build and generated files`. The rules are the ones seen in the desktop app (observation round
//! 20b): `build/`, `out/`, `target/` and `*.d.ts` were seen staying in the normal list.

/// The group a changed file is listed in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileClass {
    Normal,
    Test,
    Generated,
}

const TEST_FOLDERS: [&str; 5] = ["tests", "test", "spec", "specs", "__tests__"];
const GENERATED_FOLDERS: [&str; 2] = ["dist", ".next"];
const GENERATED_ENDINGS: [&str; 5] = [".min.js", ".min.css", ".js.map", ".css.map", ".lock"];

fn is_test_name(name: &str) -> bool {
    if name.contains(".test.") || name.contains(".spec.") {
        return true;
    }
    // `foo_test.go` and `test_x.py` need an extension; a bare `latest_test_data` is not a test.
    match name.rsplit_once('.') {
        Some((stem, _)) => stem.ends_with("_test") || name.starts_with("test_"),
        None => false,
    }
}

fn is_generated_name(name: &str) -> bool {
    name.contains(".generated.")
        || name == "package-lock.json"
        || name == "pnpm-lock.yaml"
        || GENERATED_ENDINGS.iter().any(|ending| name.ends_with(ending))
}

/// The group of `path` (a repository path with `/`). A test file inside a generated folder is a
/// test file.
pub fn file_class(path: &str) -> FileClass {
    let mut parts: Vec<&str> = path.split('/').collect();
    let name = parts.pop().unwrap_or("");
    if parts.iter().any(|folder| TEST_FOLDERS.contains(folder)) || is_test_name(name) {
        return FileClass::Test;
    }
    if parts.iter().any(|folder| GENERATED_FOLDERS.contains(folder)) || is_generated_name(name) {
        return FileClass::Generated;
    }
    FileClass::Normal
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_paths_seen_in_the_desktop_app_are_test_files() {
        for path in ["__tests__/b.js", "spec/a.rb", "src/x.test.ts", "tests/test_x.py", "bar.spec.ts", "foo_test.go", "test_root.py"] {
            assert_eq!(file_class(path), FileClass::Test, "{path}");
        }
    }

    #[test]
    fn the_paths_seen_in_the_desktop_app_are_generated_files() {
        for path in [".next/n.js", "dist/bundle.js", "api.generated.ts", "Cargo.lock", "package-lock.json", "pnpm-lock.yaml", "x.js.map", "x.min.js", "yarn.lock"] {
            assert_eq!(file_class(path), FileClass::Generated, "{path}");
        }
    }

    #[test]
    fn the_paths_seen_staying_in_the_normal_list_are_normal() {
        for path in ["build/out.txt", "build/app.js", "out/o.js", "target/t.rs", "bundle.d.ts", "main.py", "a.txt", "src/c.txt"] {
            assert_eq!(file_class(path), FileClass::Normal, "{path}");
        }
    }

    #[test]
    fn the_follow_the_pattern_cases_are_sorted_too() {
        assert_eq!(file_class("test/Main.java"), FileClass::Test);
        assert_eq!(file_class("specs/a.rb"), FileClass::Test);
        assert_eq!(file_class("styles/app.min.css"), FileClass::Generated);
        assert_eq!(file_class("styles/app.css.map"), FileClass::Generated);
    }

    #[test]
    fn a_folder_anywhere_in_the_path_counts() {
        assert_eq!(file_class("packages/web/tests/unit/a.rs"), FileClass::Test);
        assert_eq!(file_class("apps/site/dist/assets/a.png"), FileClass::Generated);
    }

    #[test]
    fn look_alikes_stay_normal() {
        // A file called `tests` is not a folder; a longer word is not the keyword.
        for path in ["tests", "src/tests", "contest_x.py", "latest_test_data", "testing/a.py", "mydist/a.js", "Tests/a.rs", "notes.test", "x.lockfile"] {
            assert_eq!(file_class(path), FileClass::Normal, "{path}");
        }
    }

    #[test]
    fn a_test_file_inside_a_generated_folder_is_a_test_file() {
        assert_eq!(file_class("dist/a.test.js"), FileClass::Test);
    }

    #[test]
    fn korean_names_are_sorted_by_the_same_rules() {
        assert_eq!(file_class("테스트/한글.txt"), FileClass::Normal);
        assert_eq!(file_class("tests/한글.txt"), FileClass::Test);
    }
}
