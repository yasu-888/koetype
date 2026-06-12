import Cocoa

@main
struct OverlayMain {
    static func main() {
        let app = NSApplication.shared
        let delegate = OverlayAppDelegate()
        app.delegate = delegate
        app.setActivationPolicy(.accessory)
        app.run()
    }
}
