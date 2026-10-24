//! The shard's actual work: finding TODO / FIXME / HACK markers.
//!
//! Keep your domain logic in plain, synchronous, unit-tested functions like
//! these; `main.rs` only wires them to the bus.

use std::path::{Component, Path};

use crate::payload::TodoFound;

/// Markers reported by this shard.
pub const MARKERS: [&str; 3] = ["TODO", "FIXME", "HACK"];

/// Files larger than this are skipped.
pub const MAX_FILE_BYTES: u64 = 256 * 1024;

/// Longest note text kept per marker.
const MAX_TEXT_CHARS: usize = 200;

/// A marker found in a text, before it is tied to a file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Marker {
    /// 1-based line number.
    pub line: u32,
    /// `TODO`, `FIXME` or `HACK`.
    pub marker: &'static str,
    /// The note following the marker.
    pub text: String,
}

/// Finds the first marker of each line. Markers must be whole words, so
/// `TODOS` or `fixme_count` do not match.
pub fn scan(text: &str) -> Vec<Marker> {
    text.lines()
        .enumerate()
        .filter_map(|(index, line)| {
            let (start, marker) = first_marker(line)?;
            let note = line[start + marker.len()..].trim_start_matches([':', ' ', '\t', '-', '(', ')']).trim_end();
            Some(Marker {
                line: u32::try_from(index + 1).unwrap_or(u32::MAX),
                marker,
                text: note.chars().take(MAX_TEXT_CHARS).collect(),
            })
        })
        .collect()
}

fn first_marker(line: &str) -> Option<(usize, &'static str)> {
    MARKERS
        .iter()
        .flat_map(|marker| line.match_indices(marker).map(move |(i, _)| (i, *marker)))
        .filter(|(i, marker)| {
            let before = line[..*i].chars().next_back();
            let after = line[i + marker.len()..].chars().next();
            !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char)
        })
        .min_by_key(|(i, _)| *i)
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

/// Scans a workspace file given its workspace relative path.
///
/// Returns nothing for paths escaping the workspace, missing files, files
/// over [`MAX_FILE_BYTES`] and binary files.
pub fn scan_file(root: &Path, relative: &str) -> std::io::Result<Vec<TodoFound>> {
    let rel = Path::new(relative);
    if rel.is_absolute() || rel.components().any(|c| !matches!(c, Component::Normal(_) | Component::CurDir)) {
        return Ok(Vec::new());
    }
    let path = root.join(rel);
    match std::fs::metadata(&path) {
        Ok(m) if m.is_file() && m.len() <= MAX_FILE_BYTES => {}
        Ok(_) => return Ok(Vec::new()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    }
    let bytes = std::fs::read(&path)?;
    if bytes.iter().take(8192).any(|&b| b == 0) {
        return Ok(Vec::new()); // binary
    }
    Ok(scan(&String::from_utf8_lossy(&bytes))
        .into_iter()
        .map(|m| TodoFound { path: relative.to_owned(), line: m.line, marker: m.marker.to_owned(), text: m.text })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_markers_with_notes() {
        let src = "fn a() {}\n// TODO: handle errors\n/* FIXME(alice) - leaks */\n# HACK\n";
        let found = scan(src);
        assert_eq!(
            found,
            vec![
                Marker { line: 2, marker: "TODO", text: "handle errors".into() },
                Marker { line: 3, marker: "FIXME", text: "alice) - leaks */".into() },
                Marker { line: 4, marker: "HACK", text: String::new() },
            ]
        );
    }

    #[test]
    fn requires_whole_words() {
        assert!(scan("let TODOS = 1; let fixme_count = 2; HACKY();").is_empty());
    }

    #[test]
    fn reports_the_first_marker_of_a_line() {
        let found = scan("x // FIXME then TODO");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].marker, "FIXME");
    }

    #[test]
    fn scans_workspace_files_safely() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir(dir.path().join("src")).unwrap();
        std::fs::write(dir.path().join("src/lib.rs"), "// TODO: test me\n").unwrap();
        std::fs::write(dir.path().join("blob.bin"), b"TODO\0binary").unwrap();
        std::fs::write(dir.path().join("big.txt"), "TODO\n".repeat(60_000)).unwrap();

        let found = scan_file(dir.path(), "src/lib.rs").unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].path.as_str(), found[0].line), ("src/lib.rs", 1));

        assert!(scan_file(dir.path(), "blob.bin").unwrap().is_empty());
        assert!(scan_file(dir.path(), "big.txt").unwrap().is_empty());
        assert!(scan_file(dir.path(), "missing.rs").unwrap().is_empty());
        assert!(scan_file(dir.path(), "../outside.rs").unwrap().is_empty());
    }
}
