import AppKit

enum Source {
    case markdown(String)
    case rich(NSAttributedString)

    var markdown: String {
        switch self {
        case .markdown(let m): return m
        case .rich(let a): return RichText.toMarkdown(a)
        }
    }
    var rawText: String {
        switch self {
        case .markdown(let m): return m
        case .rich(let a): return a.string
        }
    }
}

enum Flavor: String, CaseIterable {
    case text, html, rtf
    var type: NSPasteboard.PasteboardType {
        switch self {
        case .text: return .string
        case .html: return .html
        case .rtf: return .rtf
        }
    }
    var label: String {
        switch self {
        case .text: return "plain text"
        case .html: return "HTML"
        case .rtf: return "RTF"
        }
    }
    var pastedBy: String {
        switch self {
        case .text: return "Terminal, code editors, plain fields"
        case .html: return "Mail, Notes, Slack, browsers, Pages (formatting kept)"
        case .rtf: return "TextEdit, Word, Pages when no HTML is present"
        }
    }
}

/// What a conversion produces: a target format.
enum Target: String, CaseIterable {
    case rich, md, plain, html, text

    var title: String {
        switch self {
        case .rich: return "Rich text"
        case .md: return "Markdown"
        case .plain: return "Plain text"
        case .html: return "HTML source"
        case .text: return "Text only"
        }
    }
    var help: String {
        switch self {
        case .rich: return "Markdown -> rich text (HTML + RTF + plain). Pasting keeps headings, bold, lists, code"
        case .md: return "rich text (HTML/RTF copied from a page or a document) -> Markdown source"
        case .plain: return "Markdown or rich text -> plain text, syntax stripped, bullets as \"•\""
        case .html: return "Markdown -> HTML source, as plain text"
        case .text: return "keep only the plain-text flavor (drop HTML/RTF)"
        }
    }
    /// Which clipboard flavors to try first.
    var prefer: [Flavor] {
        switch self {
        case .rich, .html, .text: return [.text, .html, .rtf]
        case .md, .plain: return [.html, .rtf, .text]
        }
    }
}

enum Clipboard {
    static let pb = NSPasteboard.general

    static var types: [String] { (pb.types ?? []).map { $0.rawValue } }
    static var present: [Flavor] { Flavor.allCases.filter { pb.data(forType: $0.type) != nil } }

    static func read(prefer: [Flavor]) -> Source? {
        for f in prefer {
            switch f {
            case .html: if let s = pb.string(forType: .html), let a = RichText.fromHTML(s) { return .rich(a) }
            case .rtf: if let d = pb.data(forType: .rtf), let a = RichText.fromRTF(d) { return .rich(a) }
            case .text: if let s = pb.string(forType: .string) { return .markdown(s) }
            }
        }
        return nil
    }

    static func write(_ items: [(NSPasteboard.PasteboardType, Data)]) {
        pb.clearContents()
        pb.declareTypes(items.map { $0.0 }, owner: nil)
        for (t, d) in items { pb.setData(d, forType: t) }
    }

    /// Convert a source to a target. Returns the textual result and the flavors to put on the clipboard.
    static func convert(_ src: Source, to target: Target) -> (result: String, items: [(NSPasteboard.PasteboardType, Data)]) {
        switch target {
        case .rich:
            let md = src.markdown
            let body = Markdown.toHTML(md)
            let html = Markdown.wrapHTML(body)
            var items: [(NSPasteboard.PasteboardType, Data)] = [(.html, html.data(using: .utf8)!)]
            if let rtf = RichText.htmlToRTF(html) { items.append((.rtf, rtf)) }
            items.append((.string, Markdown.toPlain(md).data(using: .utf8)!))
            return (body + "\n", items)
        case .md:
            let r = src.markdown
            return (r, [(.string, r.data(using: .utf8)!)])
        case .plain:
            let r = Markdown.toPlain(src.markdown)
            return (r, [(.string, r.data(using: .utf8)!)])
        case .html:
            let r = Markdown.toHTML(src.markdown) + "\n"
            return (r, [(.string, r.data(using: .utf8)!)])
        case .text:
            let r = src.rawText
            return (r, [(.string, r.data(using: .utf8)!)])
        }
    }
}
