import Foundation

/// Small sanitizing Markdown → HTML converter for Quick Look.
/// Escapes raw HTML; does not execute scripts; remote images are omitted.
enum MarkdownHTML {
    /// HTML fragment (no document wrapper) for tests and embedding.
    static func fragment(from markdown: String) -> String {
        renderBlocks(markdown)
    }

    /// Full document for `WKWebView.loadHTMLString`.
    static func document(from markdown: String, catalogTitle: String? = nil) -> String {
        var body = ""
        if let title = catalogTitle?.trimmingCharacters(in: .whitespacesAndNewlines), !title.isEmpty {
            body += "<header class=\"catalog-title\">\(escapeText(title))</header>\n"
        }
        body += renderBlocks(markdown)
        return """
        <!DOCTYPE html>
        <html lang="en">
        <head>
        <meta charset="utf-8">
        <meta name="viewport" content="width=device-width, initial-scale=1">
        <meta http-equiv="Content-Security-Policy" content="default-src 'none'; img-src file: data:; style-src 'unsafe-inline'; script-src 'none'; connect-src 'none'; frame-src 'none'; object-src 'none'; base-uri 'none';">
        <style>\(stylesheet)</style>
        </head>
        <body>
        \(body)
        </body>
        </html>
        """
    }

    private static let stylesheet = """
    :root { color-scheme: light dark; }
    html, body {
      margin: 0;
      padding: 0;
      background: transparent;
      color: CanvasText;
      font: 15px/1.5 -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
    }
    body { padding: 20px 24px 32px; max-width: 52rem; }
    header.catalog-title {
      font-size: 13px;
      font-weight: 600;
      letter-spacing: 0.02em;
      text-transform: uppercase;
      opacity: 0.65;
      margin: 0 0 1.25rem;
    }
    h1, h2, h3, h4, h5, h6 { line-height: 1.25; margin: 1.4em 0 0.5em; }
    h1 { font-size: 1.85em; margin-top: 0; }
    h2 { font-size: 1.45em; }
    h3 { font-size: 1.2em; }
    p, ul, ol, pre, blockquote { margin: 0 0 1em; }
    ul, ol { padding-left: 1.5em; }
    li { margin: 0.2em 0; }
    a { color: LinkText; }
    code {
      font-family: ui-monospace, SFMono-Regular, Menlo, monospace;
      font-size: 0.88em;
      background: color-mix(in srgb, CanvasText 8%, transparent);
      padding: 0.1em 0.35em;
      border-radius: 4px;
    }
    pre {
      background: color-mix(in srgb, CanvasText 8%, transparent);
      padding: 0.85em 1em;
      border-radius: 8px;
      overflow: auto;
    }
    pre code { background: none; padding: 0; font-size: 0.85em; }
    blockquote {
      border-left: 3px solid color-mix(in srgb, CanvasText 25%, transparent);
      padding: 0 0 0 0.9em;
      color: color-mix(in srgb, CanvasText 80%, transparent);
    }
    img { max-width: 100%; height: auto; }
    hr { border: none; border-top: 1px solid color-mix(in srgb, CanvasText 18%, transparent); margin: 1.5em 0; }
    """
}

// MARK: - Blocks

