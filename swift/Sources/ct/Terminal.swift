import Foundation

/// ANSI colours, enabled only when writing to a terminal and NO_COLOR is unset.
enum Term {
    static let color = isatty(1) != 0 && ProcessInfo.processInfo.environment["NO_COLOR"] == nil
    static let errColor = isatty(2) != 0 && ProcessInfo.processInfo.environment["NO_COLOR"] == nil

    static func c(_ code: String, _ s: String, on: Bool = color) -> String {
        on ? "\u{1B}[\(code)m\(s)\u{1B}[0m" : s
    }
    static func bold(_ s: String) -> String { c("1", s) }
    static func dim(_ s: String) -> String { c("2", s) }
    static func cyan(_ s: String) -> String { c("36", s) }
    static func green(_ s: String) -> String { c("32", s) }
    static func yellow(_ s: String) -> String { c("33", s) }
    static func magenta(_ s: String) -> String { c("35", s) }
    static func red(_ s: String) -> String { c("31", s) }

    static func stderr(_ s: String) {
        FileHandle.standardError.write((s + "\n").data(using: .utf8)!)
    }
    static func stdout(_ s: String) {
        FileHandle.standardOutput.write(s.data(using: .utf8)!)
    }
}
