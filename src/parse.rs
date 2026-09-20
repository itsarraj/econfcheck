//! Parses `.editorconfig`'s real INI-like format: an optional
//! `root = true` at the top, then `[glob]` sections each holding
//! `key = value` properties. Later sections override earlier ones for
//! the same key when both match a given file — the real EditorConfig
//! "last matching section wins, per property" merge rule.

use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EditorConfig {
    pub root: bool,
    pub sections: Vec<Section>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Section {
    pub glob: String,
    pub properties: BTreeMap<String, String>,
}

pub fn parse(content: &str) -> EditorConfig {
    let mut root = false;
    let mut sections: Vec<Section> = Vec::new();

    for raw_line in content.lines() {
        let line = strip_comment(raw_line).trim();
        if line.is_empty() {
            continue;
        }
        if let Some(glob) = line.strip_prefix('[').and_then(|s| s.strip_suffix(']')) {
            sections.push(Section {
                glob: glob.to_string(),
                properties: BTreeMap::new(),
            });
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue; // malformed line, ignored rather than erroring the whole file out
        };
        let key = key.trim().to_lowercase();
        let value = value.trim().to_lowercase();
        match sections.last_mut() {
            Some(section) => {
                section.properties.insert(key, value);
            }
            None if key == "root" => root = value == "true",
            None => {} // a property before any [section] header, outside of `root`, is ignored
        }
    }

    EditorConfig { root, sections }
}

fn strip_comment(line: &str) -> &str {
    for (i, c) in line.char_indices() {
        if c == '#' || c == ';' {
            return &line[..i];
        }
    }
    line
}

/// Merges every section whose glob matches `relative_path`, in file
/// order, later sections overriding earlier ones property-by-property —
/// the effective rule set for one specific file.
pub fn effective_properties(
    config: &EditorConfig,
    relative_path: &str,
) -> BTreeMap<String, String> {
    let mut merged = BTreeMap::new();
    for section in &config.sections {
        if crate::glob::glob_match(&section.glob, relative_path) {
            for (k, v) in &section.properties {
                merged.insert(k.clone(), v.clone());
            }
        }
    }
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_root_and_a_single_section() {
        let cfg = parse("root = true\n\n[*]\nindent_style = space\nindent_size = 2\n");
        assert!(cfg.root);
        assert_eq!(cfg.sections.len(), 1);
        assert_eq!(cfg.sections[0].glob, "*");
        assert_eq!(
            cfg.sections[0].properties.get("indent_style").unwrap(),
            "space"
        );
        assert_eq!(cfg.sections[0].properties.get("indent_size").unwrap(), "2");
    }

    #[test]
    fn root_defaults_to_false_when_absent() {
        let cfg = parse("[*]\nindent_style = tab\n");
        assert!(!cfg.root);
    }

    #[test]
    fn comments_and_blank_lines_are_ignored() {
        let cfg =
            parse("; a semicolon comment\n# a hash comment\n\n[*.rs]\nindent_style = space\n");
        assert_eq!(cfg.sections.len(), 1);
    }

    #[test]
    fn keys_and_values_are_lowercased_and_trimmed() {
        let cfg = parse("[*]\n  Indent_Style  =  SPACE  \n");
        assert_eq!(
            cfg.sections[0].properties.get("indent_style").unwrap(),
            "space"
        );
    }

    #[test]
    fn a_malformed_line_is_skipped_not_fatal() {
        let cfg = parse("[*]\nnot_a_property_line\nindent_style = tab\n");
        assert_eq!(cfg.sections[0].properties.len(), 1);
    }

    #[test]
    fn later_section_overrides_earlier_for_the_same_matching_file() {
        let cfg = parse("[*]\nindent_style = space\n\n[*.md]\nindent_style = tab\n");
        let props = effective_properties(&cfg, "README.md");
        assert_eq!(props.get("indent_style").unwrap(), "tab");
    }

    #[test]
    fn non_matching_section_does_not_contribute_properties() {
        let cfg = parse("[*.py]\nindent_style = space\n");
        let props = effective_properties(&cfg, "main.rs");
        assert!(!props.contains_key("indent_style"));
    }

    #[test]
    fn a_property_before_any_section_header_is_ignored() {
        let cfg = parse("indent_style = tab\n[*]\nindent_size = 4\n");
        let props = effective_properties(&cfg, "x");
        assert!(!props.contains_key("indent_style"));
        assert_eq!(props.get("indent_size").unwrap(), "4");
    }
}
