import ApplicationServices
import CoreGraphics
import Foundation

public enum PermissionProbe {
    public static func currentAudit() -> PermissionAudit {
        PermissionAudit(states: [
            .accessibility: AXIsProcessTrusted() ? .granted : .denied,
            .inputMonitoring: .unknown,
            .screenRecording: CGPreflightScreenCaptureAccess() ? .granted : .denied,
        ])
    }

    public static func encodeCurrentAudit() throws -> Data {
        try JSONEncoder().encode(PermissionAuditEnvelope(states: currentAudit().stringStates))
    }
}

private struct PermissionAuditEnvelope: Codable {
    let states: [String: String]
}
