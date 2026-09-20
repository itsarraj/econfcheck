//! A minimal EditorConfig glob matcher: `*` (any run of characters
//! except `/`), `**` (any run including `/`), `?` (one character except
//! `/`), and `{a,b,c}` brace alternation. Character classes (`[abc]`,
//! `[!abc]`) are not supported — see the README's scope limits.

/// True if `path` (relative to the `.editorconfig` file's own
/// directory, always `/`-separated) matches `pattern`.
pub fn glob_match(pattern: &str, path: &str) -> bool {
    match_from(pattern.as_bytes(), path.as_bytes())
}

fn match_from(pattern: &[u8], path: &[u8]) -> bool {
    // A pattern with no `/` at all matches the path's final component
    // only — EditorConfig's own documented shorthand (`*.rs` means "any
    // file named `*.rs` at any depth", not "a top-level file").
    if !pattern.contains(&b'/') {
        let component = path.rsplit(|&c| c == b'/').next().unwrap_or(path);
        return match_here(pattern, component) || match_here(pattern, path);
    }
    match_here(pattern, path)
}

fn match_here(pattern: &[u8], text: &[u8]) -> bool {
    match_rec(pattern, text)
}

fn match_rec(pat: &[u8], text: &[u8]) -> bool {
    if pat.is_empty() {
        return text.is_empty();
    }
    if pat.starts_with(b"**") {
        // `**` matches zero or more of anything, including `/` — and,
        // matching every mainstream glob implementation's convention,
        // `**/` collapses to nothing when it matches zero directories
        // (so `src/**/*.rs` matches `src/c.rs`, not just `src/a/b.rs`).
        let after_star = &pat[2..];
        let after_slash = after_star.strip_prefix(b"/").unwrap_or(after_star);
        if match_rec(after_slash, text) {
            return true;
        }
        if !text.is_empty() {
            return match_rec(pat, &text[1..]);
        }
        return false;
    }
    if pat[0] == b'*' {
        // `*` matches zero or more characters except `/`.
        for i in 0..=text.len() {
            if text[..i].contains(&b'/') {
                break;
            }
            if match_rec(&pat[1..], &text[i..]) {
                return true;
            }
        }
        return false;
    }
    if pat[0] == b'?' {
        if text.is_empty() || text[0] == b'/' {
            return false;
        }
        return match_rec(&pat[1..], &text[1..]);
    }
    if pat[0] == b'{' {
        let close = match pat.iter().position(|&c| c == b'}') {
            Some(idx) => idx,
            None => return false, // malformed pattern: unmatched `{`
        };
        let inner = &pat[1..close];
        let rest = &pat[close + 1..];
        for alt in inner.split(|&c| c == b',') {
            let mut combined = alt.to_vec();
            combined.extend_from_slice(rest);
            if match_rec(&combined, text) {
                return true;
            }
        }
        return false;
    }
    if text.is_empty() || text[0] != pat[0] {
        return false;
    }
    match_rec(&pat[1..], &text[1..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_filename_matches_only_itself() {
        assert!(glob_match("Makefile", "Makefile"));
        assert!(!glob_match("Makefile", "makefile"));
    }

    #[test]
    fn star_matches_extension_at_any_depth() {
        assert!(glob_match("*.rs", "main.rs"));
        assert!(glob_match("*.rs", "src/deep/nested/lib.rs"));
        assert!(!glob_match("*.rs", "main.rs.bak"));
    }

    #[test]
    fn star_does_not_cross_a_path_separator() {
        assert!(!glob_match("src/*.rs", "src/deep/lib.rs"));
        assert!(glob_match("src/*.rs", "src/lib.rs"));
    }

    #[test]
    fn double_star_crosses_path_separators() {
        assert!(glob_match("src/**/*.rs", "src/a/b/c.rs"));
        assert!(glob_match("src/**/*.rs", "src/c.rs"));
    }

    #[test]
    fn question_mark_matches_exactly_one_char_not_slash() {
        assert!(glob_match("file?.txt", "file1.txt"));
        assert!(!glob_match("file?.txt", "file12.txt"));
        assert!(!glob_match("a?b", "a/b"));
    }

    #[test]
    fn brace_alternation_matches_any_option() {
        assert!(glob_match("*.{yml,yaml}", "config.yml"));
        assert!(glob_match("*.{yml,yaml}", "config.yaml"));
        assert!(!glob_match("*.{yml,yaml}", "config.json"));
    }

    #[test]
    fn top_level_wildcard_matches_nested_files_too() {
        assert!(glob_match("*", "anything"));
        assert!(glob_match("*.md", "docs/nested/README.md"));
    }

    #[test]
    fn malformed_brace_pattern_does_not_panic() {
        assert!(!glob_match("*.{yml", "config.yml"));
    }
}
