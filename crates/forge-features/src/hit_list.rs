use forge_ports::ContentHit;

/// The hits of one file, in the order the search found them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HitGroup {
    pub path: String,
    pub hits: Vec<ContentHit>,
}

/// Groups hits by file. The search returns them in path order, so a group is a run of one path.
pub fn group_hits(hits: &[ContentHit]) -> Vec<HitGroup> {
    let mut groups: Vec<HitGroup> = Vec::new();
    for hit in hits {
        match groups.last_mut() {
            Some(group) if group.path == hit.path => group.hits.push(hit.clone()),
            _ => groups.push(HitGroup { path: hit.path.clone(), hits: vec![hit.clone()] }),
        }
    }
    groups
}

/// The text before, inside and after the match (char offsets, clamped to the text).
pub fn split_hit(hit: &ContentHit) -> (String, String, String) {
    let chars: Vec<char> = hit.text.chars().collect();
    let start = hit.start.min(chars.len());
    let end = hit.end.clamp(start, chars.len());
    (chars[..start].iter().collect(), chars[start..end].iter().collect(), chars[end..].iter().collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hit(path: &str, line: usize, text: &str, start: usize, end: usize) -> ContentHit {
        ContentHit { path: path.into(), line, text: text.into(), start, end }
    }

    #[test]
    fn hits_are_grouped_by_path_in_the_order_they_arrive() {
        let hits = vec![hit("a.py", 1, "x", 0, 1), hit("a.py", 5, "y", 0, 1), hit("b/c.py", 2, "z", 0, 1)];
        let groups = group_hits(&hits);
        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].path, "a.py");
        assert_eq!(groups[0].hits.iter().map(|h| h.line).collect::<Vec<_>>(), vec![1, 5]);
        assert_eq!(groups[1].path, "b/c.py");
    }

    #[test]
    fn no_hits_no_groups() {
        assert_eq!(group_hits(&[]), vec![]);
    }

    #[test]
    fn a_hit_splits_at_char_offsets_including_hangul() {
        let (before, matched, after) = split_hit(&hit("a", 1, "한글 hello 끝", 3, 8));
        assert_eq!((before.as_str(), matched.as_str(), after.as_str()), ("한글 ", "hello", " 끝"));
    }

    #[test]
    fn offsets_past_the_text_are_clamped_not_a_panic() {
        let (before, matched, after) = split_hit(&hit("a", 1, "abc", 2, 99));
        assert_eq!((before.as_str(), matched.as_str(), after.as_str()), ("ab", "c", ""));
        let (before, matched, after) = split_hit(&hit("a", 1, "abc", 9, 12));
        assert_eq!((before.as_str(), matched.as_str(), after.as_str()), ("abc", "", ""));
    }
}
