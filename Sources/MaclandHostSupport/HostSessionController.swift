import AppKit
import Foundation
import MetalKit
import CoreGraphics

@MainActor
public final class HostSessionController: NSObject, NSApplicationDelegate {
    private let configuration: HostLaunchConfiguration
    private var window: NSWindow?
    private var compositorProcess: Process?
    private var managedRuntimeDirectory: URL?
    private var statusLabel: NSTextField?

    public init(configuration: HostLaunchConfiguration) {
        self.configuration = configuration
    }

    public func applicationDidFinishLaunching(_ notification: Notification) {
        writeStatus("host_booted")
        let screenFrame = NSScreen.main?.visibleFrame ?? NSRect(x: 0, y: 0, width: 1440, height: 900)
        let frame = configuration.mode == .fullscreen
            ? (NSScreen.main?.frame ?? screenFrame)
            : centeredDebugFrame(in: screenFrame)
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

        let contentView = NSView(frame: frame)
        contentView.wantsLayer = true

        let view = MTKView(frame: frame)
        view.translatesAutoresizingMaskIntoConstraints = false
        view.clearColor = MTLClearColor(red: 0.04, green: 0.05, blue: 0.08, alpha: 1.0)
        view.preferredFramesPerSecond = 60
        contentView.addSubview(view)

        let overlay = makeOverlayView()
        contentView.addSubview(overlay)

        NSLayoutConstraint.activate([
            view.leadingAnchor.constraint(equalTo: contentView.leadingAnchor),
            view.trailingAnchor.constraint(equalTo: contentView.trailingAnchor),
            view.topAnchor.constraint(equalTo: contentView.topAnchor),
            view.bottomAnchor.constraint(equalTo: contentView.bottomAnchor),
            overlay.leadingAnchor.constraint(equalTo: contentView.leadingAnchor, constant: 28),
            overlay.topAnchor.constraint(equalTo: contentView.topAnchor, constant: 28),
            overlay.widthAnchor.constraint(lessThanOrEqualToConstant: 520),
        ])

        window.contentView = contentView

        self.window = window
        window.makeKeyAndOrderFront(nil)
        NSApp.activate(ignoringOtherApps: true)
        applyPresentationMode()
        launchCompositorIfNeeded()
        scheduleImageCaptureIfNeeded()

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
            updateStatusLabel("Host ready")
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
            let status = "Compositor exited with status \(process.terminationStatus)"
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
            Task { @MainActor in
                self.updateStatusLabel(status)
            }
            if autoExitAfterChild {
                Task { @MainActor in
                    NSApp.terminate(nil)
                }
            }
        }

        do {
            updateStatusLabel("Launching compositor…")
            try process.run()
            compositorProcess = process
            updateStatusLabel("Compositor running")
            writeStatus("child_started")
        } catch {
            updateStatusLabel("Launch failed: \(error.localizedDescription)")
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

        for _ in 0..<16 {
            let suffix = String(format: "%08x", UInt32.random(in: UInt32.min...UInt32.max))
            let runtimeDirectory = URL(fileURLWithPath: "/tmp", isDirectory: true)
                .appendingPathComponent("ml\(suffix)", isDirectory: true)
            do {
                try FileManager.default.createDirectory(
                    at: runtimeDirectory,
                    withIntermediateDirectories: false
                )
                try FileManager.default.setAttributes(
                    [.posixPermissions: 0o700],
                    ofItemAtPath: runtimeDirectory.path
                )
                managedRuntimeDirectory = runtimeDirectory
                return runtimeDirectory
            } catch {
                continue
            }
        }

        writeStatus("runtime_dir_failed:could_not_allocate_short_path")
        return nil
    }

    private func centeredDebugFrame(in screenFrame: NSRect) -> NSRect {
        let width = min(max(screenFrame.width * 0.72, 960), 1280)
        let height = min(max(screenFrame.height * 0.72, 640), 820)
        let originX = screenFrame.origin.x + (screenFrame.width - width) / 2
        let originY = screenFrame.origin.y + (screenFrame.height - height) / 2
        return NSRect(x: originX, y: originY, width: width, height: height)
    }

