use std::path::Path;
use std::process::{Command, Stdio};

use forge_ports::BrowserPort;

/// Hands addresses and files to the system: the macOS `open` command, or on Windows the shell's
/// own handler (`rundll32 url.dll,FileProtocolHandler`) and Explorer. Neither goes through a shell
/// command line, so an address or a path is always one argument.
pub struct SystemBrowser {
    // The program that opens things. Windows has a fixed set (see below), so it is only read elsewhere.
    #[cfg_attr(windows, allow(dead_code))]
    program: String,
}

impl SystemBrowser {
    pub fn new() -> Self {
        Self::with_program(if cfg!(windows) { "rundll32" } else { "open" })
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

// Runs the command with its output thrown away; whether it exited with success.
fn succeeded(command: &mut Command, program: &str) -> Result<bool, String> {
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .map_err(|error| format!("Could not run {program}: {error}"))
}

impl BrowserPort for SystemBrowser {
    fn open(&self, url: &str) -> Result<(), String> {
        if !is_web_address(url) {
            return Err("Only http and https addresses can be opened".to_string());
        }
        let mut command = Command::new(&self.program);
        #[cfg(windows)]
        command.arg("url.dll,FileProtocolHandler");
        if succeeded(command.arg(url), &self.program)? {
            Ok(())
        } else {
            Err(format!("{} could not open the address", self.program))
        }
    }

    fn reveal(&self, path: &Path) -> Result<(), String> {
        if !path.is_absolute() {
            return Err(format!("Only an absolute path can be shown in {}", file_manager()));
        }
        #[cfg(windows)]
        {
            // Explorer exits with 1 even when it worked, so only a failure to start it is an error.
            // Explorer only selects a path written with backslashes.
            let mut argument = std::ffi::OsString::from("/select,");
            argument.push(path.to_string_lossy().replace('/', "\\"));
            succeeded(Command::new("explorer").arg(argument), "explorer")?;
            Ok(())
        }
        #[cfg(not(windows))]
        {
            // `-R` reveals instead of opening.
            if succeeded(Command::new(&self.program).arg("-R").arg(path), &self.program)? {
                Ok(())
            } else {
                Err(format!("{} could not show the file in {}", self.program, file_manager()))
            }
        }
    }

    fn open_with(&self, path: &Path, app: Option<&'static str>) -> Result<(), String> {
        if !path.is_absolute() {
            return Err("Only an absolute path can be opened".to_string());
        }
        #[cfg(windows)]
        let (mut command, program) = match app {
            // The Visual Studio Code installer puts `code.cmd` on the PATH. Rust quotes the path for
            // a batch file, and refuses one it cannot quote (a `%` or `"` in the name): that is an error here.
            Some("Visual Studio Code") => (Command::new("code.cmd"), "code".to_string()),
            Some(other) => return Err(format!("Could not open the file with {other}")),
            None if is_program_file(path) => return Err("Forge does not run programs; open its folder instead".to_string()),
            None => {
                let mut command = Command::new(&self.program);
                command.arg("url.dll,FileProtocolHandler");
                (command, self.program.clone())
            }
        };
        #[cfg(not(windows))]
        let (mut command, program) = {
            let mut command = Command::new(&self.program);
            if let Some(app) = app {
                command.arg("-a").arg(app);
            }
            (command, self.program.clone())
        };
        // The path is one argument, never part of a command line; it is absolute, so it cannot be an option.
        if succeeded(command.arg(path), &program)? {
            Ok(())
        } else {
            Err(match app {
                Some(app) => format!("Could not open the file with {app}"),
                None => format!("{program} could not open the file"),
            })
        }
    }
}

// What the shell would run rather than show if it were handed the file.
#[cfg(windows)]
fn is_program_file(path: &Path) -> bool {
    const PROGRAMS: [&str; 14] = ["exe", "com", "bat", "cmd", "lnk", "msi", "scr", "ps1", "vbs", "vbe", "js", "jse", "wsf", "hta"];
    path.extension().and_then(|extension| extension.to_str()).is_some_and(|extension| PROGRAMS.iter().any(|program| extension.eq_ignore_ascii_case(program)))
}

fn file_manager() -> &'static str {
    if cfg!(windows) {
        "Explorer"
    } else {
        "Finder"
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

    #[cfg(unix)]
    #[test]
    fn a_web_address_is_passed_to_the_program_as_one_argument() {
        assert_eq!(SystemBrowser::with_program("true").open("http://localhost:3000/a?b=1&c=2"), Ok(()));
    }

    #[cfg(unix)]
    #[test]
    fn a_program_that_fails_or_is_missing_is_reported_as_a_message() {
        assert!(SystemBrowser::with_program("false").open("https://example.com").is_err());
        let error = SystemBrowser::with_program("forge-no-such-program").open("https://example.com").unwrap_err();
        assert!(error.contains("forge-no-such-program"), "{error}");
    }

    #[cfg(unix)]
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

    #[cfg(unix)]
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

    #[cfg(unix)]
    #[test]
    fn a_program_that_fails_or_is_missing_is_reported_by_open_with() {
        assert!(SystemBrowser::with_program("false").open_with(std::path::Path::new("/tmp/a.txt"), Some("Nope")).is_err());
        let error = SystemBrowser::with_program("forge-no-such-program").open_with(std::path::Path::new("/tmp/a.txt"), None).unwrap_err();
        assert!(error.contains("forge-no-such-program"), "{error}");
    }
}
