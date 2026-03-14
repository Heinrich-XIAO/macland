import AppKit
import Foundation
import MaclandHostSupport

@main
struct MaclandHostMain {
    @MainActor
    static func main() throws {
        let configuration = try HostArgumentParser.parse(CommandLine.arguments)
        let application = NSApplication.shared
        let delegate = HostSessionController(configuration: configuration)
        application.setActivationPolicy(.regular)
        application.delegate = delegate
        withExtendedLifetime(delegate) {
            application.run()
        }
    }
}
