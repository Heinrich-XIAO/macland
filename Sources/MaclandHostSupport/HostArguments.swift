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
        var autoExitAfterChild = false

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
            case "--auto-exit-after-child":
                autoExitAfterChild = true
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
            autoExitAfterChild: autoExitAfterChild
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
