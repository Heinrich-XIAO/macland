import Foundation
import MaclandHostSupport

do {
    let data = try PermissionProbe.encodeCurrentAudit()
    FileHandle.standardOutput.write(data)
    FileHandle.standardOutput.write(Data([0x0a]))
} catch {
    fputs("macland-permissions: \(error)\n", stderr)
    exit(1)
}
