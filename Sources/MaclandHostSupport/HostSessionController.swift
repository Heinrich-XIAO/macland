import AppKit
import Foundation
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

        let view = makeSceneView(frame: frame)
        view.translatesAutoresizingMaskIntoConstraints = false
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

        let hint = NSTextField(wrappingLabelWithString: "The host window is live. This scene is a host-rendered preview while compositor frame presentation into the window is still being wired up.")
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

    private func makeSceneView(frame: NSRect) -> NSView {
        let scene = NSView(frame: frame)
        scene.wantsLayer = true
        scene.layer = CAGradientLayer()

        if let gradient = scene.layer as? CAGradientLayer {
            gradient.colors = [
                NSColor(calibratedRed: 0.06, green: 0.09, blue: 0.16, alpha: 1.0).cgColor,
                NSColor(calibratedRed: 0.03, green: 0.04, blue: 0.07, alpha: 1.0).cgColor,
            ]
            gradient.startPoint = CGPoint(x: 0.1, y: 1.0)
            gradient.endPoint = CGPoint(x: 0.9, y: 0.0)
        }

        let wallpaperGlow = NSView()
        wallpaperGlow.translatesAutoresizingMaskIntoConstraints = false
        wallpaperGlow.wantsLayer = true
        wallpaperGlow.layer?.backgroundColor = NSColor(calibratedRed: 0.16, green: 0.42, blue: 0.72, alpha: 0.22).cgColor
        wallpaperGlow.layer?.cornerRadius = 220
        scene.addSubview(wallpaperGlow)

        let panel = NSVisualEffectView()
        panel.translatesAutoresizingMaskIntoConstraints = false
        panel.material = .sidebar
        panel.blendingMode = .withinWindow
        panel.state = .active
        panel.wantsLayer = true
        panel.layer?.cornerRadius = 24
        panel.layer?.borderWidth = 1
        panel.layer?.borderColor = NSColor(calibratedWhite: 1.0, alpha: 0.08).cgColor
        scene.addSubview(panel)

        let chrome = NSView()
        chrome.translatesAutoresizingMaskIntoConstraints = false
        chrome.wantsLayer = true
        chrome.layer?.backgroundColor = NSColor(calibratedWhite: 0.08, alpha: 0.92).cgColor
        panel.addSubview(chrome)

        let title = NSTextField(labelWithString: previewTitle())
        title.translatesAutoresizingMaskIntoConstraints = false
        title.font = NSFont.systemFont(ofSize: 18, weight: .semibold)
        title.textColor = NSColor.white
        panel.addSubview(title)

        let subtitle = NSTextField(labelWithString: "Nested compositor preview")
        subtitle.translatesAutoresizingMaskIntoConstraints = false
        subtitle.font = NSFont.systemFont(ofSize: 12, weight: .medium)
        subtitle.textColor = NSColor(calibratedWhite: 0.78, alpha: 1.0)
        panel.addSubview(subtitle)

        let preview = NSView()
        preview.translatesAutoresizingMaskIntoConstraints = false
        preview.wantsLayer = true
        preview.layer?.backgroundColor = NSColor(calibratedRed: 0.94, green: 0.94, blue: 0.96, alpha: 1.0).cgColor
        preview.layer?.cornerRadius = 18
        panel.addSubview(preview)

        let accent = NSView()
        accent.translatesAutoresizingMaskIntoConstraints = false
        accent.wantsLayer = true
        accent.layer?.backgroundColor = accentColor().cgColor
        accent.layer?.cornerRadius = 10
        preview.addSubview(accent)

        let mockWindow = NSView()
        mockWindow.translatesAutoresizingMaskIntoConstraints = false
        mockWindow.wantsLayer = true
        mockWindow.layer?.backgroundColor = NSColor.white.cgColor
        mockWindow.layer?.cornerRadius = 14
        mockWindow.layer?.shadowColor = NSColor.black.cgColor
        mockWindow.layer?.shadowOpacity = 0.18
        mockWindow.layer?.shadowRadius = 20
        mockWindow.layer?.shadowOffset = CGSize(width: 0, height: -2)
        preview.addSubview(mockWindow)

        let mockHeader = NSView()
        mockHeader.translatesAutoresizingMaskIntoConstraints = false
        mockHeader.wantsLayer = true
        mockHeader.layer?.backgroundColor = NSColor(calibratedWhite: 0.96, alpha: 1.0).cgColor
        mockHeader.layer?.cornerRadius = 14
        mockWindow.addSubview(mockHeader)

        let mockBody = NSView()
        mockBody.translatesAutoresizingMaskIntoConstraints = false
        mockBody.wantsLayer = true
        mockBody.layer?.backgroundColor = NSColor(calibratedWhite: 0.985, alpha: 1.0).cgColor
        mockWindow.addSubview(mockBody)

        let bodyTitle = NSTextField(labelWithString: previewBodyTitle())
        bodyTitle.translatesAutoresizingMaskIntoConstraints = false
        bodyTitle.font = NSFont.systemFont(ofSize: 16, weight: .semibold)
        bodyTitle.textColor = NSColor(calibratedWhite: 0.15, alpha: 1.0)
        mockBody.addSubview(bodyTitle)

        let bodyText = NSTextField(wrappingLabelWithString: "Launch path, windowing, and image capture are active. The next remaining step is compositor frame transport into the host view.")
        bodyText.translatesAutoresizingMaskIntoConstraints = false
        bodyText.font = NSFont.systemFont(ofSize: 12)
        bodyText.textColor = NSColor(calibratedWhite: 0.35, alpha: 1.0)
        bodyText.maximumNumberOfLines = 3
        mockBody.addSubview(bodyText)

        let strip = NSView()
        strip.translatesAutoresizingMaskIntoConstraints = false
        strip.wantsLayer = true
        strip.layer?.backgroundColor = NSColor(calibratedWhite: 1.0, alpha: 0.72).cgColor
        strip.layer?.cornerRadius = 12
        preview.addSubview(strip)

        let strip2 = NSView()
        strip2.translatesAutoresizingMaskIntoConstraints = false
        strip2.wantsLayer = true
        strip2.layer?.backgroundColor = NSColor(calibratedWhite: 1.0, alpha: 0.45).cgColor
        strip2.layer?.cornerRadius = 12
        preview.addSubview(strip2)

        NSLayoutConstraint.activate([
            wallpaperGlow.trailingAnchor.constraint(equalTo: scene.trailingAnchor, constant: -72),
            wallpaperGlow.topAnchor.constraint(equalTo: scene.topAnchor, constant: 58),
            wallpaperGlow.widthAnchor.constraint(equalToConstant: 440),
            wallpaperGlow.heightAnchor.constraint(equalToConstant: 440),

            panel.centerXAnchor.constraint(equalTo: scene.centerXAnchor, constant: 118),
            panel.centerYAnchor.constraint(equalTo: scene.centerYAnchor, constant: 72),
            panel.widthAnchor.constraint(equalTo: scene.widthAnchor, multiplier: 0.58),
            panel.heightAnchor.constraint(equalTo: scene.heightAnchor, multiplier: 0.66),

            chrome.leadingAnchor.constraint(equalTo: panel.leadingAnchor),
            chrome.trailingAnchor.constraint(equalTo: panel.trailingAnchor),
            chrome.topAnchor.constraint(equalTo: panel.topAnchor),
            chrome.heightAnchor.constraint(equalToConstant: 38),

            title.leadingAnchor.constraint(equalTo: panel.leadingAnchor, constant: 24),
            title.topAnchor.constraint(equalTo: panel.topAnchor, constant: 18),
            subtitle.leadingAnchor.constraint(equalTo: panel.leadingAnchor, constant: 24),
            subtitle.topAnchor.constraint(equalTo: title.bottomAnchor, constant: 4),

            preview.leadingAnchor.constraint(equalTo: panel.leadingAnchor, constant: 24),
            preview.trailingAnchor.constraint(equalTo: panel.trailingAnchor, constant: -24),
            preview.topAnchor.constraint(equalTo: subtitle.bottomAnchor, constant: 20),
            preview.bottomAnchor.constraint(equalTo: panel.bottomAnchor, constant: -24),

            accent.leadingAnchor.constraint(equalTo: preview.leadingAnchor, constant: 22),
            accent.topAnchor.constraint(equalTo: preview.topAnchor, constant: 22),
            accent.widthAnchor.constraint(equalToConstant: 160),
            accent.heightAnchor.constraint(equalToConstant: 20),

            mockWindow.leadingAnchor.constraint(equalTo: preview.leadingAnchor, constant: 36),
            mockWindow.trailingAnchor.constraint(equalTo: preview.trailingAnchor, constant: -44),
            mockWindow.topAnchor.constraint(equalTo: accent.bottomAnchor, constant: 22),
            mockWindow.bottomAnchor.constraint(equalTo: preview.bottomAnchor, constant: -82),

            mockHeader.leadingAnchor.constraint(equalTo: mockWindow.leadingAnchor),
            mockHeader.trailingAnchor.constraint(equalTo: mockWindow.trailingAnchor),
            mockHeader.topAnchor.constraint(equalTo: mockWindow.topAnchor),
            mockHeader.heightAnchor.constraint(equalToConstant: 42),

            mockBody.leadingAnchor.constraint(equalTo: mockWindow.leadingAnchor),
            mockBody.trailingAnchor.constraint(equalTo: mockWindow.trailingAnchor),
            mockBody.topAnchor.constraint(equalTo: mockHeader.bottomAnchor),
            mockBody.bottomAnchor.constraint(equalTo: mockWindow.bottomAnchor),

            bodyTitle.leadingAnchor.constraint(equalTo: mockBody.leadingAnchor, constant: 20),
            bodyTitle.topAnchor.constraint(equalTo: mockBody.topAnchor, constant: 18),
            bodyText.leadingAnchor.constraint(equalTo: mockBody.leadingAnchor, constant: 20),
            bodyText.trailingAnchor.constraint(equalTo: mockBody.trailingAnchor, constant: -20),
            bodyText.topAnchor.constraint(equalTo: bodyTitle.bottomAnchor, constant: 10),

            strip.leadingAnchor.constraint(equalTo: preview.leadingAnchor, constant: 34),
            strip.bottomAnchor.constraint(equalTo: preview.bottomAnchor, constant: -28),
            strip.widthAnchor.constraint(equalToConstant: 220),
            strip.heightAnchor.constraint(equalToConstant: 26),

            strip2.leadingAnchor.constraint(equalTo: strip.trailingAnchor, constant: 14),
            strip2.bottomAnchor.constraint(equalTo: preview.bottomAnchor, constant: -28),
            strip2.widthAnchor.constraint(equalToConstant: 96),
            strip2.heightAnchor.constraint(equalToConstant: 26),
        ])

        return scene
    }

    private func previewTitle() -> String {
        let executable = configuration.compositorExecutable.map { URL(fileURLWithPath: $0).lastPathComponent } ?? "Session"
        return "\(executable) Preview"
    }

    private func previewBodyTitle() -> String {
        let executable = configuration.compositorExecutable.map { URL(fileURLWithPath: $0).lastPathComponent.lowercased() } ?? ""
        if executable.contains("niri") {
            return "Niri output target"
        }
        if executable.contains("sway") {
            return "Sway output target"
        }
        return "Compositor output target"
    }

    private func accentColor() -> NSColor {
        let executable = configuration.compositorExecutable.map { URL(fileURLWithPath: $0).lastPathComponent.lowercased() } ?? ""
        if executable.contains("niri") {
            return NSColor(calibratedRed: 0.26, green: 0.76, blue: 0.56, alpha: 1.0)
        }
        if executable.contains("sway") {
            return NSColor(calibratedRed: 0.32, green: 0.57, blue: 0.95, alpha: 1.0)
        }
        return NSColor(calibratedRed: 0.83, green: 0.53, blue: 0.29, alpha: 1.0)
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
