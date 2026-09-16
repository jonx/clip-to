import AppKit
import Carbon

/// Resident mode: a menu bar item plus a global hotkey. Pressing the hotkey pops up a
/// menu at the mouse pointer that lists the flavors currently on the clipboard and offers
/// the target formats. Uses Carbon's RegisterEventHotKey, which needs no Accessibility permission.
final class Daemon: NSObject, NSApplicationDelegate, NSMenuDelegate {
    static var shared: Daemon!
    let hotkey: Hotkey
    var statusItem: NSStatusItem!
    var hotKeyRef: EventHotKeyRef?

    init(hotkey: Hotkey) { self.hotkey = hotkey }

    func run() -> Never {
        Daemon.shared = self
        let app = NSApplication.shared
        app.setActivationPolicy(.accessory)
        app.delegate = self
        app.run()
        exit(0)
    }

    func applicationDidFinishLaunching(_ notification: Notification) {
        statusItem = NSStatusBar.system.statusItem(withLength: NSStatusItem.variableLength)
        if let b = statusItem.button {
            b.image = NSImage(systemSymbolName: "doc.on.clipboard", accessibilityDescription: "ClipTo")
            b.image?.isTemplate = true
        }
        let menu = NSMenu()
        menu.delegate = self
        statusItem.menu = menu
        registerHotkey()
        Term.stderr("ct: resident, press \(hotkey.description) to convert the clipboard. Ctrl-C or Quit in the menu to stop.")
    }

    // MARK: Menu

    func menuNeedsUpdate(_ menu: NSMenu) { fill(menu, resident: true) }

