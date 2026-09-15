# ClipTo

`ct` converts what is on the macOS clipboard between plain text, Markdown and rich text.
Copy Markdown, run `ct rich`, paste into Mail or Slack with headings, bold and lists intact.
Copy a formatted web page or document, run `ct md`, paste Markdown into your editor.

Single Swift executable, AppKit only, no dependencies. Works from the command line or as a
resident menu bar tool with a global hotkey.

## Install

```sh
git clone https://github.com/jonx/ClipTo.git
cd ClipTo
make install            # builds with `swift build -c release`, installs to /opt/homebrew/bin/ct
```

Use `make install PREFIX=/usr/local` for another location. Requires macOS 13 or later and Xcode command line tools.

## Command line

```
ct                 help plus what is on the clipboard right now
ct show            list the flavors on the clipboard with a preview

ct rich            Markdown -> rich text (HTML + RTF + plain). Pasting keeps formatting
ct md              rich text (HTML/RTF) -> Markdown source
ct plain           Markdown or rich text -> plain text, syntax stripped, bullets as "•"
ct html            Markdown -> HTML source, as plain text
ct text            keep only the plain-text flavor (drop HTML/RTF)

-i FILE            read FILE ('-' = stdin) instead of the clipboard
-o                 print the result instead of writing the clipboard
-p                 print the result after writing the clipboard
```

Output is colored when writing to a terminal. Set `NO_COLOR` to disable.

## Resident mode and hotkey

```
ct daemon                          menu bar icon + global hotkey, stays in the foreground
ct daemon --hotkey cmd+shift+f9    pick another combination
ct install [--hotkey ...]          start the daemon now and at every login (LaunchAgent)
ct uninstall                       stop it and remove the LaunchAgent
```

The default hotkey is ⌃⌥⌘V. Pressing it pops up a menu at the mouse pointer that shows which
flavors are on the clipboard (plain text, HTML, RTF), a preview, and the target formats:
Rich text, Markdown, Plain text, HTML source, Text only. Press 1 to 5 or click one. The same menu
is available from the clipboard icon in the menu bar. No Accessibility permission is needed: the
hotkey is registered through the Carbon hotkey API.

Hotkey names: `cmd`, `ctrl`, `alt`, `shift`, letters, digits, `f1`..`f12`, `space`, `return`, `tab`, `escape`.

## How clipboard flavors work

The macOS clipboard holds several representations of the same content at once. The copying
app decides which ones to provide; the pasting app picks the richest one it understands.
Terminals and code editors always take plain text. Mail, Notes, Slack, Pages, Word and browsers
take HTML first, then RTF, then plain text. `ct show` tells you which flavors are present, so you
know what a paste will do. `ct rich` writes all three from Markdown; `ct text` keeps only plain
text so every app pastes it unchanged.

Limitation: RTF carries no heading level, so headings from an RTF-only source come back as bold
paragraphs. HTML sources keep them.

## License

MIT. John Knipper, <code@jkn.me>, http://jkn.me
