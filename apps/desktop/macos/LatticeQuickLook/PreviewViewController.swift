import Cocoa
import Quartz

/// Minimal Quick Look preview for Markdown / text Lattice documents.
/// Bundle ID: `dev.lattice.desktop.quicklook` (App Group: `group.dev.lattice.shared`).
@objc(PreviewViewController)
final class PreviewViewController: NSViewController, QLPreviewingController {
    private let textView = NSTextView()
    private let scrollView = NSScrollView()

    override func loadView() {
        scrollView.hasVerticalScroller = true
        scrollView.documentView = textView
        textView.isEditable = false
        textView.font = .monospacedSystemFont(ofSize: 12, weight: .regular)
        textView.textContainerInset = NSSize(width: 16, height: 16)
        view = scrollView
    }

    func preparePreviewOfFile(at url: URL) async throws {
        let data = try Data(contentsOf: url)
        let text = String(data: data, encoding: .utf8)
            ?? String(data: data, encoding: .isoLatin1)
            ?? "<binary preview unavailable>"
        await MainActor.run {
            if let title = CatalogStore.title(forFileURL: url) {
                self.textView.string = "# \(title)\n\n" + text
            } else {
                self.textView.string = text
            }
        }
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
