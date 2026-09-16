//! Flavors on the clipboard, conversion targets, and the conversion itself.
use crate::{clipboard, markdown};

/// A representation the clipboard can hold.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Flavor { Text, Html, Rtf, Md }

impl Flavor {
    pub const ALL: [Flavor; 4] = [Flavor::Text, Flavor::Html, Flavor::Rtf, Flavor::Md];
    pub fn label(self) -> &'static str {
        match self { Flavor::Text => "plain text", Flavor::Html => "HTML", Flavor::Rtf => "RTF", Flavor::Md => "Markdown" }
    }
    pub fn pasted_by(self) -> &'static str {
        match self {
            Flavor::Text => "terminals, code editors, plain fields",
            Flavor::Html => "Mail, Notes, Slack, Outlook, Word, browsers (formatting kept)",
            Flavor::Rtf => "TextEdit, WordPad, Word when no HTML is present",
            Flavor::Md => "nobody: private ClipTo flavor holding the Markdown source, so `ct md` can restore it exactly",
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
            Target::Rich => "Markdown -> rich text (HTML + RTF + plain)",
            Target::Md => "rich text -> Markdown source",
            Target::Plain => "Markdown or rich text -> plain text",
            Target::Html => "Markdown -> HTML source",
            Target::Text => "keep only the plain-text flavor",
        }
    }
    /// Whether the conversion replaces the whole clipboard by default. Text-producing
    /// conversions only update the plain-text flavor and keep HTML/RTF, unless forced.
    pub fn replaces_all(self) -> bool { matches!(self, Target::Rich | Target::Text) }
    /// Which clipboard flavors to try first. The private Markdown flavor wins when present.
    pub fn prefer(self) -> [Flavor; 4] {
        match self {
            Target::Rich | Target::Html | Target::Text => [Flavor::Md, Flavor::Text, Flavor::Html, Flavor::Rtf],
            Target::Md | Target::Plain => [Flavor::Md, Flavor::Html, Flavor::Rtf, Flavor::Text],
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
            items.push((Flavor::Md, md.into_bytes()));
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
            Flavor::Text | Flavor::Md => if let Some(b) = clipboard::read(*f) {
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
            if has(Flavor::Md) {
                let same = clipboard::read(Flavor::Md).map(|b| b == text().into_bytes()).unwrap_or(false);
                return same.then(|| "the text flavor already is the Markdown source".to_string());
            }
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
    use super::*;

    const MD: &str = "# Update\n\n## Work in progress\n\n- **AFS+**: portable filesystem, runs as `C:Ferail` on AROS.\n- **Zed**: blocked on `mmap`.\n\n1. First\n2. Second\n\nSee [the site](https://aros.org) for more.\n";
    const HTML: &str = "<h2>Plan</h2><p>Some <b>bold</b>, <i>italic</i> and <a href=\"https://aros.org\">a link</a> with <code>code</code>.</p><ol><li>First</li><li>Second<ul><li>sub one</li><li>sub <b>two</b></li></ul></li></ol><pre>make -j8\n./configure</pre>";

    fn flavor(out: &Output, f: Flavor) -> Option<String> {
        out.items.iter().find(|(x, _)| *x == f).map(|(_, b)| String::from_utf8_lossy(b).into_owned())
    }

    #[test]
    fn markdown_to_rich_writes_every_flavor() {
        let out = convert(&Source::Markdown(MD.into()), Target::Rich);
        let html = flavor(&out, Flavor::Html).unwrap();
        assert!(html.starts_with("<!DOCTYPE html>"));
        assert!(html.contains("<h1>Update</h1>"));
        assert!(html.contains("<li><strong>AFS+</strong>: portable filesystem, runs as <code>C:Ferail</code> on AROS.</li>"));
        assert!(html.contains("<a href=\"https://aros.org\">the site</a>"));
        let plain = flavor(&out, Flavor::Text).unwrap();
        assert!(plain.starts_with("UPDATE\n"));
        assert!(plain.contains("• AFS+: portable filesystem, runs as C:Ferail on AROS.\n"));
        assert!(plain.contains("1. First\n2. Second\n"));
        assert!(plain.contains("See the site (https://aros.org) for more."));
        assert_eq!(flavor(&out, Flavor::Md).unwrap(), MD, "the source is kept in the private flavor");
        if cfg!(target_os = "macos") {
            let rtf = flavor(&out, Flavor::Rtf).expect("macOS produces RTF");
            assert!(rtf.starts_with("{\\rtf1"));
        }
    }

    #[test]
    fn markdown_to_html_and_plain() {
        let html = convert(&Source::Markdown(MD.into()), Target::Html).result;
        assert!(html.contains("<h2>Work in progress</h2>"));
        assert!(!html.contains("<!DOCTYPE"), "html target is the bare body");
        let plain = convert(&Source::Markdown(MD.into()), Target::Plain).result;
        assert!(!plain.contains('#') && !plain.contains("**") && !plain.contains('`'));
        let text = convert(&Source::Markdown(MD.into()), Target::Text).result;
        assert_eq!(text, MD, "text keeps Markdown text untouched");
    }

    #[test]
    fn html_to_markdown_plain_and_text() {
        let src = Source::Html(HTML.into());
        let md = convert(&src, Target::Md).result;
        assert!(md.contains("## Plan"));
        assert!(md.contains("Some **bold**, *italic* and [a link](https://aros.org) with `code`."));
        assert!(md.contains("1. First\n2. Second\n"));
        assert!(md.lines().any(|l| l.starts_with(" ") && l.trim_start() == "- sub one"), "{md}");
        assert!(md.contains("- sub **two**"));
        assert!(md.contains("```\nmake -j8\n./configure\n```"));
        let plain = convert(&src, Target::Plain).result;
        assert!(plain.contains("Some bold, italic and a link (https://aros.org) with code."));
        assert!(plain.contains("    make -j8\n    ./configure"));
        assert_eq!(convert(&src, Target::Text).result, plain, "text on a rich source is the plain rendering");
    }

    #[test]
    fn html_to_rich_rebuilds_from_markdown() {
        let out = convert(&Source::Html(HTML.into()), Target::Rich);
        let html = flavor(&out, Flavor::Html).unwrap();
        assert!(html.contains("<h2>Plan</h2>"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<ol>") && html.contains("<ul>"));
        assert!(html.contains("<pre><code>make -j8\n./configure"));
    }

    #[test]
    fn markdown_rich_markdown_round_trip_through_html() {
        let rich = convert(&Source::Markdown(MD.into()), Target::Rich);
        let html = flavor(&rich, Flavor::Html).unwrap();
        let back = convert(&Source::Html(html), Target::Md).result;
        assert!(back.contains("# Update\n\n## Work in progress"));
        assert!(back.contains("- **AFS+**: portable filesystem, runs as `C:Ferail` on AROS."));
        assert!(back.contains("1. First\n2. Second"));
        assert!(back.contains("[the site](https://aros.org)"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn rtf_round_trip_keeps_inline_formatting() {
        let rich = convert(&Source::Markdown(MD.into()), Target::Rich);
        let rtf = rich.items.iter().find(|(f, _)| *f == Flavor::Rtf).map(|(_, b)| b.clone()).unwrap();
        let html = clipboard::rich::rtf_to_html(&rtf).expect("AppKit converts RTF to HTML");
        let md = convert(&Source::Html(html), Target::Md).result;
        assert!(md.contains("**AFS+**"), "{md}");
        assert!(md.contains("`C:Ferail`") || md.contains("C:Ferail"), "{md}");
        assert!(md.contains("[the site](https://aros.org"), "{md}");
    }

    #[test]
    fn markdown_detection() {
        assert!(looks_like_markdown("# Title\n- **bold** item\n"));
        assert!(looks_like_markdown("runs as `C:Ferail` on AROS"));
        assert!(!looks_like_markdown("Kalamatee [1:20 AM] have a play with C:IPMI\nJohn made updates to AROS WIP.\n"));
        assert!(!looks_like_markdown("/Users/aros/Desktop/Screenshot.png"));
    }
}
