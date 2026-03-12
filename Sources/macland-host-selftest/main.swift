import Foundation
import MaclandHostSupport

struct SelfTestFailure: Error, CustomStringConvertible {
    let description: String
}

func assert(_ condition: @autoclosure () -> Bool, _ message: String) throws {
    if !condition() {
        throw SelfTestFailure(description: message)
    }
}

func runSelfTests() throws {
    let config = try HostArgumentParser.parse([
        "macland-host",
        "--windowed-debug",
        "--compositor", "/tmp/labwc",
        "--arg", "--verbose",
        "--env", "MACLAND_MODE=1",
    ])

    try assert(config.mode == .windowedDebug, "expected debug mode")
    try assert(config.compositorExecutable == "/tmp/labwc", "expected compositor path")
    try assert(config.compositorArguments == ["--verbose"], "expected compositor arguments")
    try assert(config.environment["MACLAND_MODE"] == "1", "expected environment capture")
    try assert(config.autoExitAfterChild == false, "expected auto-exit default to be false")

    let audit = PermissionAudit(states: [
        .accessibility: .granted,
        .inputMonitoring: .denied,
        .screenRecording: .unknown,
    ])
    try assert(audit.missingRequiredPermissions == [.inputMonitoring], "expected missing input monitoring")

    let tempRoot = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: tempRoot, withIntermediateDirectories: true)
    let configPath = tempRoot.appendingPathComponent("launch.json")
    let encoded = try JSONEncoder().encode(
        HostLaunchConfiguration(
            mode: .fullscreen,
            compositorExecutable: "/bin/echo",
            compositorArguments: ["hello"],
            environment: ["MACLAND_MODE": "1"],
            workingDirectory: tempRoot.path,
            statusFile: tempRoot.appendingPathComponent("status.txt").path,
            autoExitAfterChild: true
        )
    )
    try encoded.write(to: configPath)

    let decoded = try HostArgumentParser.parse([
        "macland-host",
        "--config",
        configPath.path,
    ])
    try assert(decoded.compositorExecutable == "/bin/echo", "expected config-file executable")
    try assert(decoded.autoExitAfterChild, "expected config-file auto-exit")
}

do {
    try runSelfTests()
    print("macland-host-selftest: ok")
} catch {
    fputs("macland-host-selftest: \(error)\n", stderr)
    exit(1)
}