private func renderBlocks(_ markdown: String) -> String {
    let normalized = markdown.replacingOccurrences(of: "\r\n", with: "\n").replacingOccurrences(of: "\r", with: "\n")
    let lines = normalized.split(separator: "\n", omittingEmptySubsequences: false).map(String.init)
    var html = ""
    var i = 0
    while i < lines.count {
        let line = lines[i]
        let trimmed = line.trimmingCharacters(in: .whitespaces)

        if trimmed.hasPrefix("```") {
            var body: [String] = []
            i += 1
            while i < lines.count, !lines[i].trimmingCharacters(in: .whitespaces).hasPrefix("```") {
                body.append(lines[i])
                i += 1
            }
            if i < lines.count { i += 1 }
            html += "<pre><code>\(escapeText(body.joined(separator: "\n")))</code></pre>\n"
            continue
        }

        if trimmed.isEmpty {
            i += 1
            continue
        }

        if let heading = headingMatch(trimmed) {
            html += "<h\(heading.level)>\(renderInline(heading.text))</h\(heading.level)>\n"
            i += 1
            continue
        }

        if isHorizontalRule(trimmed) {
            html += "<hr>\n"
            i += 1
            continue
        }

        if trimmed.hasPrefix(">") {
            var quoted: [String] = []
            while i < lines.count {
                let t = lines[i].trimmingCharacters(in: .whitespaces)
                if t.hasPrefix(">") {
                    var rest = String(t.dropFirst())
                    if rest.hasPrefix(" ") { rest.removeFirst() }
                    quoted.append(rest)
                    i += 1
                } else {
                    break
                }
            }
            html += "<blockquote>\(renderBlocks(quoted.joined(separator: "\n")))</blockquote>\n"
            continue
        }

        if unorderedMarker(trimmed) != nil {
            var items: [String] = []
            while i < lines.count, let item = unorderedMarker(lines[i].trimmingCharacters(in: .whitespaces)) {
                items.append(item)
                i += 1
            }
            html += "<ul>"
            for item in items {
                html += "<li>\(renderInline(item))</li>"
            }
            html += "</ul>\n"
            continue
        }

        if orderedMarker(trimmed) != nil {
            var items: [String] = []
            while i < lines.count, let item = orderedMarker(lines[i].trimmingCharacters(in: .whitespaces)) {
                items.append(item)
                i += 1
            }
            html += "<ol>"
            for item in items {
                html += "<li>\(renderInline(item))</li>"
            }
            html += "</ol>\n"
            continue
        }

        var para: [String] = [trimmed]
        i += 1
        while i < lines.count {
            let next = lines[i].trimmingCharacters(in: .whitespaces)
            if next.isEmpty
                || next.hasPrefix("```")
                || headingMatch(next) != nil
                || isHorizontalRule(next)
                || next.hasPrefix(">")
                || unorderedMarker(next) != nil
                || orderedMarker(next) != nil
            {
                break
            }
            para.append(next)
            i += 1
        }
        html += "<p>\(renderInline(para.joined(separator: " ")))</p>\n"
    }
    return html
}

private func headingMatch(_ trimmed: String) -> (level: Int, text: String)? {
    guard trimmed.hasPrefix("#") else { return nil }
    var level = 0
    for ch in trimmed {
        if ch == "#" { level += 1 } else { break }
    }
    guard (1 ... 6).contains(level) else { return nil }
    let rest = trimmed.dropFirst(level)
    guard rest.isEmpty || rest.first == " " else { return nil }
    let text = rest.trimmingCharacters(in: .whitespaces)
    guard !text.isEmpty else { return nil }
    return (level, text)
}

private func isHorizontalRule(_ trimmed: String) -> Bool {
    let compact = trimmed.filter { !$0.isWhitespace }
    guard compact.count >= 3 else { return false }
    return compact.allSatisfy { $0 == "-" } || compact.allSatisfy { $0 == "*" } || compact.allSatisfy { $0 == "_" }
}

private func unorderedMarker(_ trimmed: String) -> String? {
    for prefix in ["- ", "* ", "+ "] where trimmed.hasPrefix(prefix) {
        return String(trimmed.dropFirst(2))
    }
    return nil
}

private func orderedMarker(_ trimmed: String) -> String? {
    guard let dot = trimmed.firstIndex(of: ".") else { return nil }
    let num = trimmed[..<dot]
    guard !num.isEmpty, num.allSatisfy(\.isNumber) else { return nil }
    let rest = trimmed[trimmed.index(after: dot)...]
    guard rest.first == " " else { return nil }
    return String(rest.dropFirst())
}

// MARK: - Inline

