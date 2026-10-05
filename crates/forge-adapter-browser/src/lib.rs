use std::path::Path;
use std::process::{Command, Stdio};

use forge_ports::BrowserPort;

/// Opens addresses with the macOS `open` command, which hands them to the default browser.
pub struct SystemBrowser {
    program: String,
}

impl SystemBrowser {
    pub fn new() -> Self {
        Self::with_program("open")
    }

    pub fn with_program(program: impl Into<String>) -> Self {
        Self { program: program.into() }
    }
}

impl Default for SystemBrowser {
    fn default() -> Self {
        Self::new()
    }
}

fn is_web_address(url: &str) -> bool {
    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"));
    matches!(rest, Some(rest) if !rest.is_empty() && !url.chars().any(char::is_whitespace))
}

impl BrowserPort for SystemBrowser {
    fn open(&self, url: &str) -> Result<(), String> {
        if !is_web_address(url) {
            return Err("Only http and https addresses can be opened".to_string());
        }
        // The address is one argument, never part of a shell command line.
        let status = Command::new(&self.program)
            .arg(url)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|error| format!("Could not run {}: {error}", self.program))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("{} could not open the address", self.program))
        }
    }

    fn reveal(&self, path: &Path) -> Result<(), String> {
        if !path.is_absolute() {
            return Err("Only an absolute path can be shown in Finder".to_string());
        }
        // `-R` reveals instead of opening; the path is one argument, never part of a command line.
        let status = Command::new(&self.program)
            .arg("-R")
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|error| format!("Could not run {}: {error}", self.program))?;
        if status.success() {
            Ok(())
        } else {
            Err(format!("{} could not show the file in Finder", self.program))
        }
    }

    fn open_with(&self, path: &Path, app: Option<&'static str>) -> Result<(), String> {
        if !path.is_absolute() {
            return Err("Only an absolute path can be opened".to_string());
        }
        let mut command = Command::new(&self.program);
        if let Some(app) = app {
            command.arg("-a").arg(app);
        }
        // The path is one argument, never part of a command line; it starts with `/`, so it cannot be an option.
        let status = command
            .arg(path)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map_err(|error| format!("Could not run {}: {error}", self.program))?;
        if status.success() {
            Ok(())
        } else {
            Err(match app {
                Some(app) => format!("Could not open the file with {app}"),
                None => format!("{} could not open the file", self.program),
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use forge_ports::BrowserPort;

    use super::*;

    #[test]
    fn only_web_addresses_are_opened() {
        // The program does not exist, so an error that mentions http can only come from the check
        // that runs before any program starts.
        let browser = SystemBrowser::with_program("forge-no-such-program");
        for url in ["file:///etc/passwd", "javascript:alert(1)", "ftp://host/x", "localhost:3000", "", "http://", "https://a b"] {
            let error = browser.open(url).unwrap_err();
            assert!(error.contains("http"), "{url}: {error}");
        }
    }

    #[test]
    fn a_web_address_is_passed_to_the_program_as_one_argument() {
        assert_eq!(SystemBrowser::with_program("true").open("http://localhost:3000/a?b=1&c=2"), Ok(()));
    }

    #[test]
    fn a_program_that_fails_or_is_missing_is_reported_as_a_message() {
        assert!(SystemBrowser::with_program("false").open("https://example.com").is_err());
        let error = SystemBrowser::with_program("forge-no-such-program").open("https://example.com").unwrap_err();
        assert!(error.contains("forge-no-such-program"), "{error}");
    }

    #[test]
    fn revealing_with_a_missing_program_says_it_could_not_run() {
        let browser = SystemBrowser::with_program("/nonexistent/forge-open");
        let error = browser.reveal(std::path::Path::new("/tmp")).unwrap_err();
        assert!(error.contains("Could not run"), "{error}");
    }

    #[test]
    fn revealing_a_relative_path_is_refused_before_any_program_runs() {
        let browser = SystemBrowser::with_program("/nonexistent/forge-open");
        let error = browser.reveal(std::path::Path::new("relative/a.txt")).unwrap_err();
        assert!(error.contains("absolute"), "{error}");
    }

    #[test]
    fn opening_a_file_passes_the_path_as_one_argument() {
        assert_eq!(SystemBrowser::with_program("true").open_with(std::path::Path::new("/tmp/a b.txt"), None), Ok(()));
        assert_eq!(SystemBrowser::with_program("true").open_with(std::path::Path::new("/tmp/a.txt"), Some("Visual Studio Code")), Ok(()));
    }

    #[test]
    fn a_relative_path_or_an_option_looking_path_is_refused_before_any_program_runs() {
        let browser = SystemBrowser::with_program("forge-no-such-program");
        for path in ["a.txt", "-a", "--help", ""] {
            let error = browser.open_with(std::path::Path::new(path), None).unwrap_err();
            assert!(!error.contains("forge-no-such-program"), "{path}: {error}");
        }
    }

    #[test]
    fn a_program_that_fails_or_is_missing_is_reported_by_open_with() {
        assert!(SystemBrowser::with_program("false").open_with(std::path::Path::new("/tmp/a.txt"), Some("Nope")).is_err());
        let error = SystemBrowser::with_program("forge-no-such-program").open_with(std::path::Path::new("/tmp/a.txt"), None).unwrap_err();
        assert!(error.contains("forge-no-such-program"), "{error}");
    }
}
