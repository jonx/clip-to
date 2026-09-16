//! Flavors on the clipboard, conversion targets, and the conversion itself.
use crate::{clipboard, markdown};

/// A representation the clipboard can hold.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flavor { Text, Html, Rtf }

impl Flavor {
    pub const ALL: [Flavor; 3] = [Flavor::Text, Flavor::Html, Flavor::Rtf];
    pub fn label(self) -> &'static str {
        match self { Flavor::Text => "plain text", Flavor::Html => "HTML", Flavor::Rtf => "RTF" }
    }
    pub fn pasted_by(self) -> &'static str {
        match self {
            Flavor::Text => "terminals, code editors, plain fields",
            Flavor::Html => "Mail, Notes, Slack, Outlook, Word, browsers (formatting kept)",
            Flavor::Rtf => "TextEdit, WordPad, Word when no HTML is present",
        }
    }
}

/// What a conversion produces.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Target { Rich, Md, Plain, Html, Text }

impl Target {
    pub const ALL: [Target; 5] = [Target::Rich, Target::Md, Target::Plain, Target::Html, Target::Text];

    pub fn parse(s: &str) -> Option<Target> {
        Some(match s {
            "rich" | "r" => Target::Rich,
            "md" | "m" | "markdown" => Target::Md,
            "plain" | "p" => Target::Plain,
            "html" | "h" => Target::Html,
            "text" | "t" => Target::Text,
            _ => return None,
        })
    }
    pub fn name(self) -> &'static str {
        match self { Target::Rich => "rich", Target::Md => "md", Target::Plain => "plain", Target::Html => "html", Target::Text => "text" }
    }
    pub fn title(self) -> &'static str {
        match self { Target::Rich => "Rich text", Target::Md => "Markdown", Target::Plain => "Plain text", Target::Html => "HTML source", Target::Text => "Text only" }
    }
    pub fn help(self) -> &'static str {
        match self {
            Target::Rich => "Markdown -> rich text (HTML + RTF + plain). Pasting keeps headings, bold, lists, code",
            Target::Md => "rich text (HTML/RTF copied from a page or a document) -> Markdown source in the text flavor",
            Target::Plain => "Markdown or rich text -> plain text in the text flavor, syntax stripped, bullets as \"•\"",
            Target::Html => "Markdown -> HTML source in the text flavor",
            Target::Text => "keep only the plain-text flavor (drop HTML/RTF)",
        }
    }
    /// Whether the conversion replaces the whole clipboard by default. Text-producing
    /// conversions only update the plain-text flavor and keep HTML/RTF, unless forced.
    pub fn replaces_all(self) -> bool { matches!(self, Target::Rich | Target::Text) }
    /// Which clipboard flavors to try first.
    pub fn prefer(self) -> [Flavor; 3] {
        match self {
            Target::Rich | Target::Html | Target::Text => [Flavor::Text, Flavor::Html, Flavor::Rtf],
            Target::Md | Target::Plain => [Flavor::Html, Flavor::Rtf, Flavor::Text],
        }
    }
}

/// What we read: either Markdown-ish text, or HTML from a rich source.
pub enum Source { Markdown(String), Html(String) }

impl Source {
    pub fn markdown(&self) -> String {
        match self { Source::Markdown(m) => m.clone(), Source::Html(h) => markdown::from_html(h) }
    }
    pub fn raw_text(&self) -> String {
        match self { Source::Markdown(m) => m.clone(), Source::Html(h) => markdown::to_plain(&markdown::from_html(h)) }
    }
}

pub struct Output {
    pub result: String,
    pub items: Vec<(Flavor, Vec<u8>)>,
}

