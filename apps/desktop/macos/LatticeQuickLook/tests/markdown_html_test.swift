import Foundation

/// Fixture runner for `MarkdownHTML` (no XCTest / Xcode project).
@main
enum MarkdownHTMLTests {
    static func main() {
        var failed = 0

        func check(_ name: String, _ condition: () -> Bool) {
            if condition() {
                print("ok  \(name)")
            } else {
                print("FAIL \(name)")
                failed += 1
            }
        }

        let sample = """
        # Hello

        A paragraph with `code`.

        - one
        - two
        """
        let html = MarkdownHTML.fragment(from: sample)

        check("heading") {
            html.contains("<h1>") && html.contains("Hello") && html.contains("</h1>")
        }
        check("paragraph") {
            html.contains("<p>") && html.contains("A paragraph with") && html.contains("</p>")
        }
        check("list") {
            html.contains("<ul>") && html.contains("<li>") && html.contains("one") && html.contains("two")
                && html.contains("</ul>")
        }
        check("inline-code") {
            html.contains("<code>code</code>")
        }

        let escaped = MarkdownHTML.fragment(from: "<script>alert(1)</script>")
        check("escapes-raw-html") {
            escaped.contains("&lt;script&gt;") && !escaped.contains("<script>")
        }

        let emphasis = MarkdownHTML.fragment(from: "Say **bold** and *italic*.")
        check("emphasis") {
            emphasis.contains("<strong>bold</strong>") && emphasis.contains("<em>italic</em>")
        }

        let link = MarkdownHTML.fragment(from: "[docs](https://example.com)")
        check("link") {
            link.contains("<a href=\"https://example.com\">docs</a>")
        }

        let jsImg = MarkdownHTML.fragment(from: "![x](javascript:alert(1))")
        check("blocks-javascript-image") {
            !jsImg.lowercased().contains("javascript:")
        }

        let localImg = MarkdownHTML.fragment(from: "![diagram](./assets/plot.png)")
        check("local-image") {
            localImg.contains("<img src=\"./assets/plot.png\"") && localImg.contains("alt=\"diagram\"")
        }

        let titled = MarkdownHTML.document(from: "Hi", catalogTitle: "Catalog <Title>")
        check("catalog-title-escaped") {
            titled.contains("<header class=\"catalog-title\">Catalog &lt;Title&gt;</header>")
        }

        if failed > 0 {
            print("\(failed) failed")
            exit(1)
        }
        print("all tests passed")
    }
}
