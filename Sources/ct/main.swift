import AppKit

let version = "1.0.0"

func printHelp() {
    let T = Term.self
    print(T.bold(T.cyan("ct")) + " — ClipTo \(version): convert the macOS clipboard between plain text, Markdown and rich text")
    print(T.dim("John Knipper · http://jkn.me"))
    print()
    print(T.bold("Commands"))
    for t in Target.allCases { print("  " + T.green(t.rawValue.padding(toLength: 8, withPad: " ", startingAt: 0)) + "  " + t.help) }
    print("  " + T.green("daemon".padding(toLength: 8, withPad: " ", startingAt: 0)) + "  stay resident: menu bar icon + global hotkey (default \(Hotkey.default.description)) that pops up a format chooser")
    print("  " + T.green("install".padding(toLength: 8, withPad: " ", startingAt: 0)) + "  start the daemon now and at every login (LaunchAgent); " + T.green("uninstall") + " removes it")
    print()
    print(T.bold("Options"))
    let opts: [(String, String)] = [
        ("-i FILE", "read FILE ('-' = stdin) instead of the clipboard"),
        ("-o", "print the result instead of writing the clipboard"),
        ("-p", "print the result after writing the clipboard"),
        ("-x", "exclusive: drop the other flavors (md, plain, html keep HTML/RTF by default)"),
        ("--hotkey K", "daemon/install: key combo, e.g. ctrl+alt+cmd+v, cmd+shift+f9"),
    ]
    for (k, v) in opts { print("  " + T.yellow(k.padding(toLength: 10, withPad: " ", startingAt: 0)) + "  " + v) }
    print()
    print(T.dim("md, plain and html only update the plain-text flavor and keep HTML/RTF, so rich apps still paste the original formatting."))
    print(T.dim("Commands can be abbreviated to their first letter: ct r, ct m, ct p, ct h, ct t, ct d, ct i, ct u."))
    print(T.dim("Without a command, ct prints this help and what is on the clipboard (previews cut at 600 characters)."))
    print(T.dim("Example: copy some Markdown, run `ct rich`, paste into Mail or Slack."))
}

func printShow() {
    let T = Term.self
    let types = Clipboard.types
    print(T.bold("Clipboard") + " — " + (types.isEmpty ? T.dim("(empty)") : types.joined(separator: T.dim(", "))))
    for f in Flavor.allCases {
        if let s = Clipboard.pb.string(forType: f.type) {
            print()
            print(T.magenta("--- \(f.type.rawValue)") + T.dim(" (\(s.count) chars) ") + T.magenta("---") + "  " + T.dim("\(f.label): pasted by \(f.pastedBy)"))
            print(s.count > 600 ? String(s.prefix(600)) + T.dim("…") : s)
        }
    }
}

// MARK: Argument parsing

var infile: String? = nil
var toStdout = false
var alsoPrint = false
var exclusive = false
var hotkeySpec: String? = nil
var positional: [String] = []
let args = Array(CommandLine.arguments.dropFirst())
var idx = 0
while idx < args.count {
    let a = args[idx]
    switch a {
    case "-i": idx += 1; infile = idx < args.count ? args[idx] : "-"
    case "-o": toStdout = true
    case "-p": alsoPrint = true
    case "-x": exclusive = true
    case "--hotkey": idx += 1; hotkeySpec = idx < args.count ? args[idx] : nil
    case "-h", "--help", "help": printHelp(); exit(0)
    case "-v", "--version": print("ct \(version)"); exit(0)
    default: positional.append(a)
    }
    idx += 1
}
let aliases = ["r": "rich", "m": "md", "p": "plain", "h": "html", "t": "text", "d": "daemon", "i": "install", "u": "uninstall"]
let cmd = aliases[positional.first ?? ""] ?? positional.first ?? ""

func input(prefer: [Flavor]) -> Source {
    if let f = infile {
        let data = f == "-" ? FileHandle.standardInput.readDataToEndOfFile() : (FileManager.default.contents(atPath: f) ?? Data())
        return .markdown(String(decoding: data, as: UTF8.self))
    }
    guard let s = Clipboard.read(prefer: prefer) else {
        Term.stderr(Term.c("31", "ct: nothing usable on the clipboard", on: Term.errColor) + " (types: \(Clipboard.types.joined(separator: ", ")))")
        exit(1)
    }
    return s
}

func hotkeyOrExit() -> Hotkey {
    guard let spec = hotkeySpec else { return .default }
    guard let h = Hotkey(spec: spec) else { Term.stderr("ct: invalid hotkey '\(spec)'. Use e.g. ctrl+alt+cmd+v"); exit(2) }
    return h
}

// MARK: Dispatch

switch cmd {
case "":
    printHelp(); print(); printShow(); exit(0)
case "daemon":
    Daemon(hotkey: hotkeyOrExit()).run()
case "install":
    _ = hotkeyOrExit()
    do {
        try LaunchAgent.install(hotkey: hotkeySpec)
        Term.stderr("ct: daemon installed and started (\(LaunchAgent.plistPath)). It will start at login.")
    } catch { Term.stderr("ct: install failed: \(error.localizedDescription)"); exit(1) }
    exit(0)
case "uninstall":
    do { try LaunchAgent.uninstall(); Term.stderr("ct: daemon stopped and removed.") }
    catch { Term.stderr("ct: uninstall failed: \(error.localizedDescription)"); exit(1) }
    exit(0)
default:
    guard let target = Target(rawValue: cmd) else {
        Term.stderr("ct: unknown command '\(cmd)'\n"); printHelp(); exit(2)
    }
    let (result, items) = Clipboard.convert(input(prefer: target.prefer), to: target)
    if toStdout {
        Term.stdout(result)
    } else {
        let keep = !exclusive && !target.replacesAll
        Clipboard.write(items, keepOthers: keep)
        let names = Clipboard.present.map { $0.label }.joined(separator: ", ")
        let note = keep && Clipboard.present.count > 1 ? Term.dim("  (other flavors kept, -x to drop them)") : ""
        Term.stderr(Term.c("32", "✓", on: Term.errColor) + " clipboard: " + Term.c("36", names, on: Term.errColor) + note)
        if alsoPrint { Term.stdout(result) }
    }
}
