//! Fills the page templates in `site/src` and writes the pages to `site/`.

use crate::{COUNTER, SNIPPETS, Source, examples};
use std::path::Path;

/// Pages in sidebar order: file name without extension, title.
const PAGES: &[(&str, &str)] = &[
    ("index", "Getting started"),
    ("layout", "Layout and styling"),
    ("text", "Text and images"),
    ("buttons", "Buttons and toolbars"),
    ("controls", "Controls"),
    ("fields", "Text fields"),
    ("lists", "Lists"),
    ("menus", "Menus and dialogs"),
    ("theme", "Theme and accessibility"),
];

pub fn build(out: &Path) {
    let layout = std::fs::read_to_string(out.join("src/_layout.html")).expect("read site/src/_layout.html");
    let mut index = Vec::new();
    for (slug, title) in PAGES {
        let template = std::fs::read_to_string(out.join(format!("src/{slug}.html"))).unwrap_or_else(|e| panic!("read site/src/{slug}.html: {e}"));
        let content = add_heading_ids(&fill(&template));
        index_page(&mut index, slug, title, &content);
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
    std::fs::write(out.join("search.json"), format!("[{}]\n", index.join(","))).expect("write search.json");
    println!("wrote search.json ({} entries)", index.len());
}

fn slugify(text: &str) -> String {
    let mut id = String::new();
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            id.push(c.to_ascii_lowercase());
        } else if !id.ends_with('-') && !id.is_empty() {
            id.push('-');
        }
    }
    id.trim_end_matches('-').to_string()
}

/// Gives every `<h2>` an id, so search results can link to it.
fn add_heading_ids(content: &str) -> String {
    let mut out = String::new();
    let mut rest = content;
    while let Some(start) = rest.find("<h2>") {
        let end = rest[start..].find("</h2>").expect("unclosed h2") + start;
        let title = &rest[start + 4..end];
        out.push_str(&rest[..start]);
        out.push_str(&format!("<h2 id=\"{}\">{title}</h2>", slugify(&strip_tags(title))));
        rest = &rest[end + 5..];
    }
    out.push_str(rest);
    out
}

fn strip_tags(html: &str) -> String {
    let mut out = String::new();
    let mut in_tag = false;
    for c in html.chars() {
        match c {
            '<' => in_tag = true,
            '>' => in_tag = false,
            c if !in_tag => out.push(c),
            _ => {}
        }
    }
    out.replace("&amp;", "&").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"")
}

fn json(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', " "))
}

/// One search entry for the page's intro and one for each section: its heading
/// and the first paragraph under it.
fn index_page(index: &mut Vec<String>, slug: &str, title: &str, content: &str) {
    let first_paragraph = |html: &str| html.find("<p>").and_then(|s| html[s..].find("</p>").map(|e| strip_tags(&html[s..s + e]))).unwrap_or_default();
    let mut sections = content.split("<h2 id=\"");
    let intro = sections.next().unwrap_or_default();
    let href = if slug == "index" { "./".to_string() } else { format!("{slug}.html") };
    index.push(format!("{{\"page\":{},\"heading\":\"\",\"href\":{},\"text\":{}}}", json(title), json(&href), json(&first_paragraph(intro))));
    for section in sections {
        let (id, rest) = section.split_once("\">").expect("heading id");
        let (heading, body) = rest.split_once("</h2>").expect("heading end");
        index.push(format!(
            "{{\"page\":{},\"heading\":{},\"href\":{},\"text\":{}}}",
            json(title),
            json(&strip_tags(heading)),
            json(&format!("{href}#{id}")),
            json(&first_paragraph(body))
        ));
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
