import AppKit
import Foundation
import MetalKit

public final class HostSessionController: NSObject, NSApplicationDelegate {
    private let configuration: HostLaunchConfiguration
    private var window: NSWindow?

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

        if configuration.mode == .fullscreen {
            window.toggleFullScreen(nil)
        }
    }
}

