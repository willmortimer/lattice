import Cocoa
import Quartz
import WebKit

/// Quick Look preview that renders Markdown / Lattice pages as HTML.
/// Bundle ID: `dev.lattice.desktop.quicklook` (App Group: `group.dev.lattice.shared`).
@objc(PreviewViewController)
final class PreviewViewController: NSViewController, QLPreviewingController, WKNavigationDelegate {
    private var webView: WKWebView!
    private var loadContinuation: CheckedContinuation<Void, Error>?

    override func loadView() {
        let config = WKWebViewConfiguration()
        config.defaultWebpagePreferences.allowsContentJavaScript = false
        config.preferences.javaScriptCanOpenWindowsAutomatically = false
        config.websiteDataStore = .nonPersistent()
        let wv = WKWebView(frame: .zero, configuration: config)
        wv.navigationDelegate = self
        wv.setValue(false, forKey: "drawsBackground")
        wv.underPageBackgroundColor = .windowBackgroundColor
        webView = wv
        view = wv
    }

    func preparePreviewOfFile(at url: URL) async throws {
        let data = try Data(contentsOf: url)
        let markdown = String(data: data, encoding: .utf8)
            ?? String(data: data, encoding: .isoLatin1)
            ?? ""
        let title = CatalogStore.title(forFileURL: url)
        let html = MarkdownHTML.document(from: markdown, catalogTitle: title)
        let baseURL = url.deletingLastPathComponent()
        try await loadHTML(html, baseURL: baseURL)
    }

    private func loadHTML(_ html: String, baseURL: URL) async throws {
        try await withCheckedThrowingContinuation { (cont: CheckedContinuation<Void, Error>) in
            DispatchQueue.main.async {
                self.loadViewIfNeeded()
                self.loadContinuation = cont
                self.webView.loadHTMLString(html, baseURL: baseURL)
                DispatchQueue.main.asyncAfter(deadline: .now() + 3) { [weak self] in
                    self?.finishLoad()
                }
            }
        }
    }

    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        finishLoad()
    }

    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError _: Error) {
        finishLoad()
    }

    func webView(
        _ webView: WKWebView,
        didFailProvisionalNavigation navigation: WKNavigation!,
        withError _: Error
    ) {
        finishLoad()
    }

    func webView(
        _ webView: WKWebView,
        decidePolicyFor navigationAction: WKNavigationAction,
        decisionHandler: @escaping (WKNavigationActionPolicy) -> Void
    ) {
        if navigationAction.navigationType == .other {
            let scheme = navigationAction.request.url?.scheme?.lowercased()
            if scheme == nil || scheme == "about" || scheme == "file" || scheme == "data" {
                decisionHandler(.allow)
                return
            }
        }
        decisionHandler(.cancel)
    }

    private func finishLoad() {
        guard let cont = loadContinuation else { return }
        loadContinuation = nil
        cont.resume()
    }
}

enum CatalogStore {
    static func title(forFileURL url: URL) -> String? {
        let catalogURL = FileManager.default
            .homeDirectoryForCurrentUser
            .appendingPathComponent(
                "Library/Group Containers/group.dev.lattice.shared/Library/Application Support/Lattice/spotlight-catalog.json"
            )
        guard
            let data = try? Data(contentsOf: catalogURL),
            let json = try? JSONSerialization.jsonObject(with: data) as? [String: Any],
            let root = json["workspaceRoot"] as? String,
            let resources = json["resources"] as? [[String: Any]]
        else {
            return nil
        }
        let rootURL = URL(fileURLWithPath: root)
        guard url.path.hasPrefix(rootURL.path) else { return nil }
        var relative = String(url.path.dropFirst(rootURL.path.count))
        if relative.hasPrefix("/") { relative.removeFirst() }
        return resources.first(where: { ($0["path"] as? String) == relative })?["title"] as? String
    }
}
