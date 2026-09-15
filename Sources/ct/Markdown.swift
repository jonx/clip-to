import Foundation

// A small Markdown implementation covering what people paste: headings, bullet and
// numbered lists (nested), emphasis, strikethrough, inline and fenced code, links,
// images, block quotes and horizontal rules.

extension String {
    func replacing(_ pattern: String, with template: String) -> String {
        let re = try! NSRegularExpression(pattern: pattern)
        return re.stringByReplacingMatches(in: self, range: NSRange(startIndex..., in: self), withTemplate: template)
    }
    func firstMatch(_ pattern: String) -> [String]? {
        let re = try! NSRegularExpression(pattern: pattern)
        guard let m = re.firstMatch(in: self, range: NSRange(startIndex..., in: self)) else { return nil }
        return (0..<m.numberOfRanges).map { i in
            let r = m.range(at: i)
            return r.location == NSNotFound ? "" : String(self[Range(r, in: self)!])
        }
    }
    func matches(_ pattern: String) -> Bool { firstMatch(pattern) != nil }
    var trimmed: String { trimmingCharacters(in: .whitespaces) }
    var escapedHTML: String {
        replacingOccurrences(of: "&", with: "&amp;")
            .replacingOccurrences(of: "<", with: "&lt;")
            .replacingOccurrences(of: ">", with: "&gt;")
    }
}

enum Markdown {
    // MARK: Markdown -> HTML

