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

    let audit = PermissionAudit(states: [
        .accessibility: .granted,
        .inputMonitoring: .denied,
        .screenRecording: .unknown,
    ])
    try assert(audit.missingRequiredPermissions == [.inputMonitoring], "expected missing input monitoring")
}

do {
    try runSelfTests()
    print("macland-host-selftest: ok")
} catch {
    fputs("macland-host-selftest: \(error)\n", stderr)
    exit(1)
}
