QuickTranslate for **Windows and Linux** {{VERSION}}: select text, press **Ctrl+C twice**, and a
translation pops up next to the cursor. It uses the `claude` (Claude Code) or `codex` CLI you
already have installed and logged in; no API keys. The macOS app is released separately.

What's new: see [desktop/CHANGELOG.md](https://github.com/longfin/quick-translate/blob/desktop-v{{VERSION}}/desktop/CHANGELOG.md).
Requirements, setup and troubleshooting: [README → Windows & Linux](https://github.com/longfin/quick-translate#windows--linux).

## Downloads

| | File |
| --- | --- |
| Windows x64, installer | `QuickTranslate-{{VERSION}}-windows-x64-setup.exe` |
| Windows ARM64, installer | `QuickTranslate-{{VERSION}}-windows-arm64-setup.exe` |
| Windows x64 / ARM64, portable | `QuickTranslate-{{VERSION}}-windows-x64-portable.zip`, `…-windows-arm64-portable.zip` |
| Linux x86_64 / aarch64, any distro | `QuickTranslate-{{VERSION}}-linux-x86_64.AppImage`, `…-linux-aarch64.AppImage` |
| Debian / Ubuntu | `QuickTranslate_{{VERSION}}_amd64.deb`, `QuickTranslate_{{VERSION}}_arm64.deb` |

Check a download with `sha256sum -c SHA256SUMS --ignore-missing` (Linux) or
`Get-FileHash <file>` (PowerShell) against `SHA256SUMS`.

## These builds are not signed

- **Windows**: Microsoft Defender SmartScreen warns about an unrecognized app when you start the
  installer or `quicktranslate.exe`. Click **More info → Run anyway**. The installer is per user
  and needs no administrator rights.
- **Linux**: make the AppImage executable first: `chmod +x QuickTranslate-*.AppImage`, then run
  it. The `.deb` installs with `sudo apt install ./QuickTranslate_{{VERSION}}_amd64.deb`.

Linux builds need glibc 2.35 or newer (Ubuntu 22.04, Debian 12, Fedora 36 and later). On GNOME the
tray icon needs the AppIndicator extension.
