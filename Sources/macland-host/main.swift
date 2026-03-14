import AppKit
import Foundation
import MaclandHostSupport

@main
struct MaclandHostMain {
    @MainActor
    static func main() throws {
        let configuration = try HostArgumentParser.parse(CommandLine.arguments)
        appendPreviewBootstrapLog(to: configuration.previewLogFile, event: "main_entry")
        let application = NSApplication.shared
        let delegate = HostSessionController(configuration: configuration)
        application.setActivationPolicy(.regular)
        application.delegate = delegate
        withExtendedLifetime(delegate) {
            application.run()
        }
    }

    private static func appendPreviewBootstrapLog(to path: String?, event: String) {
        guard let path else {
            return
        }
        let url = URL(fileURLWithPath: path)
        try? FileManager.default.createDirectory(
            at: url.deletingLastPathComponent(),
            withIntermediateDirectories: true
        )
        let payload = [
            "timestamp": ISO8601DateFormatter().string(from: Date()),
            "event": event,
        ]
        guard let data = try? JSONSerialization.data(withJSONObject: payload, options: []),
              let line = String(data: data, encoding: .utf8) else {
            return
        }
        if !FileManager.default.fileExists(atPath: url.path) {
            try? (line + "\n").write(to: url, atomically: true, encoding: .utf8)
        }
    }
}
