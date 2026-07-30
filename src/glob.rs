use std::fs;
use std::path::Path;

// `*` matches any run of characters (including none); `?` matches exactly
// one character. No character classes or brace expansion — this only needs
// to cover the common *.ext / file?.txt cases.
fn wildcard_matches(pattern: &str, name: &str) -> bool {
    let pattern: Vec<char> = pattern.chars().collect();
    let name: Vec<char> = name.chars().collect();
    wildcard_matches_chars(&pattern, &name)
}

fn wildcard_matches_chars(pattern: &[char], name: &[char]) -> bool {
    match pattern.first() {
        None => name.is_empty(),
        Some('*') => (0..=name.len()).any(|i| wildcard_matches_chars(&pattern[1..], &name[i..])),
        Some('?') => !name.is_empty() && wildcard_matches_chars(&pattern[1..], &name[1..]),
        Some(&c) => name.first() == Some(&c) && wildcard_matches_chars(&pattern[1..], &name[1..]),
    }
}

fn is_pattern(token: &str) -> bool {
    token.contains('*') || token.contains('?')
}

// Expands a single glob token (e.g. "*.txt" or "sub/*.rs") into the sorted
// list of matching paths, preserving the token's directory prefix. Returns
// None if the token isn't a glob or matches nothing, so the caller can fall
// back to the literal token — matching normal shell behavior when nothing
// matches (the pattern is left as-is rather than expanding to nothing).
fn expand(token: &str) -> Option<Vec<String>> {
    if !is_pattern(token) {
        return None;
    }

    let path = Path::new(token);
    let file_pattern = path.file_name()?.to_string_lossy().to_string();
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
    let dir = parent.unwrap_or_else(|| Path::new("."));

    let mut matches: Vec<String> = fs::read_dir(dir)
        .ok()?
        .filter_map(|entry| entry.ok())
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') && !file_pattern.starts_with('.') {
                return None;
            }
            if !wildcard_matches(&file_pattern, &name) {
                return None;
            }
            Some(match parent {
                Some(p) => p.join(&name).to_string_lossy().to_string(),
                None => name,
            })
        })
        .collect();

    if matches.is_empty() {
        return None;
    }
    matches.sort();
    Some(matches)
}

// Expands every glob token in `tokens`, leaving non-glob and non-matching
// tokens untouched.
pub fn expand_all(tokens: &[String]) -> Vec<String> {
    tokens
        .iter()
        .flat_map(|token| expand(token).unwrap_or_else(|| vec![token.clone()]))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "zero_shell_glob_test_{}_{}",
            std::process::id(),
            name
        ));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn wildcard_matches_star_suffix() {
        assert!(wildcard_matches("*.txt", "notes.txt"));
        assert!(!wildcard_matches("*.txt", "notes.md"));
    }

    #[test]
    fn wildcard_matches_star_matches_empty() {
        assert!(wildcard_matches("*.txt", ".txt"));
    }

    #[test]
    fn wildcard_matches_question_mark_is_exactly_one_char() {
        assert!(wildcard_matches("file?.txt", "file1.txt"));
        assert!(!wildcard_matches("file?.txt", "file12.txt"));
        assert!(!wildcard_matches("file?.txt", "file.txt"));
    }

    #[test]
    fn wildcard_matches_literal_with_no_wildcards() {
        assert!(wildcard_matches("exact.txt", "exact.txt"));
        assert!(!wildcard_matches("exact.txt", "exact2.txt"));
    }

    #[test]
    fn expand_returns_none_for_non_glob_token() {
        assert_eq!(expand("plain.txt"), None);
    }

    #[test]
    fn expand_returns_none_when_nothing_matches() {
        let dir = temp_dir("no_match");
        let pattern = dir.join("*.zzz").to_string_lossy().to_string();
        assert_eq!(expand(&pattern), None);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn expand_matches_and_sorts_results() {
        let dir = temp_dir("matches_sorted");
        fs::write(dir.join("b.txt"), b"").unwrap();
        fs::write(dir.join("a.txt"), b"").unwrap();
        fs::write(dir.join("c.md"), b"").unwrap();

        let pattern = dir.join("*.txt").to_string_lossy().to_string();
        let matches = expand(&pattern).unwrap();
        let expected = vec![
            dir.join("a.txt").to_string_lossy().to_string(),
            dir.join("b.txt").to_string_lossy().to_string(),
        ];
        assert_eq!(matches, expected);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn expand_excludes_dotfiles_unless_pattern_starts_with_dot() {
        let dir = temp_dir("dotfiles");
        fs::write(dir.join(".hidden.txt"), b"").unwrap();
        fs::write(dir.join("visible.txt"), b"").unwrap();

        let pattern = dir.join("*.txt").to_string_lossy().to_string();
        let matches = expand(&pattern).unwrap();
        assert_eq!(
            matches,
            vec![dir.join("visible.txt").to_string_lossy().to_string()]
        );

        let dot_pattern = dir.join(".*.txt").to_string_lossy().to_string();
        let dot_matches = expand(&dot_pattern).unwrap();
        assert_eq!(
            dot_matches,
            vec![dir.join(".hidden.txt").to_string_lossy().to_string()]
        );
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn expand_all_leaves_non_matching_and_non_glob_tokens_alone() {
        let tokens: Vec<String> = vec!["ls".to_string(), "-la".to_string(), "*.zzz".to_string()];
        assert_eq!(expand_all(&tokens), tokens);
    }

    #[test]
    fn expand_all_splices_matches_into_place() {
        let dir = temp_dir("expand_all");
        fs::write(dir.join("a.txt"), b"").unwrap();
        fs::write(dir.join("b.txt"), b"").unwrap();

        let pattern = dir.join("*.txt").to_string_lossy().to_string();
        let tokens = vec!["cat".to_string(), pattern];
        let expanded = expand_all(&tokens);
        assert_eq!(
            expanded,
            vec![
                "cat".to_string(),
                dir.join("a.txt").to_string_lossy().to_string(),
                dir.join("b.txt").to_string_lossy().to_string(),
            ]
        );
        fs::remove_dir_all(&dir).unwrap();
    }
}