private func renderInline(_ text: String) -> String {
    var slots: [String] = []
    func stash(_ html: String) -> String {
        let token = "\u{E000}\(slots.count)\u{E001}"
        slots.append(html)
        return token
    }

    var s = text
    s = replace(pattern: "`([^`\\n]+)`", in: s) { match, src in
        stash("<code>\(escapeText(group(match, 1, src)))</code>")
    }
    s = replace(pattern: "!\\[([^\\]]*)\\]\\(([^)]+)\\)", in: s) { match, src in
        let alt = escapeText(group(match, 1, src))
        guard let url = sanitizedURL(group(match, 2, src), as: .image) else {
            return stash(alt.isEmpty ? "" : alt)
        }
        return stash("<img src=\"\(escapeAttr(url))\" alt=\"\(alt)\">")
    }
    s = replace(pattern: "\\[([^\\]]+)\\]\\(([^)]+)\\)", in: s) { match, src in
        let label = group(match, 1, src)
        guard let url = sanitizedURL(group(match, 2, src), as: .link) else {
            return stash(escapeText(label))
        }
        return stash("<a href=\"\(escapeAttr(url))\">\(escapeText(label))</a>")
    }

    s = escapeText(s)
    s = replace(pattern: "\\*\\*([^*\\n]+)\\*\\*", in: s) { match, src in
        "<strong>\(group(match, 1, src))</strong>"
    }
    s = replace(pattern: "__([^_\\n]+)__", in: s) { match, src in
        "<strong>\(group(match, 1, src))</strong>"
    }
    s = replace(pattern: "(?<!\\*)\\*([^*\\n]+)\\*(?!\\*)", in: s) { match, src in
        "<em>\(group(match, 1, src))</em>"
    }
    s = replace(pattern: "(?<!_)_([^_\\n]+)_(?!_)", in: s) { match, src in
        "<em>\(group(match, 1, src))</em>"
    }

    for (idx, html) in slots.enumerated() {
        s = s.replacingOccurrences(of: "\u{E000}\(idx)\u{E001}", with: html)
    }
    return s
}

private enum URLRole {
    case image
    case link
}

private func sanitizedURL(_ raw: String, as role: URLRole) -> String? {
    let trimmed = raw.trimmingCharacters(in: .whitespacesAndNewlines)
    if trimmed.isEmpty { return nil }
    let lower = trimmed.lowercased()
    if lower.hasPrefix("javascript:") || lower.hasPrefix("vbscript:") {
        return nil
    }
    switch role {
    case .image:
        if lower.hasPrefix("data:image/") { return trimmed }
        if lower.hasPrefix("http://") || lower.hasPrefix("https://") { return nil }
        if lower.hasPrefix("file:") || trimmed.contains("://") { return nil }
        return trimmed
    case .link:
        if lower.hasPrefix("http://") || lower.hasPrefix("https://") || lower.hasPrefix("mailto:") {
            return trimmed
        }
        if trimmed.contains("://") { return nil }
        return trimmed
    }
}

private func escapeText(_ text: String) -> String {
    text
        .replacingOccurrences(of: "&", with: "&amp;")
        .replacingOccurrences(of: "<", with: "&lt;")
        .replacingOccurrences(of: ">", with: "&gt;")
}

private func escapeAttr(_ text: String) -> String {
    escapeText(text).replacingOccurrences(of: "\"", with: "&quot;")
}

private func replace(
    pattern: String,
    in string: String,
    options: NSRegularExpression.Options = [],
    _ replacer: (NSTextCheckingResult, String) -> String
) -> String {
    guard let regex = try? NSRegularExpression(pattern: pattern, options: options) else {
        return string
    }
    let ns = string as NSString
    let matches = regex.matches(in: string, range: NSRange(location: 0, length: ns.length))
    guard !matches.isEmpty else { return string }
    var out = ""
    var cursor = 0
    for match in matches {
        let range = match.range
        if range.location > cursor {
            out += ns.substring(with: NSRange(location: cursor, length: range.location - cursor))
        }
        out += replacer(match, string)
        cursor = range.location + range.length
    }
    if cursor < ns.length {
        out += ns.substring(from: cursor)
    }
    return out
}

private func group(_ match: NSTextCheckingResult, _ index: Int, _ string: String) -> String {
    let range = match.range(at: index)
    guard range.location != NSNotFound else { return "" }
    return (string as NSString).substring(with: range)
}