pub fn convert(src: &Source, target: Target) -> Output {
    let text = |s: String| vec![(Flavor::Text, s.into_bytes())];
    match target {
        Target::Rich => {
            let md = src.markdown();
            let body = markdown::to_html(&md);
            let html = markdown::wrap_html(&body);
            let mut items = vec![(Flavor::Html, html.clone().into_bytes())];
            if let Some(rtf) = clipboard::rich::html_to_rtf(&html) { items.push((Flavor::Rtf, rtf)); }
            items.push((Flavor::Text, markdown::to_plain(&md).into_bytes()));
            Output { result: body + "\n", items }
        }
        Target::Md => { let r = src.markdown(); Output { items: text(r.clone()), result: r } }
        Target::Plain => { let r = markdown::to_plain(&src.markdown()); Output { items: text(r.clone()), result: r } }
        Target::Html => { let r = markdown::to_html(&src.markdown()) + "\n"; Output { items: text(r.clone()), result: r } }
        Target::Text => { let r = src.raw_text(); Output { items: text(r.clone()), result: r } }
    }
}

/// Read the clipboard, trying flavors in order, and convert. Returns None when nothing is usable.
pub fn read_source(prefer: &[Flavor]) -> Option<Source> {
    for f in prefer {
        match f {
            Flavor::Html => if let Some(b) = clipboard::read(Flavor::Html) {
                if let Ok(s) = String::from_utf8(b) { return Some(Source::Html(s)); }
            },
            Flavor::Rtf => if let Some(b) = clipboard::read(Flavor::Rtf) {
                if let Some(h) = clipboard::rich::rtf_to_html(&b) { return Some(Source::Html(h)); }
            },
            Flavor::Text => if let Some(b) = clipboard::read(Flavor::Text) {
                if let Ok(s) = String::from_utf8(b) { return Some(Source::Markdown(s)); }
            },
        }
    }
    None
}

/// Cheap check for Markdown syntax: headings, emphasis, list markers, code, links.
pub fn looks_like_markdown(text: &str) -> bool {
    let mut lines = 0;
    let mut hits = 0;
    for line in text.lines() {
        let t = line.trim_start();
        if t.is_empty() { continue; }
        lines += 1;
        if t.starts_with('#') || t.starts_with("- ") || t.starts_with("* ") || t.starts_with("> ") || t.starts_with("```")
            || t.starts_with("1. ") || t.starts_with("| ")
        { hits += 1; }
        if t.contains("**") || t.contains('`') || (t.contains("](") && t.contains('[')) { hits += 1; }
    }
    hits > 0 && (lines <= 3 || hits * 4 >= lines)
}

fn looks_like_html(text: &str) -> bool {
    let t = text.trim_start();
    t.starts_with('<') && text.contains("</")
}

/// When the clipboard already holds what `target` would produce, say why and skip the conversion.
pub fn already_satisfied(target: Target) -> Option<String> {
    let present = clipboard::present();
    let has = |f: Flavor| present.contains(&f);
    let text = || clipboard::read(Flavor::Text).and_then(|b| String::from_utf8(b).ok()).unwrap_or_default();
    match target {
        Target::Rich => has(Flavor::Html).then(|| "HTML is already on the clipboard".to_string()),
        Target::Md => {
            if !has(Flavor::Text) { return None; }
            if !has(Flavor::Html) && !has(Flavor::Rtf) { return Some("no rich flavor, the text is taken as is".into()); }
            looks_like_markdown(&text()).then(|| "the text flavor already looks like Markdown".to_string())
        }
        Target::Plain => (has(Flavor::Text) && !has(Flavor::Html) && !has(Flavor::Rtf) && !looks_like_markdown(&text()))
            .then(|| "the text has no Markdown syntax".to_string()),
        Target::Html => (has(Flavor::Text) && looks_like_html(&text())).then(|| "the text flavor already is HTML source".to_string()),
        Target::Text => (present == [Flavor::Text]).then(|| "only plain text is on the clipboard".to_string()),
    }
}

#[cfg(test)]
mod tests {
    use super::looks_like_markdown;
    #[test]
    fn markdown_detection() {
        assert!(looks_like_markdown("# Title\n- **bold** item\n"));
        assert!(looks_like_markdown("runs as `C:Ferail` on AROS"));
        assert!(!looks_like_markdown("Kalamatee [1:20 AM] have a play with C:IPMI\nJohn made updates to AROS WIP.\n"));
        assert!(!looks_like_markdown("/Users/aros/Desktop/Screenshot.png"));
    }
}
