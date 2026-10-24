<p align="center"><img src="docs/logo.png" width="128" alt="QuickTranslate 아이콘"></p>

# QuickTranslate

[English](README.md) · 한국어

DeepL 데스크톱 클라이언트처럼 텍스트를 선택하고 **⌘C 를 두 번**(Windows·Linux에서는 **Ctrl+C 두 번**) 누르면
커서 옆에 번역 창이 바로 뜨는 **macOS·Windows·Linux**용 메뉴바/트레이 앱입니다. 별도 API 키 없이 로컬에 설치된
**Claude Code CLI(Claude 구독)** 또는 **Codex CLI(ChatGPT 구독)** 를 그대로 사용합니다.

세 플랫폼 모두 [`desktop/`](desktop) 의 [Tauri 2](https://v2.tauri.app) 앱 하나로 동작합니다. 다운로드는
[릴리스 페이지](https://github.com/longfin/quick-translate/releases)(`desktop-v*` 태그)에 있습니다.

> 0.1 이전의 macOS 앱은 별도의 Swift 앱이었습니다. 그 앱을 쓰고 있었다면
> [Swift 앱에서 옮겨 올 때](#swift-앱에서-옮겨-올-때)를 참고하세요.

## 개인정보

번역할 텍스트는 선택한 CLI를 통해 본인 계정으로 Anthropic(Claude) 또는 OpenAI(Codex)에 전송됩니다.
그 외에는 아무것도 나가지 않습니다. 통계 수집, 크래시 리포트, API 키 모두 없습니다. 사용량은 일반 CLI 사용과 똑같이 구독에 포함됩니다.

## 요구 사항

- 아래 중 하나 이상이 설치·로그인되어 있어야 합니다.
  - `claude` — [Claude Code](https://docs.claude.com/en/docs/claude-code) (`claude` 를 한 번 실행해 로그인)
  - `codex` — [Codex CLI](https://github.com/openai/codex) (`codex login`)

  npm으로 설치했다면 **Node.js**(18 이상)도 CLI 옆이나 `PATH` 에 있어야 합니다.
- **macOS** 13 이상, Apple 실리콘·Intel (유니버설 앱 하나).
- **Windows** 10(1803 이상) 또는 11, x64·ARM64. UI는 Microsoft Edge **WebView2** 런타임으로 그립니다.
  Windows 11과 최신 Windows 10에는 기본 설치되어 있고, 없으면 설치 프로그램이 내려받습니다(포터블 zip은
  내려받지 않으니 [Microsoft](https://developer.microsoft.com/microsoft-edge/webview2/)에서 설치하세요).
- **Linux** x86_64·aarch64, glibc 2.35 이상(Ubuntu 22.04, Debian 12, Fedora 36 이후), X11 또는 Wayland.
  트레이 아이콘에는 `libayatana-appindicator3-1` 이 필요합니다(`.deb` 가 함께 설치). **GNOME** 은 자체 트레이가
  없어 AppIndicator 확장이 필요합니다. Ubuntu는 기본으로 켜져 있고, 다른 배포판은
  `gnome-shell-extension-appindicator` 를 설치한 뒤 확장 앱에서 켜세요. KDE Plasma, Xfce, Cinnamon, MATE는 그대로 됩니다.

## 설치

**macOS**

`QuickTranslate-<version>-macos-universal.app.zip` 의 압축을 풀고 `QuickTranslate.app` 을 `/Applications` 로
옮깁니다. ad-hoc 서명만 되어 있고 공증(notarization)은 되어 있지 않아서, 내려받은 그대로는 Gatekeeper가 실행을
막습니다. 처음 한 번 격리 속성을 지워 주세요.

```bash
xattr -d com.apple.quarantine /Applications/QuickTranslate.app
```

메뉴바에만 나타납니다(Dock 아이콘 없음). 권한은 필요 없습니다. ⌘C ⌘C는 클립보드 변화로 감지하고, 단축키는
시스템 핫키 API를 쓰므로 손쉬운 사용 권한이 필요하지 않습니다.

**Windows**

- **설치 프로그램** (`QuickTranslate-<version>-windows-x64-setup.exe`, ARM PC는 `-arm64-`): 현재 사용자에게만
  설치되고 관리자 권한이 필요 없으며 시작 메뉴에 항목을 추가합니다.
- **포터블** (`…-windows-x64-portable.zip` / `-arm64-`): 아무 곳에나 풀고 `quicktranslate.exe` 를 실행합니다.
  설정과 로그는 사용자 프로필에 저장됩니다.

코드 서명이 없어서 처음 실행할 때 SmartScreen이 "Windows의 PC 보호" 경고를 띄웁니다. **추가 정보 → 실행** 을 누르세요.

**Linux**

- **AppImage** (`QuickTranslate-<version>-linux-x86_64.AppImage` 또는 `-aarch64`), 배포판 무관:
  ```bash
  chmod +x QuickTranslate-*.AppImage
  ./QuickTranslate-*.AppImage
  ```
  AppImage는 FUSE 2가 필요합니다(`sudo apt install libfuse2`, Ubuntu 24.04는 `libfuse2t64`). 없으면
  `--appimage-extract-and-run` 을 붙여 실행하세요.
- **Debian / Ubuntu** (`QuickTranslate_<version>_amd64.deb` 또는 `_arm64.deb`):
  ```bash
  sudo apt install ./QuickTranslate_*_amd64.deb   # 앱 메뉴에서 "QuickTranslate" 실행
  ```

다운로드 옆의 `SHA256SUMS` 에 체크섬이 있습니다. 로그인할 때 자동으로 띄우려면 설정에서 **시스템 시작 시 실행** 을 켜세요.

직접 빌드하려면 [`desktop/README.md`](desktop/README.md)를 보세요.

## 사용법

Windows·Linux에서는 ⌘ 대신 Ctrl입니다.

| 동작 | 방법 |
|---|---|
| 번역 창 열기 | 텍스트 선택 후 **⌘C ⌘C** (0.4초 안에 두 번). 단축키로 바꿀 수도 있습니다(설정 참고) |
| 클립보드 번역 | 메뉴바/트레이 아이콘 → **클립보드 번역** |
| 대상 언어 변경 | 창 위쪽 언어 선택 (바로 다시 번역) |
| 원문을 고쳐 다시 번역 | 위쪽 텍스트를 고친 뒤 **⌘⏎** |
| 번역 복사 | 복사 버튼 |
| 닫기 | **Esc**, **⌘W**, 또는 창 바깥 클릭 (번역 중에는 닫히지 않음) |
| 창 고정 | 📌 버튼 (항상 위에도 고정) |

창이 떠 있을 때 다시 트리거하면 텍스트를 바꾸고 그 자리에서 다시 번역합니다.

기본으로 시스템 언어로 번역하고, 텍스트가 이미 그 언어면 다음 선호 언어(없으면 영어)로 번역합니다. 설정에서 바꿀 수 있습니다.

## 설정

메뉴바/트레이 아이콘 → **설정…**. 바꾸는 즉시 저장되고 적용됩니다.

- **엔진**: Claude(Claude Code CLI) 또는 ChatGPT(Codex CLI)
- **모델**: Claude는 `haiku`(기본, 가장 빠름) / `sonnet` / `opus`, Codex는 비워 두면 기본 모델
- **CLI 경로**: 자동 감지가 안 될 때 지정 (예: `~/.local/bin/claude`). **테스트** 로 찾은 경로를 확인할 수 있습니다
- **언어**: 기본 대상 언어와, 텍스트가 이미 그 언어일 때 쓸 대체 언어
- **표시 언어**: 시스템 따름, 또는 English / 한국어 / 日本語
- **실행 방식**: **두 번 누르기**(⌘C ⌘C, 간격 조절 가능) 또는 **한 번 누르기**(키보드로 녹화하는 전역 단축키,
  기본 `Ctrl+Shift+T`). 단축키는 이미 클립보드에 있는 텍스트를 번역하므로 먼저 복사하세요.
- **창 바깥을 클릭하면 닫기**
- **Claude를 백그라운드에 상주시키기**: 기본 켜짐. 상주 `claude` 프로세스(RSS 약 240MB, 메모리 사용량 약 110MB)가
  번역마다 CLI 시작 시간(약 0.5초)을 아껴 줍니다. 끄면 번역할 때마다 새 프로세스를 띄웁니다.
- **시스템 시작 시 실행**: LaunchAgent(macOS), HKCU `Run` 키(Windows), XDG autostart 항목(Linux)

설정은 JSON으로 `~/Library/Application Support/dev.longfin.quicktranslate/settings.json`(macOS),
`%APPDATA%\dev.longfin.quicktranslate\settings.json`(Windows), `~/.config/dev.longfin.quicktranslate/settings.json`(Linux)에
저장됩니다. 키 목록은 [`desktop/README.md`](desktop/README.md#settings)에 있고, 실행 중인 앱은 손으로 고친 내용을 1초 안에 반영합니다.

## 트리거 동작 방식

⌘C ⌘C / Ctrl+C Ctrl+C는 키보드가 아니라 클립보드로 감지합니다. 간격 안에 두 번 복사되면 "번역"으로 봅니다.
키보드 훅도 권한도 필요 없고, 터미널이나 비밀번호 칸이 보안 키보드 입력을 켜 두어도 동작합니다. 클립보드 변화는
macOS에서는 `NSPasteboard.changeCount` 를 60ms마다 확인해서(페이스트보드에는 변경 알림이 없음), Windows에서는
클립보드 리스너로, X11에서는 XFixes로, Wayland에서는 data-control 프로토콜을 지원하는 컴포지터(KDE Plasma, Sway,
Hyprland 등 wlroots 계열)에서 받습니다.

**GNOME Wayland.** GNOME에는 data-control도, (이런 앱을 위한) 전역 단축키도 없습니다. XWayland가 떠 있으면 그
클립보드를 대신 지켜보는데, Wayland 앱에서 복사한 것도 따라옵니다. XWayland가 없으면 트레이의 **클립보드 번역** 을
쓰거나, GNOME 키보드 단축키(설정 → 키보드 → 사용자 지정 단축키)에 `quicktranslate --translate` 를 지정하세요.
어떤 감시 방식이 쓰이는지는 로그에 나옵니다(`trigger: clipboard watcher = …`).

## 외부에서 번역창 열기

앱이 실행 중일 때 `quicktranslate --translate` 는 클립보드로 번역 창을 열고 `quicktranslate --settings` 는 설정을
엽니다(두 번째 실행은 요청만 넘기고 종료). Raycast, Hammerspoon, GNOME 단축키, 아무 스크립트에서나 부를 수 있습니다.
명령 위치:

- macOS: `/Applications/QuickTranslate.app/Contents/MacOS/quicktranslate`
- Windows: `quicktranslate.exe` (설치 프로그램은 `%LOCALAPPDATA%\QuickTranslate\`)
- Linux: `quicktranslate` (`.deb` 는 `/usr/bin/quicktranslate`), 또는 AppImage 자체

## 문제 해결

- **"`claude` CLI를 찾을 수 없음"**: `PATH` 와 흔한 설치 위치(npm 전역 폴더, Volta, fnm, nvm, bun, scoop, Homebrew,
  `~/.local/bin` 등)를 찾아본 뒤 로그인 셸에 물어봅니다(Windows는 `where.exe`). 다른 곳에 있다면 설정에서 **CLI 경로** 를
  지정하고 **테스트** 를 누르세요(터미널에서 `command -v claude`, Windows는 `where claude` 로 경로 확인). Windows에서 npm은
  `claude.cmd` 셰임을 설치하니 그 파일이나 `claude.exe` 를 지정하면 됩니다.
- **터미널에선 되는데 앱에선 안 됨**: Finder, Dock, 런처로 띄운 앱은 셸 rc 파일을 읽지 않아서 거기서 설정한 `PATH` 가
  없습니다. 앱은 CLI가 있는 폴더를 `PATH` 에 넣어 넘기므로 npm 셰임이 옆의 `node` 를 찾습니다(nvm, fnm). `node` 가 다른
  곳에 있으면 CLI 경로를 래퍼 스크립트로 지정하거나 Node를 시스템에 설치하세요.
- **macOS에서 "손상되었기 때문에 열 수 없습니다"**: 격리 속성이 남아 있는 것입니다. [설치](#설치)를 참고하세요.
- **GNOME에서 트레이 아이콘이 안 보임**: AppIndicator 확장을 켜고(요구 사항 참고) 다시 로그인하세요. 아이콘은 어두운
  패널용 흰색이라 밝은 패널에서는 잘 안 보입니다.
- **⌘C ⌘C / Ctrl+C Ctrl+C가 반응하지 않음**: 로그에서 감시 방식을 확인하거나(위 Wayland 참고) 단축키로 바꾸세요.
- **로그**: `~/Library/Logs/QuickTranslate.log`(macOS), `%LOCALAPPDATA%\QuickTranslate\QuickTranslate.log`(Windows),
  `~/.local/state/QuickTranslate/QuickTranslate.log`(Linux, `$XDG_STATE_HOME` 이 있으면 그 아래).

## Swift 앱에서 옮겨 올 때

0.1 이전에는 macOS용으로 네이티브 Swift 앱(번들 ID `dev.swen.QuickTranslate`)을 배포했습니다. Tauri 앱이 이를
대체합니다([#20](https://github.com/longfin/quick-translate/issues/20)).

- 예전 앱을 종료하고 `/Applications` 에서 지우세요. 둘 다 ⌘C ⌘C에 반응합니다.
- 설정은 아직 옮겨 오지 않습니다. 엔진, 모델, 언어, 실행 방식을 다시 설정하세요.
- 예전 앱은 일부 단축키에 손쉬운 사용 권한이 필요했지만 이 앱은 필요 없습니다. 시스템 설정 → 개인정보 보호 및 보안 →
  손쉬운 사용의 예전 항목과 로그인 항목은 지워도 됩니다.
- 스크립트 트리거는 `dev.swen.QuickTranslate.translate` 분산 알림 대신 `quicktranslate --translate` 입니다.
- 한 번 누르기 단축키는 이미 클립보드에 있는 텍스트를 번역합니다. Swift 앱은 먼저 ⌘C를 보냈습니다.

## 언어

UI는 기본 영어이고 시스템 언어에 따라 한국어나 일본어로 자동 전환됩니다. 첫 실행 시 기본 번역 언어도 시스템 언어를
따릅니다. 문자열은 영어 원문을 키로 [`desktop/src/locales/`](desktop/src/locales)에 있습니다. 언어를 추가하려면
[CONTRIBUTING](CONTRIBUTING.md#adding-a-ui-language)을 보세요.

## 동작 방식

- Claude: `claude -p --output-format stream-json --include-partial-messages --tools "" --strict-mcp-config --setting-sources ""`
  를 실행해 토큰을 창에 스트리밍합니다. 이 플래그들은 사용자 설정, 훅, MCP 서버를 건너뛰어 빨리 시작하고 입력 토큰을
  수백 개로 줄입니다. 확장 사고(extended thinking)는 끕니다(`MAX_THINKING_TOKENS=0`).
- "Claude 상주"를 켜면 `claude -p --input-format stream-json` 프로세스 하나가 계속 떠 있습니다. 번역 하나가 사용자
  메시지 하나이고, 결과가 오면 CLI가 로컬에서 처리하는 `/clear` 를 보내서 매번 새 컨텍스트로 시작하면서도 프로세스,
  연결, 프롬프트 캐시는 따뜻하게 유지합니다. 취소하거나 모델을 바꾸면 프로세스를 다시 띄웁니다.
- Codex: `codex exec -s read-only -o <file>` 를 실행해 최종 메시지를 읽습니다(스트리밍 없음).
- 번역 창은 지금 쓰고 있는 앱의 포커스를 빼앗지 않고 커서 옆에 뜨는 웹뷰 창입니다(macOS에서는 non-activating `NSPanel`).

## 프로젝트 구조

```
desktop/    앱 (Tauri 2): src-tauri/ (Rust), src/ (웹 UI, 바닐라 TypeScript), locales
docs/       logo.png, icon-1024.png
.github/    CI와 릴리스 워크플로
```

앱 내부 구조는 [`desktop/README.md`](desktop/README.md)에 있습니다.

## 기여

[CONTRIBUTING.md](CONTRIBUTING.md)를 참고하세요.

## 라이선스

[MIT](LICENSE)
