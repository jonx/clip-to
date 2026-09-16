# ClipTo

`ct` converts what is on the clipboard between plain text, Markdown and rich text.
Copy Markdown, run `ct rich`, paste into Mail, Slack or Outlook with headings, bold and lists intact.
Copy a formatted web page or document, run `ct md`, paste Markdown into your editor.

One Rust binary for macOS, Windows and Linux. Works from the command line everywhere, and as a
resident tray / menu bar tool with a global hotkey on macOS and Windows.

## Install

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
ct html    (ct h)  Markdown -> HTML source, in the plain-text flavor
ct text    (ct t)  keep only the plain-text flavor (drop HTML/RTF)

-i FILE            read FILE ('-' = stdin) instead of the clipboard
-o                 print the result instead of writing the clipboard
-p                 print the result after writing the clipboard
-x                 exclusive: drop the other flavors
-f                 force: convert even when the clipboard already holds the requested format
```

If the clipboard already holds what you ask for, nothing is converted and the clipboard is left
untouched: `rich` when HTML is present, `md` when there is no rich flavor or the text already
looks like Markdown, `plain` when the text has no Markdown syntax, `text` when only plain text
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

The default hotkey is Ctrl+Alt+Cmd+V on macOS (⌃⌥⌘V) and Ctrl+Alt+Win+V on Windows. Pressing it
pops up a menu at the mouse pointer that shows which flavors are on the clipboard (plain text,
HTML, RTF), a preview, and the target formats: Rich text, Markdown, Plain text, HTML source,
Text only. The same menu is available from the clipboard icon in the menu bar / tray, with Quit.
A short "✓ Rich text" confirmation appears next to the icon after each conversion.

Picking a format from the hotkey popup converts the clipboard, gives focus back to the app you
were in, and pastes the result there (⌘V on macOS, Ctrl+V on Windows). On macOS this needs the
Accessibility permission for `ct`; the system asks the first time. Use `--no-paste` to only
convert and paste yourself.

Hotkey names: `ctrl`, `alt`, `shift`, `super` (Cmd on macOS, Win on Windows), letters, digits,
`f1`..`f12`, `space`, `enter`, `tab`, `escape`, joined with `+`.

Login start uses a LaunchAgent (`~/Library/LaunchAgents/me.jkn.clipto.plist`) on macOS and the
`HKCU\Software\Microsoft\Windows\CurrentVersion\Run` key on Windows. No Accessibility permission
is needed on macOS: the hotkey goes through the Carbon hotkey API. On Windows the daemon detaches
from its console window when it starts.

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
- `swift/`: the original macOS-only Swift implementation, kept as the behaviour reference.

Limitations: RTF carries no heading level, and reading an RTF-only clipboard flattens nested
lists and code blocks. HTML sources keep everything.

## License

MIT. John Knipper, <code@jkn.me>, http://jkn.me
