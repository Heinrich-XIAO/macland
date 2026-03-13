import AppKit
import Foundation
import MetalKit

@MainActor
public final class HostSessionController: NSObject, NSApplicationDelegate {
    private let configuration: HostLaunchConfiguration
    private var window: NSWindow?
    private var compositorProcess: Process?
    private var managedRuntimeDirectory: URL?

    public init(configuration: HostLaunchConfiguration) {
        self.configuration = configuration
    }

    public func applicationDidFinishLaunching(_ notification: Notification) {
        let frame = NSScreen.main?.frame ?? NSRect(x: 0, y: 0, width: 1440, height: 900)
        let styleMask: NSWindow.StyleMask = configuration.mode == .fullscreen ? [.borderless] : [.titled, .closable, .resizable, .miniaturizable]
        let window = NSWindow(
            contentRect: frame,
            styleMask: styleMask,
            backing: .buffered,
            defer: false
        )
        window.title = "macland-host"
        window.isReleasedWhenClosed = false
        window.collectionBehavior = [.fullScreenPrimary, .fullScreenAllowsTiling]
        window.backgroundColor = NSColor.black

        let view = MTKView(frame: frame)
        view.clearColor = MTLClearColor(red: 0.04, green: 0.05, blue: 0.08, alpha: 1.0)
        view.preferredFramesPerSecond = 60
        window.contentView = view

        self.window = window
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        applyPresentationMode()
        launchCompositorIfNeeded()

        if configuration.mode == .fullscreen {
            window.toggleFullScreen(nil)
        }
    }

    public func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        true
    }

    public func applicationWillTerminate(_ notification: Notification) {
        compositorProcess?.terminate()
        if let managedRuntimeDirectory {
            try? FileManager.default.removeItem(at: managedRuntimeDirectory)
        }
    }

    private func applyPresentationMode() {
        guard configuration.mode == .fullscreen else {
            NSApp.presentationOptions = []
            return
        }

        NSApp.presentationOptions = [
            .hideDock,
            .hideMenuBar,
            .disableProcessSwitching,
            .disableForceQuit,
            .disableHideApplication,
        ]
    }

    private func launchCompositorIfNeeded() {
        guard let executable = configuration.compositorExecutable else {
            writeStatus("host_started")
            return
        }

        let process = Process()
        process.executableURL = URL(fileURLWithPath: executable)
        process.arguments = configuration.compositorArguments
        if let workingDirectory = configuration.workingDirectory {
            process.currentDirectoryURL = URL(fileURLWithPath: workingDirectory)
        }

        var environment = ProcessInfo.processInfo.environment
        for (key, value) in configuration.environment {
            environment[key] = value
        }
        if environment["XDG_RUNTIME_DIR"] == nil || environment["XDG_RUNTIME_DIR"]?.isEmpty == true {
            if let runtimeDirectory = prepareRuntimeDirectory() {
                environment["XDG_RUNTIME_DIR"] = runtimeDirectory.path
            }
        }
        process.environment = environment
        let statusFile = configuration.statusFile
        let autoExitAfterChild = configuration.autoExitAfterChild
        process.terminationHandler = { process in
            if let statusFile {
                let payload = StatusEnvelope(
                    status: "child_exit:\(process.terminationStatus)",
                    permissions: PermissionProbe.currentAudit().stringStates
                )
                let data = try? JSONEncoder().encode(payload)
                try? data?.write(
                    to: URL(fileURLWithPath: statusFile),
                    options: .atomic
                )
            }
            if autoExitAfterChild {
                Task { @MainActor in
                    NSApp.terminate(nil)
                }
            }
        }

        do {
            try process.run()
            compositorProcess = process
            writeStatus("child_started")
        } catch {
            writeStatus("child_failed:\(error.localizedDescription)")
            if configuration.autoExitAfterChild {
                Task { @MainActor in
                    NSApp.terminate(nil)
                }
            }
        }
    }

    private func writeStatus(_ status: String) {
        guard let statusFile = configuration.statusFile else {
            return
        }
        let payload = StatusEnvelope(
            status: status,
            permissions: PermissionProbe.currentAudit().stringStates
        )
        let data = try? JSONEncoder().encode(payload)
        try? data?.write(
            to: URL(fileURLWithPath: statusFile),
            options: .atomic
        )
    }

    private func prepareRuntimeDirectory() -> URL? {
        if let managedRuntimeDirectory {
            return managedRuntimeDirectory
        }

        let runtimeDirectory = URL(fileURLWithPath: NSTemporaryDirectory(), isDirectory: true)
            .appendingPathComponent("macland-runtime", isDirectory: true)
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        do {
            try FileManager.default.createDirectory(
                at: runtimeDirectory,
                withIntermediateDirectories: true
            )
            try FileManager.default.setAttributes(
                [.posixPermissions: 0o700],
                ofItemAtPath: runtimeDirectory.path
            )
            managedRuntimeDirectory = runtimeDirectory
            return runtimeDirectory
        } catch {
            writeStatus("runtime_dir_failed:\(error.localizedDescription)")
            return nil
        }
    }
}

private struct StatusEnvelope: Codable {
    let status: String
    let permissions: [String: String]
}
