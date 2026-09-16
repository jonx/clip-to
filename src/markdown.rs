//! Markdown <-> HTML <-> plain text. Pure Rust, shared by every platform.
//!
//! Markdown -> HTML uses pulldown-cmark (CommonMark + tables, strikethrough, task lists).
//! HTML -> Markdown uses htmd. Markdown -> plain text walks the pulldown-cmark events.

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

fn options() -> Options {
    Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS
}

/// Raw HTML inside Markdown is treated as text, so `<name>` shows up instead of vanishing.
fn events(md: &str) -> impl Iterator<Item = Event<'_>> {
    Parser::new_ext(md, options()).map(|e| match e {
        Event::Html(s) | Event::InlineHtml(s) => Event::Text(s),
        other => other,
    })
}

pub fn to_html(md: &str) -> String {
    let mut out = String::new();
    pulldown_cmark::html::push_html(&mut out, events(md));
    out
}

pub fn wrap_html(body: &str) -> String {
    format!(
        "<!DOCTYPE html><html><head><meta charset=\"utf-8\"><style>\n\
         body{{font-family:-apple-system,Segoe UI,Helvetica,Arial,sans-serif;font-size:13px}}\n\
         code,pre{{font-family:Menlo,Consolas,monospace;font-size:12px}}\n\
         </style></head><body>{body}</body></html>"
    )
}

pub fn from_html(html: &str) -> String {
    use htmd::options::{BulletListMarker, CodeBlockFence, CodeBlockStyle, HrStyle, Options as HOptions};
    let conv = htmd::HtmlToMarkdown::builder()
        .skip_tags(vec!["script", "style", "head", "title", "meta", "link"])
        .options(HOptions {
            bullet_list_marker: BulletListMarker::Dash,
            code_block_style: CodeBlockStyle::Fenced,
            code_block_fence: CodeBlockFence::Backticks,
            hr_style: HrStyle::Dashes,
            ul_bullet_spacing: 1,
            ol_number_spacing: 1,
            ..Default::default()
        })
        .build();
    let md = conv.convert(&code_in_pre(html)).unwrap_or_default();
    normalize(&md)
}

/// htmd only fences `<pre><code>`; bare `<pre>` blocks (common in copied HTML) get a `<code>` wrapper.
fn code_in_pre(html: &str) -> String {
    let mut out = String::with_capacity(html.len() + 32);
    let mut rest = html;
    let lower = html.to_ascii_lowercase();
    let mut idx = 0;
    while let Some(start) = lower[idx..].find("<pre") {
        let start = idx + start;
        let Some(tag_end) = lower[start..].find('>') else { break };
        let tag_end = start + tag_end + 1;
        let Some(end) = lower[tag_end..].find("</pre>") else { break };
        let end = tag_end + end;
        let inner = &html[tag_end..end];
        out.push_str(&rest[..tag_end - idx]);
        if inner.trim_start().to_ascii_lowercase().starts_with("<code") {
            out.push_str(inner);
        } else {
            out.push_str("<code>");
            out.push_str(inner);
            out.push_str("</code>");
        }
        rest = &html[end..];
        idx = end;
    }
    out.push_str(rest);
    out
}

/// Collapse runs of blank lines and trailing spaces; end with exactly one newline.
fn normalize(s: &str) -> String {
    let mut out = String::new();
    let mut blank = 0;
    for line in s.lines() {
        let l = line.trim_end();
        if l.is_empty() {
            blank += 1;
            if blank > 1 { continue; }
        } else {
            blank = 0;
        }
        out.push_str(l);
        out.push('\n');
    }
    out.trim_matches('\n').to_string() + "\n"
}

