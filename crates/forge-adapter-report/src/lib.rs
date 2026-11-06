//! The report a person attaches when something goes wrong: Forge's logs, the computer's logs and its details in
//! one zip, with the home folder, user name, keys, tokens and e-mail addresses hidden.

mod environment;
mod mask;
mod oslogs;
mod system;
pub use system::free_space;
mod transcripts;

use std::io::Write;
use std::path::{Path, PathBuf};

use forge_ports::{LogReportPort, ReportSummary};
use time::macros::format_description;
use time::OffsetDateTime;
use zip::write::SimpleFileOptions;

pub use environment::Details;
pub use mask::Masker;

pub struct ZipLogReport {
    logs_dir: PathBuf,
    home: Option<PathBuf>,
    details: Details,
}

impl ZipLogReport {
    /// `logs_dir` is where `forge-log` writes; `home` is the person's home folder, hidden in the text.
    pub fn new(logs_dir: PathBuf, home: Option<PathBuf>) -> Self {
        Self { logs_dir, home, details: Details::default() }
    }

    /// Adds the settings file and the programs to ask for their version.
    pub fn with_details(mut self, details: Details) -> Self {
        self.details = details;
        self
    }

    /// The parts of the report: (name in the zip, masked text), and how many details were hidden.
    fn gather(&self, conversations: bool) -> (Vec<(String, String)>, usize) {
        let masker = Masker::new(self.home.as_deref());
        let mut hidden = 0;
        let mut parts: Vec<(String, String)> = Vec::new();
        let mut add = |name: String, text: String| {
            let (masked, count) = masker.mask(&text);
            hidden += count;
            parts.push((name, masked));
        };
        add("system-info.txt".into(), system::describe());
        add("environment.txt".into(), environment::describe(&self.details));
        let files = forge_log::log_files(&self.logs_dir);
        if files.is_empty() {
            add("logs/forge.log".into(), "Forge has not written a log yet.\n".into());
        }
        for path in files {
            let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            add(format!("logs/{name}"), std::fs::read(&path).map(|bytes| String::from_utf8_lossy(&bytes).into_owned()).unwrap_or_default());
        }
        for part in oslogs::collect(self.home.as_deref()) {
            add(part.name, part.text);
        }
        if conversations {
            for part in transcripts::newest(self.details.transcripts_dir.as_deref()) {
                add(part.name, part.text);
            }
        }
        let listing = parts.iter().map(|(name, _)| format!("  {name}")).collect::<Vec<_>>().join("\n");
        parts.insert(0, ("README.txt".into(), readme(&listing, hidden, conversations)));
        (parts, hidden)
    }
}

fn readme(listing: &str, hidden: usize, conversations: bool) -> String {
    let conv = if conversations { "the newest conversations are included, with what was said in them" } else { "not included" };
    format!(
        "Forge problem report\n\n\
         Made on {}. This file holds:\n{listing}\n\n\
         Conversations: {conv}.\n\
         Hidden before saving ({hidden} places): the home folder and user name, API keys, tokens, passwords in URLs and e-mail addresses.\n\
         Left in: project folder names, file names and the times of events. Read the files before you send them.\n",
        OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339).unwrap_or_default()
    )
}

impl LogReportPort for ZipLogReport {
    fn suggested_name(&self) -> String {
        let now = OffsetDateTime::now_local().unwrap_or_else(|_| OffsetDateTime::now_utc());
        let stamp = now.format(format_description!("[year][month][day]-[hour][minute]")).unwrap_or_default();
        format!("forge-report-{stamp}.zip")
    }

    fn logs_dir(&self) -> PathBuf {
        self.logs_dir.clone()
    }

    fn write_report(&self, dest: &Path, conversations: bool) -> Result<ReportSummary, String> {
        forge_log::info("report", &format!("saving a problem report (conversations: {conversations})"));
        let (parts, hidden) = self.gather(conversations);
        write_zip(dest, &parts)?;
        Ok(ReportSummary { entries: parts.into_iter().map(|(name, _)| name).collect(), hidden })
    }
}

fn write_zip(dest: &Path, parts: &[(String, String)]) -> Result<(), String> {
    let file = std::fs::File::create(dest).map_err(|err| err.to_string())?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
    for (name, text) in parts {
        zip.start_file(name, options).map_err(|err| err.to_string())?;
        zip.write_all(text.as_bytes()).map_err(|err| err.to_string())?;
    }
    zip.finish().map_err(|err| err.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Read;

    use super::*;

    fn report_in(dir: &Path) -> ZipLogReport {
        let logs = dir.join("logs");
        std::fs::create_dir_all(&logs).unwrap();
        std::fs::write(logs.join("forge.log"), "2026-11-02T00:00:00Z INFO app: opened /Users/jamesyeo/Developer/x key sk-ant-api03-SECRETSECRET1\n").unwrap();
        ZipLogReport::new(logs, Some(PathBuf::from("/Users/jamesyeo")))
    }

    fn read_entry(zip_path: &Path, name: &str) -> String {
        let mut archive = zip::ZipArchive::new(std::fs::File::open(zip_path).unwrap()).unwrap();
        let mut text = String::new();
        archive.by_name(name).unwrap().read_to_string(&mut text).unwrap();
        text
    }

    #[test]
    fn the_zip_holds_a_readme_the_system_info_and_the_masked_log() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("out.zip");
        let summary = report_in(dir.path()).write_report(&dest, false).unwrap();
        for name in ["README.txt", "system-info.txt", "logs/forge.log"] {
            assert!(summary.entries.iter().any(|entry| entry == name), "{:?}", summary.entries);
        }
        let log = read_entry(&dest, "logs/forge.log");
        assert!(log.contains("opened ~/Developer/x"), "{log}");
        assert!(!log.contains("SECRETSECRET") && !log.contains("jamesyeo"), "{log}");
        assert!(summary.hidden >= 2);
    }

    #[test]
    fn the_readme_lists_what_is_inside() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("out.zip");
        report_in(dir.path()).write_report(&dest, false).unwrap();
        let readme = read_entry(&dest, "README.txt");
        assert!(readme.contains("logs/forge.log") && readme.contains("Hidden before saving"), "{readme}");
    }

    #[test]
    fn with_no_log_yet_the_report_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let dest = dir.path().join("out.zip");
        ZipLogReport::new(dir.path().join("none"), None).write_report(&dest, false).unwrap();
        assert!(read_entry(&dest, "logs/forge.log").contains("has not written a log yet"));
    }

    #[test]
    fn an_unwritable_destination_is_an_error_text() {
        let dir = tempfile::tempdir().unwrap();
        let result = report_in(dir.path()).write_report(&dir.path().join("missing/out.zip"), false);
        assert!(result.is_err());
    }

    #[test]
    fn the_suggested_name_is_a_zip_with_the_time() {
        let name = report_in(tempfile::tempdir().unwrap().path()).suggested_name();
        assert!(name.starts_with("forge-report-") && name.ends_with(".zip"), "{name}");
    }
}
