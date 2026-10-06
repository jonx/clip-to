//! Translate clipboard CSS emphasis and Office list paragraphs into Markdown.
//! This handles text semantics, not page layout or the full CSS cascade.
use htmd::{Element, Node, element_handler::{HandlerResult, Handlers}};
use markup5ever_rcdom::NodeData;
use std::collections::HashMap;
use std::rc::Rc;

pub type Styles = HashMap<String, String>;

fn attr(element: &Element<'_>, name: &str) -> String {
    element.attrs.iter().find(|a| a.name.local.as_ref() == name).map(|a| a.value.to_string()).unwrap_or_default()
}

fn text(node: &Rc<Node>) -> String {
    if let NodeData::Text { contents } = &node.data { return contents.borrow().to_string(); }
    node.children.borrow().iter().map(text).collect()
}

/// Office commonly puts emphasis in simple class rules in a <style> element.
pub fn styles(tree: &Rc<Node>) -> Styles {
    fn visit(node: &Rc<Node>, result: &mut Styles) {
        if let NodeData::Element { name, .. } = &node.data {
            if name.local.as_ref() == "style" {
                let css = text(node);
                for rule in css.split('}') {
                    let Some((selectors, body)) = rule.rsplit_once('{') else { continue };
                    for selector in selectors.split(',').map(str::trim) {
                        // Simple selectors only: .class, span.class, p.class.
                        if selector.contains([' ', ':', '>', '[', '#', '\n']) { continue; }
                        let Some((tag, class)) = selector.split_once('.') else { continue };
                        if class.is_empty() || class.contains('.') { continue; }
                        let key = format!("{tag}.{class}");
                        result.entry(key).or_default().push_str(&format!(";{body}"));
                    }
                }
            }
        }
        for child in node.children.borrow().iter() { visit(child, result); }
    }
    let mut result = Styles::new();
    visit(tree, &mut result);
    result
}

fn declarations(style: &str) -> HashMap<String, String> {
    style.split(';').filter_map(|s| s.split_once(':')).map(|(k, v)| {
        (k.trim().to_ascii_lowercase(), v.trim().trim_end_matches("!important").trim().to_ascii_lowercase())
    }).collect()
}

fn wrap(content: String, delimiter: &str) -> String {
    let core = content.trim();
    if core.is_empty() { return content; }
    let start = content.len() - content.trim_start().len();
    let end = content.trim_end().len();
    format!("{}{delimiter}{core}{delimiter}{}", &content[..start], &content[end..])
}

fn list_marker(node: &Rc<Node>) -> Option<String> {
    if let NodeData::Element { attrs, .. } = &node.data {
        let attrs = attrs.borrow();
        let style = attrs.iter().find(|a| a.name.local.as_ref() == "style").map(|a| declarations(&a.value)).unwrap_or_default();
        if style.get("mso-list").map(String::as_str) == Some("ignore") { return Some(text(node)); }
    }
    node.children.borrow().iter().find_map(list_marker)
}

pub fn handler(styles: Styles) -> impl Fn(&dyn Handlers, Element<'_>) -> Option<HandlerResult> + Send + Sync {
    move |handlers, element| {
        let mut css = String::new();
        for class in attr(&element, "class").split_whitespace() {
            for selector in [format!(".{class}"), format!("{}.{class}", element.tag)] {
                if let Some(rule) = styles.get(&selector) { css.push_str(rule); css.push(';'); }
            }
        }
        css.push_str(&attr(&element, "style"));
        let properties = declarations(&css);
        let list = properties.get("mso-list").map(String::as_str).unwrap_or("");
        if list == "ignore" { return None; } // numbering is emitted by the parent paragraph
        let bold = properties.get("font-weight").map(|v| v == "bold" || v == "bolder" || v.parse::<u16>().map(|n| n >= 600).unwrap_or(false)).unwrap_or(false)
            && !matches!(element.tag, "b" | "strong");
        let italic = properties.get("font-style").map(|v| v == "italic" || v.starts_with("oblique")).unwrap_or(false)
            && !matches!(element.tag, "i" | "em");
        let strike = properties.get("text-decoration").or_else(|| properties.get("text-decoration-line"))
            .map(|v| v.contains("line-through")).unwrap_or(false) && !matches!(element.tag, "s" | "del" | "strike");
        let is_list = element.tag == "p" && list.split_whitespace().any(|v| v.starts_with("level"));
        if is_list {
            let level = list.split_whitespace().find_map(|v| v.strip_prefix("level")?.parse::<usize>().ok()).unwrap_or(1).clamp(1, 32);
            let marker = list_marker(element.node).unwrap_or_default();
            let digits: String = marker.trim().chars().take_while(char::is_ascii_digit).collect();
            let marker = if digits.is_empty() { "-".into() } else { format!("{digits}.") };
            let mut content = handlers.walk_children(element.node).content.trim().to_string();
            if bold { content = wrap(content, "**"); }
            if italic { content = wrap(content, "*"); }
            if strike { content = wrap(content, "~~"); }
            return Some(format!("\n{}{} {}\n", "    ".repeat(level - 1), marker, content).into());
        }
        let mut result = handlers.fallback(element)?;
        if bold { result.content = wrap(result.content, "**"); }
        if italic { result.content = wrap(result.content, "*"); }
        if strike { result.content = wrap(result.content, "~~"); }
        Some(result)
    }
}
