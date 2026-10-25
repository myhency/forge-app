//! Guards shared by the Office readers, which all open a zip of XML parts. A hostile file is
//! refused here before any parser sees it (in 57A calamine aborted the whole process on one).

use std::io::{Read, Seek, Write};

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
}