    func fill(_ menu: NSMenu, resident: Bool) {
        menu.removeAllItems()
        let present = Clipboard.present
        let header = NSMenuItem(title: present.isEmpty ? "Clipboard: empty" : "Clipboard: " + present.map { $0.label }.joined(separator: ", "), action: nil, keyEquivalent: "")
        header.isEnabled = false
        menu.addItem(header)
        if let s = Clipboard.pb.string(forType: .string) {
            let preview = s.trimmingCharacters(in: .whitespacesAndNewlines).replacingOccurrences(of: "\n", with: " ⏎ ")
            let p = NSMenuItem(title: "   " + (preview.count > 60 ? String(preview.prefix(60)) + "…" : preview), action: nil, keyEquivalent: "")
            p.isEnabled = false
            menu.addItem(p)
        }
        menu.addItem(.separator())
        let convertTo = NSMenuItem(title: "Convert to", action: nil, keyEquivalent: "")
        convertTo.isEnabled = false
        menu.addItem(convertTo)
        for (i, t) in Target.allCases.enumerated() {
            let item = NSMenuItem(title: t.title, action: #selector(convert(_:)), keyEquivalent: "\(i + 1)")
            item.keyEquivalentModifierMask = []
            item.target = self
            item.representedObject = t.rawValue
            item.toolTip = t.help
            item.isEnabled = !present.isEmpty
            menu.addItem(item)
        }
        if resident {
            menu.addItem(.separator())
            let hk = NSMenuItem(title: "Hotkey: \(hotkey.description)", action: nil, keyEquivalent: "")
            hk.isEnabled = false
            menu.addItem(hk)
            let about = NSMenuItem(title: "ClipTo — John Knipper, jkn.me", action: #selector(openSite), keyEquivalent: "")
            about.target = self
            menu.addItem(about)
            let quit = NSMenuItem(title: "Quit ct", action: #selector(quit), keyEquivalent: "q")
            quit.target = self
            menu.addItem(quit)
        }
    }

    @objc func convert(_ sender: NSMenuItem) {
        guard let raw = sender.representedObject as? String, let target = Target(rawValue: raw) else { return }
        guard let src = Clipboard.read(prefer: target.prefer) else { flash("✗ empty"); return }
        let (_, items) = Clipboard.convert(src, to: target)
        Clipboard.write(items, keepOthers: !target.replacesAll)
        flash("✓ " + target.title)
    }

    /// Show a short confirmation next to the status icon.
    func flash(_ text: String) {
        statusItem.button?.title = " " + text
        DispatchQueue.main.asyncAfter(deadline: .now() + 1.5) { [weak self] in self?.statusItem.button?.title = "" }
    }

    @objc func openSite() { NSWorkspace.shared.open(URL(string: "http://jkn.me")!) }
    @objc func quit() { NSApp.terminate(nil) }

    // MARK: Hotkey

    func registerHotkey() {
        var eventType = EventTypeSpec(eventClass: OSType(kEventClassKeyboard), eventKind: UInt32(kEventHotKeyPressed))
        InstallEventHandler(GetApplicationEventTarget(), { _, _, _ -> OSStatus in
            DispatchQueue.main.async { Daemon.shared.popup() }
            return noErr
        }, 1, &eventType, nil, nil)
        let id = EventHotKeyID(signature: OSType(0x4354_4F21), id: 1) // "CTO!"
        let status = RegisterEventHotKey(hotkey.keyCode, hotkey.carbonModifiers, id, GetApplicationEventTarget(), 0, &hotKeyRef)
        if status != noErr {
            Term.stderr("ct: could not register hotkey \(hotkey.description) (error \(status)); it may be taken by another app.")
        }
    }

    func popup() {
        let menu = NSMenu()
        fill(menu, resident: false)
        NSApp.activate(ignoringOtherApps: true)
        menu.popUp(positioning: nil, at: NSEvent.mouseLocation, in: nil)
    }
}

/// A key combination such as "ctrl+alt+cmd+v".
struct Hotkey: CustomStringConvertible {
    var keyCode: UInt32
    var carbonModifiers: UInt32
    var description: String

    static let `default` = Hotkey(spec: "ctrl+alt+cmd+v")!

    static let keyCodes: [String: UInt32] = [
        "a": 0, "s": 1, "d": 2, "f": 3, "h": 4, "g": 5, "z": 6, "x": 7, "c": 8, "v": 9, "b": 11, "q": 12, "w": 13,
        "e": 14, "r": 15, "y": 16, "t": 17, "1": 18, "2": 19, "3": 20, "4": 21, "6": 22, "5": 23, "=": 24, "9": 25,
        "7": 26, "-": 27, "8": 28, "0": 29, "]": 30, "o": 31, "u": 32, "[": 33, "i": 34, "p": 35, "l": 37, "j": 38,
        "'": 39, "k": 40, ";": 41, "\\": 42, ",": 43, "/": 44, "n": 45, "m": 46, ".": 47, "`": 50, "space": 49,
        "return": 36, "tab": 48, "escape": 53, "f1": 122, "f2": 120, "f3": 99, "f4": 118, "f5": 96, "f6": 97,
        "f7": 98, "f8": 100, "f9": 101, "f10": 109, "f11": 103, "f12": 111,
    ]

    init?(spec: String) {
        var mods: UInt32 = 0
        var names: [String] = []
        var key: UInt32?
        for part in spec.lowercased().split(separator: "+").map(String.init) {
            switch part {
            case "cmd", "command", "⌘": mods |= UInt32(cmdKey); names.append("⌘")
            case "ctrl", "control", "⌃": mods |= UInt32(controlKey); names.append("⌃")
            case "alt", "option", "opt", "⌥": mods |= UInt32(optionKey); names.append("⌥")
            case "shift", "⇧": mods |= UInt32(shiftKey); names.append("⇧")
            default:
                guard let k = Hotkey.keyCodes[part] else { return nil }
                key = k; names.append(part.uppercased())
            }
        }
        guard let k = key, mods != 0 else { return nil }
        keyCode = k; carbonModifiers = mods; description = names.joined()
    }
}

// MARK: - LaunchAgent (start at login)

enum LaunchAgent {
    static let label = "me.jkn.clipto"
    static var plistPath: String {
        NSHomeDirectory() + "/Library/LaunchAgents/\(label).plist"
    }

    static func install(hotkey: String?) throws {
        let exe = URL(fileURLWithPath: CommandLine.arguments[0]).resolvingSymlinksInPath().path
        var args = [exe, "daemon"]
        if let h = hotkey { args += ["--hotkey", h] }
        let plist: [String: Any] = [
            "Label": label,
            "ProgramArguments": args,
            "RunAtLoad": true,
            "KeepAlive": true,
            "ProcessType": "Interactive",
        ]
        let data = try PropertyListSerialization.data(fromPropertyList: plist, format: .xml, options: 0)
        try FileManager.default.createDirectory(atPath: (plistPath as NSString).deletingLastPathComponent, withIntermediateDirectories: true)
        try data.write(to: URL(fileURLWithPath: plistPath))
        _ = shell("launchctl", "bootout", "gui/\(getuid())/\(label)")
        let r = shell("launchctl", "bootstrap", "gui/\(getuid())", plistPath)
        if r != 0 { throw NSError(domain: "ct", code: Int(r), userInfo: [NSLocalizedDescriptionKey: "launchctl bootstrap failed (\(r))"]) }
    }

    static func uninstall() throws {
        _ = shell("launchctl", "bootout", "gui/\(getuid())/\(label)")
        if FileManager.default.fileExists(atPath: plistPath) { try FileManager.default.removeItem(atPath: plistPath) }
    }

    @discardableResult
    static func shell(_ args: String...) -> Int32 {
        let p = Process()
        p.executableURL = URL(fileURLWithPath: "/bin/launchctl")
        p.arguments = Array(args.dropFirst())
        p.standardOutput = FileHandle.nullDevice; p.standardError = FileHandle.nullDevice
        try? p.run(); p.waitUntilExit()
        return p.terminationStatus
    }
}
