import Foundation

public enum HostArgumentParser {
    public static func parse(_ arguments: [String]) throws -> HostLaunchConfiguration {
        if let configPath = configPath(in: arguments) {
            let data = try Data(contentsOf: URL(fileURLWithPath: configPath))
            return try JSONDecoder().decode(HostLaunchConfiguration.self, from: data)
        }

        var mode: SessionMode = .fullscreen
        var executable: String?
        var executableArgs: [String] = []
        var env: [String: String] = [:]
        var workingDirectory: String?
        var statusFile: String?
        var previewLogFile: String?
        var autoExitAfterChild = false
        var captureImagePath: String?
        var captureDelayMillis: Int?
        var autoExitAfterCapture = false

        var iterator = arguments.dropFirst().makeIterator()
        while let arg = iterator.next() {
            switch arg {
            case "--fullscreen":
                mode = .fullscreen
            case "--windowed-debug":
                mode = .windowedDebug
            case "--compositor":
                executable = iterator.next()
            case "--arg":
                if let value = iterator.next() {
                    executableArgs.append(value)
                }
            case "--env":
                if let pair = iterator.next(), let index = pair.firstIndex(of: "=") {
                    let key = String(pair[..<index])
                    let value = String(pair[pair.index(after: index)...])
                    env[key] = value
                }
            case "--working-directory":
                workingDirectory = iterator.next()
            case "--status-file":
                statusFile = iterator.next()
            case "--preview-log-file":
                previewLogFile = iterator.next()
            case "--auto-exit-after-child":
                autoExitAfterChild = true
            case "--capture-image":
                captureImagePath = iterator.next()
            case "--capture-delay-ms":
                if let value = iterator.next(), let parsed = Int(value) {
                    captureDelayMillis = parsed
                }
            case "--auto-exit-after-capture":
                autoExitAfterCapture = true
            default:
                continue
            }
        }

        return HostLaunchConfiguration(
            mode: mode,
            compositorExecutable: executable,
            compositorArguments: executableArgs,
            environment: env,
            workingDirectory: workingDirectory,
            statusFile: statusFile,
            previewLogFile: previewLogFile,
            autoExitAfterChild: autoExitAfterChild,
            captureImagePath: captureImagePath,
            captureDelayMillis: captureDelayMillis,
            autoExitAfterCapture: autoExitAfterCapture
        )
    }

    private static func configPath(in arguments: [String]) -> String? {
        var iterator = arguments.dropFirst().makeIterator()
        while let arg = iterator.next() {
            if arg == "--config" {
                return iterator.next()
            }
        }
        return nil
    }
}
