# ClipTo

`ct` converts what is on the clipboard between plain text, Markdown and rich text.
Copy Markdown, run `ct rich`, paste into Mail, Slack or Outlook with headings, bold and lists intact.
Copy a formatted web page or document, run `ct md`, paste Markdown into your editor.

One Rust binary for macOS, Windows and Linux. Works from the command line everywhere, and as a
resident tray / menu bar tool with a global hotkey on macOS and Windows.

## Install

Prebuilt binaries for macOS (Apple Silicon and Intel), Windows and Linux are on the
[Releases page](https://github.com/jonx/clip-to/releases): unpack and put `ct` on your PATH.
On macOS, a downloaded binary is quarantined; run `xattr -d com.apple.quarantine ct` once.

Or build from source:

```sh
git clone https://github.com/jonx/clip-to.git
cd clip-to
make install            # cargo build --release, then copies target/release/ct to /opt/homebrew/bin
```

Use `make install PREFIX=/usr/local` for another location. On Windows, `cargo build --release`
and put `target\release\ct.exe` somewhere on your PATH. On Linux, `make install PREFIX=~/.local`.
Needs a Rust toolchain (rustup.rs); no other dependency.

## Command line

```
ct                 help plus what is on the clipboard right now (previews cut at 600 characters)

ct rich    (ct r)  Markdown -> rich text (HTML + RTF + plain). Pasting keeps formatting
ct md      (ct m)  rich text (HTML/RTF) -> Markdown source, in the plain-text flavor
ct plain   (ct p)  Markdown or rich text -> plain text, syntax stripped, bullets as "•"
ct html    (ct h)  Markdown or rich text -> clean HTML source, in the plain-text flavor
ct text    (ct t)  keep only the plain-text flavor (drop HTML/RTF)

-i FILE            read FILE ('-' = stdin) instead of the clipboard
-o                 print the result instead of writing the clipboard
-p                 print the result after writing the clipboard
-x                 exclusive: drop the other flavors
-f                 force: convert even when the clipboard already holds the requested format
--rtf              rich: write RTF without HTML, for Apple Notes, TextEdit, Pages
```

Apple Notes, TextEdit, Pages and Stickies pick HTML when it is present and flatten it to their own
styles, while they render RTF faithfully (real heading sizes, bold inside headings). The hotkey
popup detects those apps and writes RTF only for them; on the command line use `ct rich --rtf`.

If the clipboard already holds what you ask for, nothing is converted and the clipboard is left
untouched: `rich` when HTML is present and the text is not Markdown, `md` when there is no rich
flavor or the text already looks like Markdown, `plain` when the text has no Markdown syntax, `text` when only plain text
is there. `rich` also stores the Markdown source in a private flavor (`me.jkn.clipto.markdown` on macOS,
`ClipTo Markdown` on Windows) that no other app sees, so `ct md` afterwards restores it exactly.
This never degrades content copied from a rich app. `-f` forces the conversion. From the hotkey popup, hold ⌥ (Alt on Windows)
while choosing.

`md`, `plain` and `html` are non-destructive: they only replace the plain-text flavor and keep
HTML and RTF, so Mail or Slack still paste the original formatting while a terminal or editor gets
the converted text. Add `-x` to drop the other flavors. `rich` rebuilds all flavors from Markdown,
and `text` keeps plain text only.

`daemon`, `install` and `uninstall` shorten to `d`, `i`, `u` as well.
Output is colored when writing to a terminal. Set `NO_COLOR` to disable.

## Resident mode and hotkey

```
ct daemon                            tray icon + global hotkey, stays in the foreground
ct daemon --hotkey ctrl+shift+f9     pick another combination
ct daemon --no-paste                 only convert the clipboard, do not paste the result
ct install [--hotkey ...] [--no-paste]   start the daemon now and at every login
ct uninstall                         stop it and remove it from login
```

The default hotkey is Ctrl+Alt+Cmd+V on macOS (⌃⌥⌘V) and Ctrl+Alt+Win+V on Windows.

### macOS: the chooser panel

Pressing the hotkey opens a floating panel next to the pointer. The app you are in keeps the
focus. Left: the targets (Rich text, Markdown, Plain text, HTML source, Text only) and a
"Force conversion" checkbox. Right: a full, scrollable preview of what the chosen target will
put on the clipboard, rendered by the system text engine (the one TextEdit and Notes use) for
rich text, as source for Markdown and HTML, as text otherwise. The preview never touches the
clipboard: only choosing does.

- Hover or ↑↓ to select, click, ⏎ or 1 to 5 to choose, esc to close.
- Hold ⌥ (or press space) to force a conversion the skip rule would leave alone.
- Right-click hides a choice (kept in `~/.config/clipto/hidden`), ⌘R restores all.
- Choosing converts the clipboard and pastes into the app you came from (⌘V is simulated,
  which needs the Accessibility permission; the menu bar icon shows the status and opens the
  right Settings pane). `--no-paste` disables the paste.
- Rich text for Apple Notes, TextEdit, Pages and Stickies is written as RTF without HTML,
  because those apps pick HTML when present and flatten it to their own styles. Other apps get
  HTML plus RTF. On the command line, `ct rich --rtf` does the same.

Background colours are stripped in the preview only, so dark-theme HTML copied from editors
stays readable; the clipboard keeps them and the destination app decides.

### Windows: the popup menu

The hotkey pops up a context menu at the pointer listing the flavors present, a preview line,
and the targets. Hold Alt while choosing to force a conversion. The result is pasted with a
simulated Ctrl+V, no permission needed. The daemon detaches from its console when it starts.

Hotkey names: `ctrl`, `alt`, `shift`, `super` (Cmd on macOS, Win on Windows), letters, digits,
`f1`..`f12`, `space`, `enter`, `tab`, `escape`, joined with `+`.

Login start uses a LaunchAgent (`~/Library/LaunchAgents/me.jkn.clipto.plist`) on macOS and the
`HKCU\Software\Microsoft\Windows\CurrentVersion\Run` key on Windows. The hotkey itself needs no
Accessibility permission on macOS (Carbon hotkey API); only the paste does.

## How clipboard flavors work

The clipboard holds several representations of the same content at once. The copying app decides
which ones to provide; the pasting app picks the richest one it understands. Terminals and code
editors always take plain text. Mail, Notes, Slack, Outlook, Word, Pages and browsers take HTML
first, then RTF, then plain text. `ct` on its own tells you which flavors are present, so you
know what a paste will do. `ct rich` writes all of them from Markdown; `ct text` keeps only plain
text so every app pastes it unchanged.

On macOS the flavors are `public.utf8-plain-text`, `public.html` and `public.rtf` on NSPasteboard.
On Windows they are `CF_UNICODETEXT`, the registered `HTML Format` (CF_HTML, with its offset
header) and `Rich Text Format`.

## How it is built

- `markdown.rs`: Markdown -> HTML with pulldown-cmark, HTML -> Markdown with htmd, Markdown ->
  plain text by walking the parser events. Pure Rust, shared by every platform.
- `convert.rs`: flavors, targets, the skip rule, and conversion tests between every pair of
  formats (Markdown, HTML, RTF on macOS, plain text), run with `cargo test`.
- `clipboard/macos.rs`: NSPasteboard through objc2-app-kit, plus NSAttributedString for
  RTF <-> HTML, so `rich` also writes RTF and RTF-only sources can be read.
- `clipboard/windows.rs`: Win32 clipboard through clipboard-win. RTF is listed and preserved but
  not converted (Windows has no system converter); HTML covers Word, Outlook and browsers.
- `clipboard/linux.rs`: X11 and Wayland through arboard, text and HTML. Because a Linux clipboard
  is served by the copying process, `ct` leaves a small background copy of itself (`ct __serve`)
  offering the content until another app takes the clipboard over. Command line only: no tray,
  no hotkey, no auto-paste, no RTF, no private Markdown flavor (the Markdown heuristic applies).
- `daemon.rs`: tao event loop, tray-icon, muda menus and global-hotkey. These crates call the
  native APIs: NSStatusItem, NSMenu and Carbon hotkeys on macOS; Shell_NotifyIcon, TrackPopupMenu
  and RegisterHotKey on Windows.
- `macos_panel.rs`: the chooser panel, written directly against AppKit through objc2: a
  non-activating NSPanel, a custom NSView for the list, an NSButton checkbox and an NSTextView
  preview. `config.rs` stores the hidden choices.
- `swift/`: the original macOS-only Swift implementation, kept as the behaviour reference.

Limitations: RTF carries no heading level, and reading an RTF-only clipboard flattens nested
lists and code blocks. HTML sources keep everything.

## License

MIT. John Knipper, <code@jkn.me>, http://jkn.me