    private func makeOverlayView() -> NSView {
        let card = NSVisualEffectView()
        card.translatesAutoresizingMaskIntoConstraints = false
        card.material = .hudWindow
        card.blendingMode = .withinWindow
        card.state = .active
        card.wantsLayer = true
        card.layer?.cornerRadius = 16

        let stack = NSStackView()
        stack.translatesAutoresizingMaskIntoConstraints = false
        stack.orientation = .vertical
        stack.alignment = .leading
        stack.spacing = 10

        let title = NSTextField(labelWithString: "macland")
        title.font = NSFont.systemFont(ofSize: 24, weight: .semibold)
        title.textColor = NSColor.white

        let mode = NSTextField(labelWithString: configuration.mode == .fullscreen ? "Fullscreen host session" : "Windowed debug session")
        mode.font = NSFont.systemFont(ofSize: 13, weight: .medium)
        mode.textColor = NSColor(calibratedWhite: 0.84, alpha: 1.0)

        let compositor = configuration.compositorExecutable ?? "No compositor executable"
        let command = ([URL(fileURLWithPath: compositor).lastPathComponent] + configuration.compositorArguments).joined(separator: " ")
        let commandLabel = NSTextField(wrappingLabelWithString: command)
        commandLabel.font = NSFont.monospacedSystemFont(ofSize: 12, weight: .regular)
        commandLabel.textColor = NSColor(calibratedWhite: 0.76, alpha: 1.0)
        commandLabel.maximumNumberOfLines = 3

        let hint = NSTextField(wrappingLabelWithString: "The host window is live. A black background currently means the compositor launched, but frame presentation into the Metal view is not implemented yet.")
        hint.font = NSFont.systemFont(ofSize: 12)
        hint.textColor = NSColor(calibratedWhite: 0.7, alpha: 1.0)
        hint.maximumNumberOfLines = 4

        let status = NSTextField(labelWithString: "Preparing host…")
        status.font = NSFont.systemFont(ofSize: 13, weight: .semibold)
        status.textColor = NSColor(calibratedRed: 0.56, green: 0.83, blue: 1.0, alpha: 1.0)
        self.statusLabel = status

        [title, mode, commandLabel, hint, status].forEach(stack.addArrangedSubview)
        card.addSubview(stack)
        NSLayoutConstraint.activate([
            stack.leadingAnchor.constraint(equalTo: card.leadingAnchor, constant: 18),
            stack.trailingAnchor.constraint(equalTo: card.trailingAnchor, constant: -18),
            stack.topAnchor.constraint(equalTo: card.topAnchor, constant: 18),
            stack.bottomAnchor.constraint(equalTo: card.bottomAnchor, constant: -18),
        ])
        return card
    }

    private func updateStatusLabel(_ text: String) {
        statusLabel?.stringValue = text
    }

    private func scheduleImageCaptureIfNeeded() {
        guard let captureImagePath = configuration.captureImagePath else {
            return
        }

        let delayMillis = max(configuration.captureDelayMillis ?? 1200, 0)
        DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(delayMillis)) { [weak self] in
            self?.captureWindowImage(to: captureImagePath)
        }
    }

    private func captureWindowImage(to path: String) {
        guard let window, let contentView = window.contentView else {
            updateStatusLabel("Image capture failed: missing window")
            return
        }

        let bounds = contentView.bounds
        guard let bitmap = contentView.bitmapImageRepForCachingDisplay(in: bounds) else {
            updateStatusLabel("Image capture failed: no bitmap")
            return
        }

        contentView.cacheDisplay(in: bounds, to: bitmap)
        guard let pngData = bitmap.representation(using: .png, properties: [:]) else {
            updateStatusLabel("Image capture failed: PNG encode")
            return
        }

        do {
            try pngData.write(to: URL(fileURLWithPath: path), options: .atomic)
            updateStatusLabel("Captured image to \(URL(fileURLWithPath: path).lastPathComponent)")
            if configuration.autoExitAfterCapture {
                if let compositorProcess, compositorProcess.isRunning {
                    compositorProcess.terminate()
                }
                NSApp.terminate(nil)
            }
        } catch {
            updateStatusLabel("Image capture failed: \(error.localizedDescription)")
        }
    }
}

private struct StatusEnvelope: Codable {
    let status: String
    let permissions: [String: String]
}
