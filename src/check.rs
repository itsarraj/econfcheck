//! Checks one file's real bytes against its effective EditorConfig
//! properties (see `parse::effective_properties`) and reports every
//! violation found — line-numbered where the violation is per-line
//! (trailing whitespace, indent style), file-level where it isn't
//! (final newline, line-ending consistency).

use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Violation {
    pub line: Option<usize>,
    pub rule: String,
    pub message: String,
}

pub fn check_content(
    content: &str,
    raw_bytes: &[u8],
    props: &BTreeMap<String, String>,
) -> Vec<Violation> {
    let mut violations = Vec::new();

    if let Some(style) = props.get("indent_style") {
        check_indent_style(content, style, &mut violations);
    }
    if let (Some(style), Some(size)) = (props.get("indent_style"), props.get("indent_size")) {
        if style == "space" {
            if let Ok(size) = size.parse::<usize>() {
                check_indent_size(content, size, &mut violations);
            }
        }
    }
    if props.get("trim_trailing_whitespace").map(String::as_str) == Some("true") {
        check_trailing_whitespace(content, &mut violations);
    }
    if props.get("insert_final_newline").map(String::as_str) == Some("true") {
        check_final_newline(raw_bytes, &mut violations);
    }
    if let Some(eol) = props.get("end_of_line") {
        check_end_of_line(raw_bytes, eol, &mut violations);
    }

    violations
}

fn leading_whitespace(line: &str) -> &str {
    let end = line
        .find(|c: char| c != ' ' && c != '\t')
        .unwrap_or(line.len());
    &line[..end]
}

fn check_indent_style(content: &str, style: &str, out: &mut Vec<Violation>) {
    let wrong = if style == "space" { '\t' } else { ' ' };
    // Only the FIRST character of the indent run matters — a `space`
    // policy still allows tabs used for mid-line alignment after some
    // spaces, matching how real EditorConfig-aware editors interpret
    // this: the file's block-indent character, not every whitespace byte.
    for (i, line) in content.lines().enumerate() {
        let indent = leading_whitespace(line);
        if let Some(first) = indent.chars().next() {
            if first == wrong {
                out.push(Violation {
                    line: Some(i + 1),
                    rule: "indent_style".to_string(),
                    message: format!("expected {style} indentation, found a leading '{wrong}'"),
                });
            }
        }
    }
}

fn check_indent_size(content: &str, size: usize, out: &mut Vec<Violation>) {
    if size == 0 {
        return;
    }
    for (i, line) in content.lines().enumerate() {
        let indent = leading_whitespace(line);
        if indent.contains('\t') {
            continue; // a tab-indented line under indent_size is indent_style's problem, not this rule's
        }
        let n = indent.chars().count();
        if n > 0 && !n.is_multiple_of(size) {
            out.push(Violation {
                line: Some(i + 1),
                rule: "indent_size".to_string(),
                message: format!("indented {n} spaces, not a multiple of {size}"),
            });
        }
    }
}

fn check_trailing_whitespace(content: &str, out: &mut Vec<Violation>) {
    for (i, line) in content.lines().enumerate() {
        if line != line.trim_end_matches([' ', '\t']) {
            out.push(Violation {
                line: Some(i + 1),
                rule: "trim_trailing_whitespace".to_string(),
                message: "trailing whitespace".to_string(),
            });
        }
    }
}

fn check_final_newline(raw_bytes: &[u8], out: &mut Vec<Violation>) {
    if raw_bytes.is_empty() {
        return; // an empty file has no missing-newline problem to report
    }
    if *raw_bytes.last().unwrap() != b'\n' {
        out.push(Violation {
            line: None,
            rule: "insert_final_newline".to_string(),
            message: "file does not end with a newline".to_string(),
        });
    }
}

