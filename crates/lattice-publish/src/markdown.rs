//! Minimal Markdown → HTML for offline page exports (no live Lattice).

use crate::error::Result;

/// Convert CommonMark-ish Markdown into an HTML fragment.
///
/// Supports headings, paragraphs, fenced code, lists, blockquotes, hr, and
/// a small set of inline marks (code, bold, italic, links). Enough for page
/// export without pulling a Markdown crate into the workspace.
pub fn markdown_to_html(markdown: &str) -> Result<String> {
    let mut out = String::new();
    let mut lines = markdown.lines().peekable();
    let mut in_code = false;
    let mut code_lang = String::new();
    let mut code_buf = String::new();
    let mut list_open = false;

    while let Some(line) = lines.next() {
        if in_code {
            if line.starts_with("```") {
                out.push_str("<pre><code");
                if !code_lang.is_empty() {
                    out.push_str(" class=\"language-");
                    out.push_str(&escape_attr(&code_lang));
                    out.push('"');
                }
                out.push('>');
                out.push_str(&escape_html(&code_buf));
                out.push_str("</code></pre>\n");
                in_code = false;
                code_lang.clear();
                code_buf.clear();
            } else {
                if !code_buf.is_empty() {
                    code_buf.push('\n');
                }
                code_buf.push_str(line);
            }
            continue;
        }

        if let Some(lang) = line.strip_prefix("```") {
            if list_open {
                out.push_str("</ul>\n");
                list_open = false;
            }
            in_code = true;
            code_lang = lang.trim().to_string();
            code_buf.clear();
            continue;
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            if list_open {
                out.push_str("</ul>\n");
                list_open = false;
            }
            continue;
        }

        if trimmed == "---" || trimmed == "***" || trimmed == "___" {
            if list_open {
                out.push_str("</ul>\n");
                list_open = false;
            }
            out.push_str("<hr />\n");
            continue;
        }

        if let Some(rest) = strip_heading(trimmed) {
            if list_open {
                out.push_str("</ul>\n");
                list_open = false;
            }
            let (level, text) = rest;
            out.push_str(&format!("<h{level}>{}</h{level}>\n", render_inline(text)));
            continue;
        }

        if let Some(item) = trimmed
            .strip_prefix("- ")
            .or_else(|| trimmed.strip_prefix("* "))
        {
            if !list_open {
                out.push_str("<ul>\n");
                list_open = true;
            }
            out.push_str(&format!("<li>{}</li>\n", render_inline(item)));
            continue;
        }

        if let Some(quote) = trimmed.strip_prefix("> ") {
            if list_open {
                out.push_str("</ul>\n");
                list_open = false;
            }
            out.push_str(&format!(
                "<blockquote><p>{}</p></blockquote>\n",
                render_inline(quote)
            ));
            continue;
        }

        if list_open {
            out.push_str("</ul>\n");
            list_open = false;
        }

        // Gather consecutive paragraph lines.
        let mut para = trimmed.to_string();
        while let Some(next) = lines.peek() {
            let next_trim = next.trim();
            if next_trim.is_empty()
                || next_trim.starts_with("```")
                || next_trim.starts_with('#')
                || next_trim.starts_with("- ")
                || next_trim.starts_with("* ")
                || next_trim.starts_with("> ")
                || next_trim == "---"
            {
                break;
            }
            para.push(' ');
            para.push_str(next_trim);
            lines.next();
        }
        out.push_str(&format!("<p>{}</p>\n", render_inline(&para)));
    }

    if in_code {
        out.push_str("<pre><code>");
        out.push_str(&escape_html(&code_buf));
        out.push_str("</code></pre>\n");
    }
    if list_open {
        out.push_str("</ul>\n");
    }

    Ok(out)
}

