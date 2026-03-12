import Foundation

public enum HostArgumentParser {
    public static func parse(_ arguments: [String]) throws -> HostLaunchConfiguration {
        var mode: SessionMode = .fullscreen
        var executable: String?
        var executableArgs: [String] = []
        var env: [String: String] = [:]

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
            default:
                continue
            }
        }

        return HostLaunchConfiguration(
            mode: mode,
            compositorExecutable: executable,
            compositorArguments: executableArgs,
            environment: env
        )
    }
}

