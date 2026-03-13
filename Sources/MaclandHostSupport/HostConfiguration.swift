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
    public var workingDirectory: String?
    public var statusFile: String?
    public var autoExitAfterChild: Bool
    public var captureImagePath: String?
    public var captureDelayMillis: Int?
    public var autoExitAfterCapture: Bool

    public init(
        mode: SessionMode,
        compositorExecutable: String? = nil,
        compositorArguments: [String] = [],
        environment: [String: String] = [:],
        permissionHints: [PermissionKind] = PermissionKind.requiredDefaults,
        workingDirectory: String? = nil,
        statusFile: String? = nil,
        autoExitAfterChild: Bool = false,
        captureImagePath: String? = nil,
        captureDelayMillis: Int? = nil,
        autoExitAfterCapture: Bool = false
    ) {
        self.mode = mode
        self.compositorExecutable = compositorExecutable
        self.compositorArguments = compositorArguments
        self.environment = environment
        self.permissionHints = permissionHints
        self.workingDirectory = workingDirectory
        self.statusFile = statusFile
        self.autoExitAfterChild = autoExitAfterChild
        self.captureImagePath = captureImagePath
        self.captureDelayMillis = captureDelayMillis
        self.autoExitAfterCapture = autoExitAfterCapture
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

    public var stringStates: [String: String] {
        Dictionary(uniqueKeysWithValues: states.map { key, value in
            (key.rawValue, value.rawValue)
        })
    }
}