fn strip_heading(line: &str) -> Option<(u8, &str)> {
    let mut level = 0u8;
    let bytes = line.as_bytes();
    while (level as usize) < bytes.len() && bytes[level as usize] == b'#' && level < 6 {
        level += 1;
    }
    if level == 0 {
        return None;
    }
    let rest = line[level as usize..].trim_start();
    if rest.is_empty() && line.len() == level as usize {
        return None;
    }
    // Require a space after hashes for ATX headings when body remains.
    if line.len() > level as usize
        && bytes[level as usize] != b' '
        && bytes[level as usize] != b'\t'
    {
        return None;
    }
    Some((level, rest))
}

fn render_inline(input: &str) -> String {
    let mut out = String::new();
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '`' {
            if let Some(end) = chars[i + 1..].iter().position(|&c| c == '`') {
                let content: String = chars[i + 1..i + 1 + end].iter().collect();
                out.push_str("<code>");
                out.push_str(&escape_html(&content));
                out.push_str("</code>");
                i += end + 2;
                continue;
            }
        }
        if chars[i] == '[' {
            if let Some(link) = parse_link(&chars[i..]) {
                out.push_str("<a href=\"");
                out.push_str(&escape_attr(&link.href));
                out.push_str("\">");
                out.push_str(&escape_html(&link.text));
                out.push_str("</a>");
                i += link.consumed;
                continue;
            }
        }
        if chars[i] == '*' && i + 1 < chars.len() && chars[i + 1] == '*' {
            if let Some(end) = find_closing(&chars[i + 2..], &['*', '*']) {
                let content: String = chars[i + 2..i + 2 + end].iter().collect();
                out.push_str("<strong>");
                out.push_str(&escape_html(&content));
                out.push_str("</strong>");
                i += end + 4;
                continue;
            }
        }
        if chars[i] == '*' {
            if let Some(end) = chars[i + 1..].iter().position(|&c| c == '*') {
                let content: String = chars[i + 1..i + 1 + end].iter().collect();
                if !content.is_empty() {
                    out.push_str("<em>");
                    out.push_str(&escape_html(&content));
                    out.push_str("</em>");
                    i += end + 2;
                    continue;
                }
            }
        }
        out.push_str(&escape_html(&chars[i].to_string()));
        i += 1;
    }
    out
}

struct Link<'a> {
    text: String,
    href: String,
    consumed: usize,
    _marker: std::marker::PhantomData<&'a ()>,
}

fn parse_link(chars: &[char]) -> Option<Link<'static>> {
    if chars.first() != Some(&'[') {
        return None;
    }
    let close_text = chars[1..].iter().position(|&c| c == ']')?;
    let after = 1 + close_text + 1;
    if chars.get(after) != Some(&'(') {
        return None;
    }
    let close_href = chars[after + 1..].iter().position(|&c| c == ')')?;
    let text: String = chars[1..1 + close_text].iter().collect();
    let href: String = chars[after + 1..after + 1 + close_href].iter().collect();
    Some(Link {
        text,
        href,
        consumed: after + 1 + close_href + 1,
        _marker: std::marker::PhantomData,
    })
}

fn find_closing(chars: &[char], needle: &[char]) -> Option<usize> {
    if needle.len() != 2 {
        return None;
    }
    let mut i = 0;
    while i + 1 < chars.len() {
        if chars[i] == needle[0] && chars[i + 1] == needle[1] {
            return Some(i);
        }
        i += 1;
    }
    None
}

pub fn escape_html(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for ch in input.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&#39;"),
            _ => out.push(ch),
        }
    }
    out
}

pub fn escape_attr(input: &str) -> String {
    escape_html(input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_heading_and_paragraph() {
        let html = markdown_to_html("# Hello\n\nWorld **bold** and `code`.\n").unwrap();
        assert!(html.contains("<h1>Hello</h1>"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<code>code</code>"));
    }

    #[test]
    fn renders_fenced_code() {
        let html = markdown_to_html("```rust\nfn main() {}\n```\n").unwrap();
        assert!(html.contains("language-rust"));
        assert!(html.contains("fn main() {}"));
    }
}
