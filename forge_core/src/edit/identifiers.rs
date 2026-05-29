use std::path::Path;

pub(crate) fn language_from_extension(path: &Path) -> crate::types::Language {
    match path.extension().and_then(|e| e.to_str()) {
        Some("rs") => crate::types::Language::Rust,
        Some("py") => crate::types::Language::Python,
        Some("java") => crate::types::Language::Java,
        Some("ts") | Some("tsx") => crate::types::Language::TypeScript,
        Some("js") | Some("jsx") => crate::types::Language::JavaScript,
        Some("c") | Some("h") => crate::types::Language::C,
        Some("cpp") | Some("cc") | Some("cxx") | Some("hpp") => crate::types::Language::Cpp,
        Some("go") => crate::types::Language::Go,
        _ => crate::types::Language::Unknown("".to_string()),
    }
}

pub(crate) fn identifier_spans(
    content: &[u8],
    name: &str,
    lang: crate::types::Language,
) -> Vec<(usize, usize)> {
    let name_bytes = name.as_bytes();
    let name_len = name_bytes.len();
    let content_len = content.len();
    let mut spans = Vec::new();
    if name_len == 0 || content_len < name_len {
        return spans;
    }

    let qual_prefixes = qualified_prefixes(&lang);

    let mut i = 0;
    while i + name_len <= content_len {
        if &content[i..i + name_len] == name_bytes {
            let before_ok = i == 0 || !is_ident_char(content[i - 1], &lang);
            let after_ok =
                i + name_len == content_len || !is_ident_char(content[i + name_len], &lang);
            if before_ok && after_ok {
                spans.push((i, i + name_len));
            }
        }

        for prefix in &qual_prefixes {
            let full = format!("{}{}", prefix, name);
            let full_bytes = full.as_bytes();
            let full_len = full_bytes.len();
            if i + full_len <= content_len && &content[i..i + full_len] == full_bytes {
                let qual_name_start = i + prefix.len();
                let before_ok =
                    qual_name_start == 0 || !is_ident_char(content[qual_name_start - 1], &lang);
                let after_ok = qual_name_start + name_len == content_len
                    || !is_ident_char(content[qual_name_start + name_len], &lang);
                if before_ok && after_ok {
                    let already = spans
                        .iter()
                        .any(|(s, e)| *s == qual_name_start && *e == qual_name_start + name_len);
                    if !already {
                        spans.push((qual_name_start, qual_name_start + name_len));
                    }
                }
            }
        }

        i += 1;
    }
    spans
}

fn qualified_prefixes(lang: &crate::types::Language) -> Vec<&'static str> {
    match lang {
        crate::types::Language::Rust => vec!["self.", "crate::", "super::"],
        crate::types::Language::Python => vec!["self.", "cls."],
        crate::types::Language::Java => vec!["this."],
        crate::types::Language::TypeScript => vec!["this."],
        crate::types::Language::JavaScript => vec!["this."],
        crate::types::Language::C => vec!["struct ", "enum "],
        crate::types::Language::Cpp => vec!["this->", "struct ", "enum ", "class "],
        crate::types::Language::Go => vec![],
        crate::types::Language::Unknown(_) => vec![],
    }
}

fn is_ident_char(b: u8, lang: &crate::types::Language) -> bool {
    match lang {
        crate::types::Language::Rust => b.is_ascii_alphanumeric() || b == b'_',
        crate::types::Language::Python => b.is_ascii_alphanumeric() || b == b'_',
        crate::types::Language::Java => b.is_ascii_alphanumeric() || b == b'_',
        crate::types::Language::TypeScript => b.is_ascii_alphanumeric() || b == b'_' || b == b'$',
        crate::types::Language::JavaScript => b.is_ascii_alphanumeric() || b == b'_' || b == b'$',
        crate::types::Language::C => b.is_ascii_alphanumeric() || b == b'_',
        crate::types::Language::Cpp => b.is_ascii_alphanumeric() || b == b'_',
        crate::types::Language::Go => b.is_ascii_alphanumeric() || b == b'_',
        crate::types::Language::Unknown(_) => b.is_ascii_alphanumeric() || b == b'_',
    }
}
