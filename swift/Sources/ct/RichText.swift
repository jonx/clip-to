import AppKit

/// Rich text (NSAttributedString) <-> HTML / RTF / Markdown, all in-process via AppKit.
enum RichText {
    static func fromHTML(_ html: String) -> NSAttributedString? {
        guard let data = html.data(using: .utf8) else { return nil }
        return NSAttributedString(html: data, options: [.characterEncoding: String.Encoding.utf8.rawValue], documentAttributes: nil)
    }

    static func fromRTF(_ rtf: Data) -> NSAttributedString? {
        NSAttributedString(rtf: rtf, documentAttributes: nil)
    }

    static func htmlToRTF(_ html: String) -> Data? {
        guard let a = fromHTML(html) else { return nil }
        return a.rtf(from: NSRange(location: 0, length: a.length),
                     documentAttributes: [.documentType: NSAttributedString.DocumentType.rtf])
    }

    /// Walk paragraphs and attribute runs and emit Markdown. Uses the paragraph style's
    /// header level and text lists, font traits for bold/italic/monospace, and link attributes.
    static func toMarkdown(_ a: NSAttributedString) -> String {
        let text = a.string as NSString
        var out: [String] = []
        var prevWasList = false
        var olCounters: [Int] = []
        var codeBlock: [String] = []

        func flushCode() {
            if !codeBlock.isEmpty {
                out.append("```\n" + codeBlock.joined(separator: "\n") + "\n```\n")
                codeBlock.removeAll()
            }
        }

        var pos = 0
        while pos < text.length {
            let paraRange = text.paragraphRange(for: NSRange(location: pos, length: 0))
            pos = paraRange.location + paraRange.length
            var body = text.substring(with: paraRange)
            if body.hasSuffix("\n") { body.removeLast() }
            if body.hasSuffix("\r") { body.removeLast() }
            let style = (paraRange.length > 0 ? a.attribute(.paragraphStyle, at: paraRange.location, effectiveRange: nil) : nil) as? NSParagraphStyle
            let headerLevel = style?.headerLevel ?? 0
            let lists = style?.textLists ?? []

            // Skip the list marker text AppKit inserts ("\t•\t", "\t1\t") before styling runs.
            var skip = 0
            if !lists.isEmpty, let m = body.firstMatch(#"^\s*(?:[•◦▪\-*]|\d+[.)]?|[a-zA-Z][.)])\s+"#) {
                skip = (m[0] as NSString).length
            }
            let bodyLen = (body as NSString).length

            var inline = ""
            var allMono = paraRange.length > 0
            a.enumerateAttributes(in: NSRange(location: paraRange.location + skip, length: max(0, bodyLen - skip)), options: []) { attrs, r, _ in
                var run = text.substring(with: r)
                let font = attrs[.font] as? NSFont
                let traits = font?.fontDescriptor.symbolicTraits ?? []
                let name = font?.fontName.lowercased() ?? ""
                let mono = traits.contains(.monoSpace) || name.contains("menlo") || name.contains("courier") || name.contains("mono")
                if !mono { allMono = false }
                if !run.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty {
                    let lead = String(run.prefix(while: { $0 == " " || $0 == "\t" }))
                    let trailLen = run.reversed().prefix(while: { $0 == " " || $0 == "\t" }).count
                    let core = String(run.dropFirst(lead.count).dropLast(trailLen))
                    let trail = String(run.suffix(trailLen))
                    var c = core
                    if mono && headerLevel == 0 { c = "`" + c + "`" }
                    if traits.contains(.bold) && headerLevel == 0 { c = "**" + c + "**" }
                    if traits.contains(.italic) { c = "_" + c + "_" }
                    if let link = attrs[.link] {
                        let url = (link as? URL)?.absoluteString ?? (link as? String) ?? ""
                        c = "[\(c)](\(url))"
                    }
                    run = lead + c + trail
                }
                inline += run
            }
            inline = inline.replacing(#"\s+"#, with: " ").trimmed

            if !lists.isEmpty {
                flushCode()
                let depth = lists.count
                let fmt = lists.last!.markerFormat.rawValue
                let ordered = fmt.contains("decimal") || fmt.contains("lower") || fmt.contains("upper")
                if depth > olCounters.count { olCounters.append(0) }
                while depth < olCounters.count { olCounters.removeLast() }
                olCounters[depth - 1] += 1
                let marker = ordered ? "\(olCounters[depth - 1]). " : "- "
                if !prevWasList, let last = out.last, !last.isEmpty { out.append("") }
                if !inline.isEmpty { out.append(String(repeating: "  ", count: depth - 1) + marker + inline) }
                prevWasList = true
                continue
            }
            olCounters.removeAll()
            if prevWasList { out.append(""); prevWasList = false }

            if headerLevel > 0 {
                flushCode()
                if let last = out.last, !last.isEmpty { out.append("") }
                out.append(String(repeating: "#", count: headerLevel) + " " + inline)
                out.append("")
            } else if allMono && !inline.isEmpty && paraRange.length > 1 {
                codeBlock.append(body.replacingOccurrences(of: "`", with: ""))
            } else {
                flushCode()
                if inline.isEmpty {
                    if let last = out.last, !last.isEmpty { out.append("") }
                } else {
                    out.append(inline)
                    out.append("")
                }
            }
        }
        flushCode()
        var s = out.joined(separator: "\n")
        s = s.replacing(#"\n{3,}"#, with: "\n\n")
        return s.trimmingCharacters(in: .whitespacesAndNewlines) + "\n"
    }
}
