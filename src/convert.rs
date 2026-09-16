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