fn check_end_of_line(raw_bytes: &[u8], eol: &str, out: &mut Vec<Violation>) {
    let expected_has_cr = eol == "crlf";
    let mut line_no = 1usize;
    let mut i = 0usize;
    while i < raw_bytes.len() {
        if raw_bytes[i] == b'\n' {
            let has_cr = i > 0 && raw_bytes[i - 1] == b'\r';
            if has_cr != expected_has_cr {
                out.push(Violation {
                    line: Some(line_no),
                    rule: "end_of_line".to_string(),
                    message: format!("expected {eol} line ending"),
                });
            }
            line_no += 1;
        }
        i += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn props(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    #[test]
    fn flags_a_tab_indented_line_when_space_style_expected() {
        let v = check_content(
            "fn x() {\n\tlet a = 1;\n}\n",
            b"fn x() {\n\tlet a = 1;\n}\n",
            &props(&[("indent_style", "space")]),
        );
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, "indent_style");
        assert_eq!(v[0].line, Some(2));
    }

    #[test]
    fn space_indented_file_passes_space_style() {
        let content = "fn x() {\n    let a = 1;\n}\n";
        let v = check_content(
            content,
            content.as_bytes(),
            &props(&[("indent_style", "space")]),
        );
        assert!(v.is_empty());
    }

    #[test]
    fn flags_indent_not_a_multiple_of_configured_size() {
        let content = "a\n   b\n";
        let v = check_content(
            content,
            content.as_bytes(),
            &props(&[("indent_style", "space"), ("indent_size", "4")]),
        );
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, "indent_size");
    }

    #[test]
    fn tab_indented_line_is_exempt_from_indent_size_check() {
        let content = "\tx\n";
        let v = check_content(
            content,
            content.as_bytes(),
            &props(&[("indent_style", "space"), ("indent_size", "4")]),
        );
        assert!(v.iter().all(|x| x.rule != "indent_size"));
    }

    #[test]
    fn flags_trailing_whitespace_on_exactly_the_right_line() {
        let content = "clean\ndirty   \nclean\n";
        let v = check_content(
            content,
            content.as_bytes(),
            &props(&[("trim_trailing_whitespace", "true")]),
        );
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].line, Some(2));
    }

    #[test]
    fn does_not_check_trailing_whitespace_when_rule_absent() {
        let content = "dirty   \n";
        let v = check_content(content, content.as_bytes(), &BTreeMap::new());
        assert!(v.is_empty());
    }

    #[test]
    fn flags_missing_final_newline() {
        let content = "no newline at end";
        let v = check_content(
            content,
            content.as_bytes(),
            &props(&[("insert_final_newline", "true")]),
        );
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].rule, "insert_final_newline");
    }

    #[test]
    fn does_not_flag_final_newline_when_present() {
        let content = "has one\n";
        let v = check_content(
            content,
            content.as_bytes(),
            &props(&[("insert_final_newline", "true")]),
        );
        assert!(v.is_empty());
    }

    #[test]
    fn empty_file_never_flags_missing_final_newline() {
        let v = check_content("", b"", &props(&[("insert_final_newline", "true")]));
        assert!(v.is_empty());
    }

    #[test]
    fn flags_crlf_line_when_lf_expected() {
        let raw = b"a\r\nb\n";
        let v = check_content("a\r\nb\n", raw, &props(&[("end_of_line", "lf")]));
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].line, Some(1));
    }

    #[test]
    fn flags_lf_line_when_crlf_expected() {
        let raw = b"a\nb\r\n";
        let v = check_content("a\nb\r\n", raw, &props(&[("end_of_line", "crlf")]));
        assert_eq!(v.len(), 1);
        assert_eq!(v[0].line, Some(1));
    }

    #[test]
    fn consistent_crlf_file_passes_crlf_rule() {
        let raw = b"a\r\nb\r\n";
        let v = check_content("a\r\nb\r\n", raw, &props(&[("end_of_line", "crlf")]));
        assert!(v.is_empty());
    }

    #[test]
    fn no_properties_means_no_violations_at_all() {
        let content = "\tanything   \nwith no final newline";
        let v = check_content(content, content.as_bytes(), &BTreeMap::new());
        assert!(v.is_empty());
    }

    #[test]
    fn multiple_rule_violations_on_one_file_all_reported() {
        let content = "\tbad   \nfine";
        let v = check_content(
            content,
            content.as_bytes(),
            &props(&[
                ("indent_style", "space"),
                ("trim_trailing_whitespace", "true"),
                ("insert_final_newline", "true"),
            ]),
        );
        let rules: Vec<&str> = v.iter().map(|x| x.rule.as_str()).collect();
        assert!(rules.contains(&"indent_style"));
        assert!(rules.contains(&"trim_trailing_whitespace"));
        assert!(rules.contains(&"insert_final_newline"));
    }
}
