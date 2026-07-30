pub fn parse_flags(args: &[&str]) -> Vec<String> {
    let mut parsed_flags = Vec::new();

    for &arg in args {
        if arg.starts_with('-') && arg.len() > 2 {
            arg.chars()
                .skip(1)
                .for_each(|c| parsed_flags.push(format!("-{}", c)));
        } else {
            parsed_flags.push(arg.to_string());
        }
    }

    parsed_flags
}

// Splits a line into tokens, treating single- or double-quoted spans as a
// single argument so that e.g. `mkdir "my dir"` produces one argument
// containing a space instead of two.
pub fn tokenize(input: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();
    let mut in_single_quotes = false;
    let mut in_double_quotes = false;
    let mut has_token = false;

    for c in input.chars() {
        match c {
            '\'' if !in_double_quotes => {
                in_single_quotes = !in_single_quotes;
                has_token = true;
            }
            '"' if !in_single_quotes => {
                in_double_quotes = !in_double_quotes;
                has_token = true;
            }
            c if c.is_whitespace() && !in_single_quotes && !in_double_quotes => {
                if has_token {
                    tokens.push(std::mem::take(&mut current));
                    has_token = false;
                }
            }
            c => {
                current.push(c);
                has_token = true;
            }
        }
    }

    if has_token {
        tokens.push(current);
    }

    tokens
}

#[cfg(test)]
mod tests {
    use super::*;

    fn as_str_vec(v: &[String]) -> Vec<&str> {
        v.iter().map(|s| s.as_str()).collect()
    }

    #[test]
    fn parse_flags_expands_combined_short_flags() {
        assert_eq!(as_str_vec(&parse_flags(&["-la"])), vec!["-l", "-a"]);
    }

    #[test]
    fn parse_flags_leaves_separate_flags_untouched() {
        assert_eq!(as_str_vec(&parse_flags(&["-l", "-a"])), vec!["-l", "-a"]);
    }

    #[test]
    fn parse_flags_leaves_non_flag_arguments_untouched() {
        assert_eq!(as_str_vec(&parse_flags(&["file.txt"])), vec!["file.txt"]);
    }

    #[test]
    fn parse_flags_handles_mixed_flags_and_paths() {
        assert_eq!(
            as_str_vec(&parse_flags(&["-la", "file.txt"])),
            vec!["-l", "-a", "file.txt"]
        );
    }

    #[test]
    fn tokenize_splits_on_whitespace() {
        assert_eq!(tokenize("ls -la /tmp"), vec!["ls", "-la", "/tmp"]);
    }

    #[test]
    fn tokenize_keeps_double_quoted_span_as_one_token() {
        assert_eq!(tokenize(r#"mkdir "my dir""#), vec!["mkdir", "my dir"]);
    }

    #[test]
    fn tokenize_keeps_single_quoted_span_as_one_token() {
        assert_eq!(tokenize("mkdir 'my dir'"), vec!["mkdir", "my dir"]);
    }

    #[test]
    fn tokenize_preserves_internal_whitespace_in_quotes() {
        assert_eq!(tokenize(r#"echo "a   b""#), vec!["echo", "a   b"]);
    }

    #[test]
    fn tokenize_concatenates_adjacent_quoted_and_unquoted_text() {
        assert_eq!(tokenize(r#"echo hello"world""#), vec!["echo", "helloworld"]);
    }

    #[test]
    fn tokenize_handles_empty_input() {
        assert_eq!(tokenize(""), Vec::<String>::new());
    }
}
