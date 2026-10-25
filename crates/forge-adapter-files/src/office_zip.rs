//! Guards shared by the Office readers, which all open a zip of XML parts. A hostile file is
//! refused here before any parser sees it (in 57A calamine aborted the whole process on one).

use std::io::{Cursor, Read, Seek, Write};

use forge_ports::FilesError;

// A compound file starts with these bytes; Office keeps an encrypted (password-protected) file in
// one instead of a zip.
const COMPOUND_MAGIC: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];

/// No part of an Office file may unpack to more than this, so a small zip cannot fill the memory.
pub(crate) const PART_BYTES: u64 = 100 * 1024 * 1024;
// A real Office file has a few dozen parts; far more is a file made to keep the reader busy.
const MAX_PARTS: usize = 10_000;

/// True when the zip has more parts than any real Office file, or a part whose declared unpacked
/// size is over `part_limit`. The declared size can lie, so a read must still be bounded by
/// `copy_bounded`.
pub(crate) fn declares_too_much<R: Read + Seek>(archive: &mut zip::ZipArchive<R>, part_limit: u64) -> bool {
    if archive.len() > MAX_PARTS {
        return true;
    }
    (0..archive.len()).any(|index| archive.by_index_raw(index).is_ok_and(|part| part.size() > part_limit))
}

/// Copies at most `limit + 1` bytes of `part` into `out` and gives how many it copied, so a count
/// over `limit` means the part really unpacks to more than `limit`. Nothing is sized by what the
/// part declares.
pub(crate) fn copy_bounded(part: impl Read, limit: u64, out: &mut impl Write) -> std::io::Result<u64> {
    std::io::copy(&mut part.take(limit + 1), out)
}

/// The Office file's zip. A compound file fails with `locked` (it is how Office stores a
/// password-protected file); anything else that is not a zip fails with the zip library's text.
pub(crate) fn open<'a>(bytes: &'a [u8], locked: &str) -> Result<zip::ZipArchive<Cursor<&'a [u8]>>, FilesError> {
    if bytes.starts_with(&COMPOUND_MAGIC) {
        return Err(FilesError::Failed(locked.to_string()));
    }
    zip::ZipArchive::new(Cursor::new(bytes)).map_err(|error| FilesError::Failed(error.to_string()))
}

/// The bytes of the part `name` (matched in any case, as Office part names are), or `None` when
/// the zip has no such part; `too_big` when it unpacks to more than `limit`.
pub(crate) fn read_part<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
    name: &str,
    limit: u64,
    too_big: impl Fn() -> FilesError,
) -> Result<Option<Vec<u8>>, FilesError> {
    let Some(index) = archive.index_for_name(name).or_else(|| archive.file_names().position(|found| found.is_ok_and(|found| found.eq_ignore_ascii_case(name)))) else {
        return Ok(None);
    };
    let part = archive.by_index(index).map_err(|error| FilesError::Failed(error.to_string()))?;
    let mut bytes = Vec::new();
    let unpacked = copy_bounded(part, limit, &mut bytes).map_err(|error| FilesError::Failed(error.to_string()))?;
    if unpacked > limit {
        return Err(too_big());
    }
    Ok(Some(bytes))
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use super::*;

    fn zip_of(parts: &[(&str, &[u8])]) -> zip::ZipArchive<Cursor<Vec<u8>>> {
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default().compression_method(zip::CompressionMethod::Deflated);
        for (name, bytes) in parts {
            zip.start_file(*name, options).unwrap();
            zip.write_all(bytes).unwrap();
        }
        zip::ZipArchive::new(Cursor::new(zip.finish().unwrap().into_inner())).unwrap()
    }

    #[test]
    fn a_part_declaring_more_than_the_limit_is_too_much() {
        let mut archive = zip_of(&[("a.xml", &[b'x'; 200])]);
        assert!(!declares_too_much(&mut archive, 200));
        assert!(declares_too_much(&mut archive, 199));
    }

    #[test]
    fn a_zip_of_more_than_ten_thousand_parts_is_too_much() {
        let names: Vec<String> = (0..=MAX_PARTS).map(|index| format!("p{index}")).collect();
        let parts: Vec<(&str, &[u8])> = names.iter().map(|name| (name.as_str(), &[][..])).collect();
        assert!(declares_too_much(&mut zip_of(&parts), PART_BYTES));
        assert!(!declares_too_much(&mut zip_of(&parts[..MAX_PARTS]), PART_BYTES));
    }

    #[test]
    fn a_bounded_copy_stops_one_byte_past_the_limit() {
        let mut out = Vec::new();
        assert_eq!(copy_bounded(&[b'x'; 50][..], 10, &mut out).unwrap(), 11);
        assert_eq!(out.len(), 11);
        assert_eq!(copy_bounded(&[b'x'; 5][..], 10, &mut std::io::sink()).unwrap(), 5);
    }

    #[test]
    fn a_part_is_found_in_any_case_and_its_read_is_bounded_whatever_it_declares() {
        let too_big = || FilesError::Failed("big".to_string());
        let mut archive = zip_of(&[("Word/Document.xml", &[b'x'; 200])]);
        assert_eq!(read_part(&mut archive, "word/document.xml", 200, too_big).unwrap().map(|bytes| bytes.len()), Some(200));
        assert_eq!(read_part(&mut archive, "word/document.xml", 199, too_big), Err(too_big()));
        assert_eq!(read_part(&mut archive, "word/missing.xml", 200, too_big), Ok(None));
    }

    #[test]
    fn a_compound_file_fails_with_the_locked_text_and_other_bytes_with_the_zip_text() {
        let mut compound = Vec::from(COMPOUND_MAGIC);
        compound.extend([0u8; 100]);
        let Err(FilesError::Failed(message)) = open(&compound, "locked") else { panic!("opened") };
        assert_eq!(message, "locked");
        let Err(FilesError::Failed(message)) = open(b"just words", "locked") else { panic!("opened") };
        assert!(!message.is_empty() && message != "locked", "{message}");
    }
}