/// Markdown -> readable plain text: headings kept (H1 upper-cased), bullets as "•",
/// numbered lists numbered, links as "text (url)", code indented, emphasis dropped.
pub fn to_plain(md: &str) -> String {
    let mut out = String::new();
    let mut lists: Vec<Option<u64>> = Vec::new();
    let mut heading: Option<HeadingLevel> = None;
    let mut in_code = false;
    let mut link: Option<String> = None;
    let mut in_image = false;

    fn ensure_newline(out: &mut String) {
        if !out.is_empty() && !out.ends_with('\n') { out.push('\n'); }
    }
    fn ensure_blank(out: &mut String) {
        ensure_newline(out);
        if !out.is_empty() && !out.ends_with("\n\n") { out.push('\n'); }
    }

    for ev in events(md) {
        match ev {
            Event::Start(Tag::Heading { level, .. }) => { ensure_blank(&mut out); heading = Some(level); }
            Event::End(TagEnd::Heading(_)) => { out.push('\n'); heading = None; }
            Event::Start(Tag::Paragraph) => { if lists.is_empty() { ensure_blank(&mut out); } }
            Event::End(TagEnd::Paragraph) => { ensure_newline(&mut out); }
            Event::Start(Tag::List(start)) => { if lists.is_empty() { ensure_blank(&mut out); } lists.push(start); }
            Event::End(TagEnd::List(_)) => { lists.pop(); if lists.is_empty() { ensure_newline(&mut out); } }
            Event::Start(Tag::Item) => {
                ensure_newline(&mut out);
                let depth = lists.len().saturating_sub(1);
                out.push_str(&"  ".repeat(depth));
                match lists.last_mut() {
                    Some(Some(n)) => { out.push_str(&format!("{n}. ")); *n += 1; }
                    _ => out.push_str("• "),
                }
            }
            Event::End(TagEnd::Item) => { ensure_newline(&mut out); }
            Event::Start(Tag::CodeBlock(_)) => { ensure_blank(&mut out); in_code = true; }
            Event::End(TagEnd::CodeBlock) => { in_code = false; ensure_newline(&mut out); }
            Event::Start(Tag::Link { dest_url, .. }) => { link = Some(dest_url.to_string()); }
            Event::End(TagEnd::Link) => {
                if let Some(u) = link.take() { out.push_str(&format!(" ({u})")); }
            }
            Event::Start(Tag::Image { .. }) => { in_image = true; }
            Event::End(TagEnd::Image) => { in_image = false; }
            Event::Text(t) => {
                if in_image { out.push_str(&t); continue; }
                if in_code {
                    for line in t.lines() { out.push_str("    "); out.push_str(line); out.push('\n'); }
                } else if heading == Some(HeadingLevel::H1) {
                    out.push_str(&t.to_uppercase());
                } else {
                    out.push_str(&t);
                }
            }
            Event::Code(c) => out.push_str(&c),
            Event::SoftBreak => out.push(' '),
            Event::HardBreak => out.push('\n'),
            Event::Rule => { ensure_blank(&mut out); out.push_str(&"—".repeat(20)); out.push('\n'); }
            Event::TaskListMarker(done) => out.push_str(if done { "[x] " } else { "[ ] " }),
            Event::Start(Tag::TableCell) => { out.push_str("| "); }
            Event::End(TagEnd::TableCell) => { out.push(' '); }
            Event::End(TagEnd::TableRow) | Event::End(TagEnd::TableHead) => { out.push_str("|\n"); }
            _ => {}
        }
    }
    out.trim_matches('\n').to_string() + "\n"
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "# AROS work update\n## jonx — work in progress\n- **AFS+**: portable filesystem. Runs as `C:Ferail` on AROS.\n- **Zed**: blocked on `mmap`.\n## <name> — work in progress\n- ...\n";

    #[test]
    fn html_keeps_structure_and_escapes_raw_tags() {
        let h = to_html(SAMPLE);
        assert!(h.contains("<h1>AROS work update</h1>"));
        assert!(h.contains("<li><strong>AFS+</strong>: portable filesystem. Runs as <code>C:Ferail</code> on AROS.</li>"));
        assert!(h.contains("&lt;name&gt;"), "raw html must be escaped: {h}");
    }

    #[test]
    fn plain_strips_syntax() {
        let p = to_plain(SAMPLE);
        assert!(p.starts_with("AROS WORK UPDATE\n"));
        assert!(p.contains("• AFS+: portable filesystem. Runs as C:Ferail on AROS.\n"));
        assert!(!p.contains("**"));
    }

    #[test]
    fn round_trip_markdown_html_markdown() {
        let md = from_html(&to_html(SAMPLE));
        assert!(md.contains("# AROS work update"));
        assert!(md.contains("- **AFS+**: portable filesystem. Runs as `C:Ferail` on AROS."));
        assert!(md.contains("## \\<name> — work in progress"), "{md}");
    }

    #[test]
    fn nested_and_ordered_lists_from_html() {
        let html = "<h2>Plan</h2><p>Some <b>bold</b> and <a href=\"https://aros.org\">a link</a> with <code>code</code>.</p>\
                    <ol><li>First</li><li>Second<ul><li>sub one</li><li>sub <b>two</b></li></ul></li><li>Third</li></ol>\
                    <pre>make -j8\n./configure</pre><p>Done.</p>";
        let md = from_html(html);
        assert!(md.contains("## Plan"));
        assert!(md.contains("Some **bold** and [a link](https://aros.org) with `code`."));
        assert!(md.contains("1. First\n2. Second"));
        assert!(md.contains("- sub one"));
        assert!(md.contains("```\nmake -j8\n./configure\n```"), "{md}");
        let plain = to_plain(&md);
        assert!(plain.contains("1. First\n2. Second\n"));
        assert!(plain.contains("a link (https://aros.org)"));
    }
}
