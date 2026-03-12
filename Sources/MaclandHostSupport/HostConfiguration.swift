import Foundation

public enum SessionMode: String, Codable, Sendable {
    case fullscreen
    case windowedDebug
}

public struct HostLaunchConfiguration: Codable, Equatable, Sendable {
    public var mode: SessionMode
    public var compositorExecutable: String?
    public var compositorArguments: [String]
    public var environment: [String: String]
    public var permissionHints: [PermissionKind]

    public init(
        mode: SessionMode,
        compositorExecutable: String? = nil,
        compositorArguments: [String] = [],
        environment: [String: String] = [:],
        permissionHints: [PermissionKind] = PermissionKind.requiredDefaults
    ) {
        self.mode = mode
        self.compositorExecutable = compositorExecutable
        self.compositorArguments = compositorArguments
        self.environment = environment
        self.permissionHints = permissionHints
    }
}

public enum PermissionState: String, Codable, Sendable {
    case granted
    case denied
    case unknown
}

public enum PermissionKind: String, Codable, CaseIterable, Sendable {
    case accessibility
    case inputMonitoring
    case screenRecording

    public static let requiredDefaults: [PermissionKind] = [
        .accessibility,
        .inputMonitoring,
    ]
}

public struct PermissionAudit: Codable, Equatable, Sendable {
    public var states: [PermissionKind: PermissionState]

    public init(states: [PermissionKind: PermissionState]) {
        self.states = states
    }

    public var missingRequiredPermissions: [PermissionKind] {
        PermissionKind.requiredDefaults.filter { states[$0] != .granted }
    }

    public static let placeholder = PermissionAudit(
        states: [
            .accessibility: .unknown,
            .inputMonitoring: .unknown,
            .screenRecording: .unknown,
        ]
    )
}

