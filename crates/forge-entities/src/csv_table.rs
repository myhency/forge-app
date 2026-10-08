//! A small CSV reader for the Files panel's table (RFC 4180: quoted fields, doubled quotes, new
//! lines inside quotes, CRLF). It keeps only the rows the table shows and counts the rest.

/// The most data rows the Files panel's table shows.
pub const CSV_ROW_LIMIT: usize = 500;

/// A csv text as a table: `header` and every row have the same number of cells.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CsvTable {
    pub header: Vec<String>,
    pub rows: Vec<Vec<String>>,
    /// Every data record in the text, kept or not.
    pub total_rows: usize,
    /// `total_rows` is more than the limit the table was read with.
    pub truncated: bool,
}

// Ends the record being read. A line with nothing on it is not a record. `keep` is how many records
// (the header included) are stored.
fn end_record(record: &mut Vec<String>, field: &mut String, seen: &mut usize, kept: &mut Vec<Vec<String>>, keep: usize) {
    record.push(std::mem::take(field));
    let row = std::mem::take(record);
    if row.len() == 1 && row[0].is_empty() {
        return;
    }
    *seen += 1;
    if kept.len() < keep {
        kept.push(row);
    }
}

/// Reads `text` as csv, keeping the header and at most `limit` data rows.
pub fn parse_csv(text: &str, limit: usize) -> CsvTable {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let keep = limit.saturating_add(1);
    let mut kept: Vec<Vec<String>> = Vec::new();
    let mut seen = 0usize;
    let mut record: Vec<String> = Vec::new();
    let mut field = String::new();
    let mut quoted = false;
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if quoted {
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    chars.next();
                    field.push('"');
                } else {
                    quoted = false;
                }
            } else {
                field.push(c);
            }
            continue;
        }
        match c {
            '"' if field.is_empty() => quoted = true,
            ',' => record.push(std::mem::take(&mut field)),
            '\r' => {
                if chars.peek() == Some(&'\n') {
                    chars.next();
                }
                end_record(&mut record, &mut field, &mut seen, &mut kept, keep);
            }
            '\n' => end_record(&mut record, &mut field, &mut seen, &mut kept, keep),
            _ => field.push(c),
        }
    }
    if !field.is_empty() || !record.is_empty() {
        end_record(&mut record, &mut field, &mut seen, &mut kept, keep);
    }

    let columns = kept.iter().map(Vec::len).max().unwrap_or(0);
    for row in &mut kept {
        row.resize(columns, String::new());
    }
    let mut records = kept.into_iter();
    let header = records.next().unwrap_or_default();
    let total_rows = seen.saturating_sub(1);
    CsvTable { header, rows: records.collect(), total_rows, truncated: total_rows > limit }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cells(row: &[&str]) -> Vec<String> {
        row.iter().map(|cell| cell.to_string()).collect()
    }

    #[test]
    fn the_first_record_is_the_header_and_the_rest_are_rows() {
        let table = parse_csv("name,age\nAda,36\nLinus,55\n", 500);
        assert_eq!(table.header, cells(&["name", "age"]));
        assert_eq!(table.rows, vec![cells(&["Ada", "36"]), cells(&["Linus", "55"])]);
        assert_eq!(table.total_rows, 2);
        assert!(!table.truncated);
    }

    #[test]
    fn a_quoted_field_keeps_commas_doubled_quotes_and_new_lines() {
        let table = parse_csv("a,b\n\"x, y\",\"say \"\"hi\"\"\"\n\"line one\nline two\",z\n", 500);
        assert_eq!(table.rows[0], cells(&["x, y", "say \"hi\""]));
        assert_eq!(table.rows[1], cells(&["line one\nline two", "z"]));
        assert_eq!(table.total_rows, 2);
    }

    #[test]
    fn windows_line_ends_and_a_byte_order_mark_are_not_part_of_the_cells() {
        let table = parse_csv("\u{feff}a,b\r\n1,2\r\n3,4", 500);
        assert_eq!(table.header, cells(&["a", "b"]));
        assert_eq!(table.rows, vec![cells(&["1", "2"]), cells(&["3", "4"])]);
    }

    #[test]
    fn blank_lines_are_skipped_and_a_last_line_needs_no_new_line() {
        let table = parse_csv("a,b\n\n1,2\n\n\n3,4", 500);
        assert_eq!(table.total_rows, 2);
        assert_eq!(table.rows[1], cells(&["3", "4"]));
    }

    #[test]
    fn a_short_row_is_padded_and_a_wide_row_widens_the_header() {
        let table = parse_csv("a,b,c\n1\n1,2,3,4\n", 500);
        assert_eq!(table.header, cells(&["a", "b", "c", ""]));
        assert_eq!(table.rows[0], cells(&["1", "", "", ""]));
        assert_eq!(table.rows[1], cells(&["1", "2", "3", "4"]));
    }

    #[test]
    fn rows_past_the_limit_are_counted_but_not_kept() {
        let table = parse_csv("n\n1\n2\n3\n4\n5\n", 2);
        assert_eq!(table.rows, vec![cells(&["1"]), cells(&["2"])]);
        assert_eq!(table.total_rows, 5);
        assert!(table.truncated);
    }

    #[test]
    fn exactly_the_limit_is_not_truncated() {
        let table = parse_csv("n\n1\n2\n", 2);
        assert_eq!(table.total_rows, 2);
        assert!(!table.truncated);
    }

    #[test]
    fn an_empty_text_and_a_header_only_text_have_no_rows() {
        let empty = parse_csv("", 500);
        assert!(empty.header.is_empty() && empty.rows.is_empty());
        assert_eq!(empty.total_rows, 0);
        let header_only = parse_csv("a,b\n", 500);
        assert_eq!(header_only.header, cells(&["a", "b"]));
        assert!(header_only.rows.is_empty());
    }

    #[test]
    fn an_empty_quoted_cell_at_the_end_of_a_line_is_kept() {
        let table = parse_csv("a,b\n1,\"\"\n", 500);
        assert_eq!(table.rows[0], cells(&["1", ""]));
    }
}
