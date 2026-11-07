//! Fills the page templates in `site/src` and writes the pages to `site/`.

use crate::{COUNTER, SNIPPETS, Source, examples};
use std::path::Path;

/// Pages in sidebar order: file name without extension, title.
const PAGES: &[(&str, &str)] = &[("index", "Getting started"), ("buttons", "Buttons and toolbars"), ("controls", "Controls")];

pub fn build(out: &Path) {
    let layout = std::fs::read_to_string(out.join("src/_layout.html")).expect("read site/src/_layout.html");
    for (slug, title) in PAGES {
        let template = std::fs::read_to_string(out.join(format!("src/{slug}.html"))).unwrap_or_else(|e| panic!("read site/src/{slug}.html: {e}"));
        let content = fill(&template);
        let nav: String = PAGES
            .iter()
            .map(|(s, t)| {
                let current = if s == slug { " aria-current=\"page\"" } else { "" };
                {
                let href = if *s == "index" { "./".to_string() } else { format!("{s}.html") };
                format!("<li><a href=\"{href}\"{current}>{t}</a></li>")
            }
            })
            .collect();
        let page = layout.replace("{{title}}", title).replace("{{nav}}", &nav).replace("{{content}}", &content);
        std::fs::write(out.join(format!("{slug}.html")), page).expect("write page");
        println!("wrote {slug}.html");
    }
}

fn fill(template: &str) -> String {
    let mut text = template.to_string();
    for (tag, class) in [("rust", "rust"), ("sh", "sh")] {
        let open = format!("{{{{{tag}}}}}");
        let close = format!("{{{{/{tag}}}}}");
        while let Some(start) = text.find(&open) {
            let end = text[start..].find(&close).expect("unclosed code block") + start;
            let code = text[start + open.len()..end].trim_matches('\n');
            let block = code_block(code, class == "rust");
            text.replace_range(start..end + close.len(), &block);
        }
    }
    while let Some(start) = text.find("{{example ") {
        let end = text[start..].find("}}").expect("unclosed example") + start;
        let name = text[start + "{{example ".len()..end].trim().to_string();
        text.replace_range(start..end + 2, &example(&name));
    }
    text
}

fn example(name: &str) -> String {
    let all = examples();
    let example = all.iter().find(|e| e.name == name).unwrap_or_else(|| panic!("no example named {name}"));
    let code = match example.source {
        Source::Counter => COUNTER.lines().skip_while(|l| l.starts_with("//!") || l.is_empty()).collect::<Vec<_>>().join("\n"),
        Source::Snippet(_) => function(SNIPPETS, name),
    };
    let [w, h] = example.size;
    format!(
        "<figure class=\"example\"><div class=\"preview\"><img class=\"light\" src=\"img/{name}-light.png\" alt=\"{alt}\" width=\"{w}\" height=\"{h}\"><img class=\"dark\" src=\"img/{name}-dark.png\" alt=\"{alt}\" width=\"{w}\" height=\"{h}\"></div>{code}</figure>",
        alt = escape(example.alt),
        code = code_block(&code, true),
    )
}

/// The source of `pub fn name`, from its signature to its closing brace.
fn function(source: &str, name: &str) -> String {
    let start = source.find(&format!("pub fn {name}(")).unwrap_or_else(|| panic!("no function {name} in snippets.rs"));
    let rest = &source[start..];
    let mut depth = 0;
    let mut end = rest.len();
    for (i, c) in rest.char_indices() {
        match c {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    end = i + 1;
                    break;
                }
            }
            _ => {}
        }
    }
    rest[..end].strip_prefix("pub ").unwrap_or(&rest[..end]).to_string()
}

fn code_block(code: &str, rust: bool) -> String {
    let body = if rust { highlight(code) } else { escape(code) };
    format!("<pre class=\"code\" tabindex=\"0\"><code>{body}</code></pre>")
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

const KEYWORDS: &[&str] = &[
    "use", "fn", "pub", "struct", "enum", "impl", "type", "let", "mut", "match", "self", "Self", "for", "in", "if", "else", "return", "mod", "move", "const", "as",
];

/// A small Rust highlighter: comments, strings, numbers, keywords, types and calls.
fn highlight(code: &str) -> String {
    let chars: Vec<char> = code.chars().collect();
    let mut out = String::new();
    let mut i = 0;
    let span = |out: &mut String, class: &str, text: &str| out.push_str(&format!("<span class=\"{class}\">{}</span>", escape(text)));
    while i < chars.len() {
        let c = chars[i];
        if c == '/' && chars.get(i + 1) == Some(&'/') {
            let end = chars[i..].iter().position(|&c| c == '\n').map_or(chars.len(), |p| i + p);
            span(&mut out, "c", &chars[i..end].iter().collect::<String>());
            i = end;
        } else if c == '"' {
            let mut end = i + 1;
            while end < chars.len() && chars[end] != '"' {
                end += if chars[end] == '\\' { 2 } else { 1 };
            }
            let end = (end + 1).min(chars.len());
            span(&mut out, "s", &chars[i..end].iter().collect::<String>());
            i = end;
        } else if c.is_ascii_digit() {
            let end = (i..chars.len()).find(|&j| !(chars[j].is_ascii_alphanumeric() || chars[j] == '.' || chars[j] == '_')).unwrap_or(chars.len());
            span(&mut out, "n", &chars[i..end].iter().collect::<String>());
            i = end;
        } else if c.is_alphabetic() || c == '_' {
            let end = (i..chars.len()).find(|&j| !(chars[j].is_alphanumeric() || chars[j] == '_')).unwrap_or(chars.len());
            let word: String = chars[i..end].iter().collect();
            let call = chars.get(end) == Some(&'(') || (chars.get(end) == Some(&'!') && chars.get(end + 1) == Some(&'('));
            if KEYWORDS.contains(&word.as_str()) {
                span(&mut out, "k", &word);
            } else if word.chars().next().is_some_and(char::is_uppercase) {
                span(&mut out, "t", &word);
            } else if call {
                span(&mut out, "f", &word);
            } else {
                out.push_str(&escape(&word));
            }
            i = end;
        } else {
            out.push_str(&escape(&c.to_string()));
            i += 1;
        }
    }
    out
}