    static func toHTML(_ md: String) -> String {
        let lines = md.replacingOccurrences(of: "\r\n", with: "\n").components(separatedBy: "\n")
        var out: [String] = []
        var lists: [(indent: Int, tag: String)] = []
        var para: [String] = []
        var i = 0

        func flushPara() {
            if !para.isEmpty {
                out.append("<p>" + inline(para.map { $0.trimmed }.joined(separator: " ")) + "</p>")
                para.removeAll()
            }
        }
        func closeLists(to indent: Int = -1) {
            while let last = lists.last, last.indent > indent {
                out.append("</li></\(last.tag)>"); lists.removeLast()
            }
        }

        while i < lines.count {
            let line = lines[i]
            let stripped = line.trimmed
            if let m = line.firstMatch(#"^\s*```(\w*)"#) {
                flushPara(); closeLists()
                var buf: [String] = []
                i += 1
                while i < lines.count && !lines[i].matches(#"^\s*```"#) { buf.append(lines[i]); i += 1 }
                i += 1
                let cls = m[1].isEmpty ? "" : " class=\"language-\(m[1])\""
                out.append("<pre><code\(cls)>" + buf.joined(separator: "\n").escapedHTML + "</code></pre>")
                continue
            }
            if stripped.isEmpty { flushPara(); closeLists(); i += 1; continue }
            if let m = line.firstMatch(#"^(#{1,6})\s+(.*?)\s*#*\s*$"#) {
                flushPara(); closeLists()
                let n = m[1].count
                out.append("<h\(n)>" + inline(m[2]) + "</h\(n)>")
                i += 1; continue
            }
            if line.matches(#"^\s*([-*_])(\s*\1){2,}\s*$"#) {
                flushPara(); closeLists(); out.append("<hr>"); i += 1; continue
            }
            if let m = line.firstMatch(#"^(\s*)([-*+]|\d+[.)])\s+(.*)$"#) {
                flushPara()
                let indent = m[1].replacingOccurrences(of: "\t", with: "    ").count
                let tag = m[2].first!.isNumber ? "ol" : "ul"
                let text = inline(m[3])
                if let last = lists.last, indent > last.indent {
                    out.append("<\(tag)><li>" + text); lists.append((indent, tag))
                } else {
                    closeLists(to: indent)
                    if let last = lists.last, last.indent == indent {
                        out.append("</li><li>" + text)
                    } else {
                        out.append("<\(tag)><li>" + text); lists.append((indent, tag))
                    }
                }
                i += 1; continue
            }
            if line.matches(#"^\s*>"#) {
                flushPara(); closeLists()
                var buf: [String] = []
                while i < lines.count && lines[i].matches(#"^\s*>"#) {
                    buf.append(lines[i].replacing(#"^\s*>\s?"#, with: "")); i += 1
                }
                out.append("<blockquote>" + toHTML(buf.joined(separator: "\n")) + "</blockquote>")
                continue
            }
            if !lists.isEmpty && line.hasPrefix(" ") {
                out.append(" " + inline(stripped)); i += 1; continue
            }
            closeLists()
            para.append(line); i += 1
        }
        flushPara(); closeLists()
        return out.joined(separator: "\n")
    }

    static func inline(_ raw: String) -> String {
        let s = raw.escapedHTML
        var out = ""
        var rest = Substring(s)
        while let open = rest.firstIndex(of: "`") {
            let before = rest[rest.startIndex..<open]
            let afterOpen = rest[rest.index(after: open)...]
            if let close = afterOpen.firstIndex(of: "`") {
                out += marks(String(before))
                out += "<code>" + afterOpen[afterOpen.startIndex..<close] + "</code>"
                rest = afterOpen[afterOpen.index(after: close)...]
            } else { break }
        }
        out += marks(String(rest))
        return out
    }

    private static func marks(_ p: String) -> String {
        var s = p
        s = s.replacing(#"!\[([^\]]*)\]\(([^)\s]+)\)"#, with: #"<img alt="$1" src="$2">"#)
        s = s.replacing(#"\[([^\]]+)\]\(([^)\s]+)\)"#, with: #"<a href="$2">$1</a>"#)
        s = s.replacing(#"\*\*(.+?)\*\*"#, with: "<strong>$1</strong>")
        s = s.replacing(#"__(.+?)__"#, with: "<strong>$1</strong>")
        s = s.replacing(#"(?<![\w*])\*(?!\s)(.+?)(?<!\s)\*(?![\w*])"#, with: "<em>$1</em>")
        s = s.replacing(#"(?<![\w_])_(?!\s)(.+?)(?<!\s)_(?![\w_])"#, with: "<em>$1</em>")
        s = s.replacing(#"~~(.+?)~~"#, with: "<del>$1</del>")
        s = s.replacing(#"&lt;(https?://[^\s&]+)&gt;"#, with: #"<a href="$1">$1</a>"#)
        return s
    }

    static func wrapHTML(_ body: String) -> String {
        """
        <!DOCTYPE html><html><head><meta charset="utf-8"><style>
        body{font-family:-apple-system,Helvetica,Arial,sans-serif;font-size:13px}
        code,pre{font-family:Menlo,monospace;font-size:12px}
        </style></head><body>\(body)</body></html>
        """
    }

    // MARK: Markdown -> plain text

    static func toPlain(_ md: String) -> String {
        var out: [String] = []
        var inFence = false
        for var line in md.replacingOccurrences(of: "\r\n", with: "\n").components(separatedBy: "\n") {
            if line.matches(#"^\s*```"#) { inFence.toggle(); continue }
            if inFence { out.append("    " + line); continue }
            if let m = line.firstMatch(#"^(#{1,6})\s+(.*?)\s*#*\s*$"#) {
                line = m[1].count == 1 ? m[2].uppercased() : m[2]
            }
            line = line.replacing(#"^(\s*)[-*+]\s+"#, with: "$1• ")
            line = line.replacing(#"^\s*>\s?"#, with: "")
            if line.matches(#"^\s*([-*_])(\s*\1){2,}\s*$"#) { line = String(repeating: "—", count: 20) }
            line = line.replacing(#"`([^`]+)`"#, with: "$1")
            line = line.replacing(#"!\[([^\]]*)\]\([^)]*\)"#, with: "$1")
            line = line.replacing(#"\[([^\]]+)\]\(([^)\s]+)\)"#, with: "$1 ($2)")
            line = line.replacing(#"\*\*(.+?)\*\*"#, with: "$1")
            line = line.replacing(#"__(.+?)__"#, with: "$1")
            line = line.replacing(#"(?<![\w*])\*(?!\s)(.+?)(?<!\s)\*(?![\w*])"#, with: "$1")
            line = line.replacing(#"(?<![\w_])_(?!\s)(.+?)(?<!\s)_(?![\w_])"#, with: "$1")
            line = line.replacing(#"~~(.+?)~~"#, with: "$1")
            out.append(line)
        }
        return out.joined(separator: "\n").trimmingCharacters(in: .whitespacesAndNewlines) + "\n"
    }
}
