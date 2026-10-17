<p align="center"><img src="docs/logo.png" width="128" alt="QuickTranslate 아이콘"></p>

# QuickTranslate

[English](README.md) · 한국어

DeepL 데스크톱 클라이언트처럼 **⌘C 를 두 번** 누르면 선택한 텍스트의 번역 창이 바로 뜨는 macOS 메뉴바 앱입니다.
별도 API 키 없이 로컬에 설치된 **Claude Code CLI(Claude 구독)** 또는 **Codex CLI(ChatGPT 구독)** 를 그대로 사용합니다.

**Windows·Linux**에서는 같은 기능을 트레이 앱으로 제공합니다. 텍스트를 선택하고 **Ctrl+C를 두 번** 누르면 됩니다.
[Windows·Linux](#windowslinux) 항목을 참고하세요.

## 개인정보

번역할 텍스트는 선택한 CLI를 통해 본인 계정으로 Anthropic(Claude) 또는 OpenAI(Codex)에 전송됩니다.
그 외에는 아무것도 나가지 않습니다. 통계 수집, 크래시 리포트, API 키 모두 없습니다. 사용량은 일반 CLI 사용과 똑같이 구독에 포함됩니다.

## 요구 사항

- macOS 13 이상, Swift 5.9+ 툴체인 (Xcode Command Line Tools면 충분)
- 아래 중 하나 이상이 설치·로그인되어 있어야 합니다.
  - `claude` — [Claude Code](https://docs.claude.com/en/docs/claude-code) (`claude` 를 한 번 실행해 로그인)
  - `codex` — [Codex CLI](https://github.com/openai/codex) (`codex login`)

## 빌드 / 설치

```bash
cd macos
./build.sh            # macos/build/QuickTranslate.app 생성
./build.sh --install  # /Applications 에 복사 후 실행
```

기본 단축키 ⌘C ⌘C는 키보드 대신 **클립보드가 짧은 간격으로 두 번 바뀌는 것**을 감지하므로 **권한이 필요 없습니다.**
다른 단축키를 고르면 **손쉬운 사용(Accessibility)** 권한을 요청하며(시스템 설정 → 개인정보 보호 및 보안 → 손쉬운 사용),
허용하면 macOS가 키 이벤트를 전달하도록 앱이 한 번 자동 재시작됩니다.

> ad-hoc 서명(기본값)은 빌드할 때마다 서명이 바뀌어 기존 손쉬운 사용 권한이 조용히 무효화됩니다
> (목록에는 켜져 있는데 동작하지 않는 상태). `./build.sh --install` 은 설치 시 `tccutil reset` 으로
> 이 항목을 지워 다시 허용 창이 뜨게 합니다. 수동으로는 `tccutil reset Accessibility dev.swen.QuickTranslate`.
> 권한을 유지하려면 고정된 서명 ID가 필요합니다. `./make-cert.sh` 로 자체 서명 인증서("QuickTranslate Dev")를
> 로그인 키체인에 만들어 두면 `build.sh` 가 자동으로 그걸로 서명하며, 이후 재빌드해도 권한이 유지됩니다.
> Apple 개발자 인증서가 있으면 `CODESIGN_IDENTITY="Apple Development: ..." ./build.sh` 도 됩니다.
>
> 권한을 허용하면 앱을 재시작하지 않아도 2초 안에 자동으로 단축키 감지가 시작됩니다.

## 사용법

| 동작 | 방법 |
|---|---|
| 번역 창 열기 | 텍스트 선택 후 **⌘C ⌘C** (0.4초 안에 두 번). 설정에서 변경 가능 |
| 번역 언어 바꾸기 | 창 상단 언어 선택 (바꾸면 즉시 재번역) |
| 원문 수정 후 재번역 | 상단 원문 영역 편집 → **⌘⏎** |
| 번역 결과 복사 | **⇧⌘C** 또는 복사 버튼 |
| 창 닫기 | **Esc**, ⌘W, 창 닫기 버튼, 또는 창 바깥 클릭 (번역 중에는 바깥 클릭으로 닫히지 않음) |
| 최소화 / 확대 | ⌘M 또는 창 버튼. 다시 단축키를 누르면 복원됨 |
| 번역 중 취소 | **⌘.** 또는 취소 버튼 (이미 받은 부분은 남음) |
| 창 고정 | 📌 버튼 (바깥을 클릭해도 닫히지 않음) |

기본 동작: 텍스트를 **한국어**로 번역하고, 이미 한국어인 텍스트는 **영어**로 번역합니다. 메뉴바 아이콘 → 설정에서 바꿀 수 있습니다.

## 설정

메뉴바 아이콘 → **설정…**

- **엔진**: Claude (Claude Code CLI) / ChatGPT (Codex CLI)
- **모델**: Claude는 `haiku`(기본, 가장 빠름) / `sonnet` / `opus`, Codex는 비우면 기본 모델
- **Claude를 백그라운드에 상주**: 기본 켬. `claude` 프로세스(약 300MB)를 띄워둬서 번역 1건이 약 2초에서 0.7~1.3초로 줄어듭니다.
  끄면 번역마다 새 프로세스를 띄웁니다.
- **CLI 경로**: 자동 탐색이 실패하면 직접 지정 (`~/.local/bin/claude` 등)
- **언어**: 기본 번역 언어와, 원문이 이미 그 언어일 때 사용할 언어
- **표시 언어**: 시스템 설정 따름 / English / 한국어 / 日本語 (바꾸면 앱이 다시 시작됨)
- **단축키**: "변경…"을 누르고 원하는 조합을 누르면 됩니다(⌘·⌥·⌃ 중 하나 또는 펑션 키 필요). 실행 방식은
  **두 번 누르기**(기본 ⌘C ⌘C처럼 복사 단축키를 겹쳐 쓰는 방식)와 **한 번 누르기**(⌃⌥T 같은 전용 단축키.
  앱이 ⌘C를 대신 보내 선택 텍스트를 복사하고, 그 키 입력은 아래 앱에 전달되지 않음) 중 선택.
  감지 방식: ⌘C ⌘C는 클립보드 변화(권한 불필요, 보안 입력 무관), 그 외 두 번 누르기는 CGEvent 탭(손쉬운 사용 권한),
  한 번 누르기는 시스템 핫키 API
- **두 번 누름 인식 간격**, **바깥 클릭 시 닫기**

## 단축키가 안 잡힐 때

- **보안 키보드 입력(Secure Keyboard Entry)**: iTerm2, Ghostty, cmux 같은 터미널, 암호 입력창, 일부 Electron 앱이 이 기능을 켜면
  macOS가 모든 앱의 전역 키 감시를 막고, 그 앱이 백그라운드에 있어도 유지되는 경우가 있습니다. 기본 ⌘C ⌘C는 클립보드로
  감지하므로 영향이 없고, 한 번 누르기 단축키는 시스템 핫키 API를 씁니다. ⌘C가 아닌 조합을 두 번 누르는 설정만 키 이벤트에
  의존하며, 이때는 메뉴바 메뉴와 설정 창에 어느 앱이 켰는지 표시됩니다.
- **권한을 앱 실행 후에 허용한 경우**: macOS는 실행 시점의 권한으로 키 이벤트 전달 여부를 정하므로 앱이 자동으로 한 번 재시작합니다.

## 외부에서 번역창 열기

Raycast, Hammerspoon, 다른 단축키 앱에서 아래 알림을 보내면 클립보드 번역창이 열립니다.

```bash
osascript -l JavaScript -e 'ObjC.import("Foundation"); $.NSDistributedNotificationCenter.defaultCenter.postNotificationNameObject("dev.swen.QuickTranslate.translate", $())'
```

문제가 생기면 `~/Library/Logs/QuickTranslate.log` 를 확인하세요.

## Windows·Linux

[`desktop/`](desktop) 에 있는 별도의 Tauri 앱으로, 번역 창·엔진·언어·설정은 macOS 앱과 같습니다.
시스템 트레이(알림 영역)에 상주하며 쓰기 전까지는 창이 없습니다. 설치 파일은
[릴리스 페이지](https://github.com/longfin/quick-translate/releases)(`desktop-v*` 태그)에서 받을 수 있습니다.

### 요구 사항

- **Windows** 10(1803 이상) 또는 11, x64·ARM64. UI는 Microsoft Edge **WebView2** 런타임으로 그립니다.
  Windows 11과 업데이트된 Windows 10에는 이미 설치되어 있고, 없으면 설치 프로그램이 내려받습니다
  (포터블 zip은 내려받지 않으니 [Microsoft](https://developer.microsoft.com/microsoft-edge/webview2/)에서 설치하세요).
- **Linux** x86_64·aarch64, glibc 2.35 이상(Ubuntu 22.04, Debian 12, Fedora 36 이후), X11 또는 Wayland.
  트레이 아이콘에 `libayatana-appindicator3-1` 이 필요합니다(`.deb` 는 자동으로 설치). **GNOME**은 자체 트레이가 없어
  AppIndicator 확장이 필요합니다. Ubuntu는 기본으로 켜져 있고, 다른 배포판은 `gnome-shell-extension-appindicator` 를
  설치한 뒤 확장 앱에서 켜세요. KDE Plasma, Xfce, Cinnamon, MATE는 그대로 동작합니다.
- macOS와 마찬가지로 `claude` 또는 `codex` CLI가 설치·로그인되어 있어야 합니다. npm으로 설치했다면
  **Node.js**(18 이상)도 필요하며, CLI와 같은 폴더나 `PATH` 에 있어야 합니다.

### 설치

**Windows**

- **설치 프로그램**(`QuickTranslate-<버전>-windows-x64-setup.exe`, ARM PC는 `-arm64-`): 현재 사용자용으로 설치되어
  관리자 권한이 필요 없고, 시작 메뉴에 등록됩니다.
- **포터블**(`…-windows-x64-portable.zip` / `-arm64-`): 원하는 곳에 압축을 풀고 `quicktranslate.exe` 를 실행합니다.
  설정과 로그는 그래도 사용자 프로필에 저장됩니다.

코드 서명이 없는 빌드라 처음 실행할 때 SmartScreen이 "Windows의 PC 보호" 창을 띄웁니다.
**추가 정보 → 실행**을 누르세요.

**Linux**

- **AppImage**(`QuickTranslate-<버전>-linux-x86_64.AppImage` 또는 `-aarch64`), 배포판 무관:
  ```bash
  chmod +x QuickTranslate-*.AppImage
  ./QuickTranslate-*.AppImage
  ```
  AppImage는 FUSE 2가 필요합니다(`sudo apt install libfuse2`, Ubuntu 24.04는 `libfuse2t64`). 없으면
  `--appimage-extract-and-run` 옵션을 붙여 실행하세요.
- **Debian / Ubuntu**(`QuickTranslate_<버전>_amd64.deb` 또는 `_arm64.deb`):
  ```bash
  sudo apt install ./QuickTranslate_*_amd64.deb   # 이후 앱 메뉴에서 "QuickTranslate" 실행
  ```

체크섬은 함께 올라가는 `SHA256SUMS` 에 있습니다. 로그인할 때 자동으로 띄우려면 설정에서 **시스템 시작 시 실행**을 켜세요.

### 사용법

텍스트를 선택하고 **Ctrl+C Ctrl+C**(0.4초 안에 두 번)를 누르면 커서 옆에 번역 창이 뜹니다. 트레이 아이콘 메뉴에는
**클립보드 번역**, **설정…**, **종료**가 있습니다. 번역 창은 **Esc** 나 **Ctrl+W** 로 닫히고(번역 중이 아니면 바깥을
클릭해도 닫힘), **Ctrl+Enter** 로 수정한 원문을 다시 번역하며, 📌 버튼으로 고정하면 항상 위에 떠 있습니다.
설정 항목은 macOS 앱과 같고 **시스템 시작 시 실행**이 추가되어 있습니다.

**트리거 동작 방식.** Mac의 ⌘C ⌘C처럼 Ctrl+C Ctrl+C도 키보드가 아니라 클립보드로 감지합니다. 짧은 간격 안에
두 번 복사하면 번역을 시작합니다. 키보드 후킹도, 별도 권한도 필요 없습니다. 클립보드 변경은 Windows에서는
클립보드 리스너로, X11에서는 XFixes로, Wayland에서는 data-control 프로토콜을 지원하는 컴포지터(KDE Plasma, Sway,
Hyprland 등 wlroots 계열)에서 그 프로토콜로 전달받습니다.

**대신 단축키 쓰기.** 설정 → 실행 방식에서 **한 번 누르기**를 고르면 전역 단축키(기본 `Ctrl+Shift+T`)를 씁니다.
이미 클립보드에 있는 내용을 번역하므로 먼저 복사해 두어야 합니다.

**GNOME의 Wayland.** GNOME에는 data-control도, (이런 앱이 쓸 수 있는) 전역 단축키도 없습니다. XWayland가 떠 있으면
대신 XWayland의 클립보드를 감시하며, Wayland 앱에서 복사한 내용도 여기에 반영됩니다. XWayland가 없으면 트레이의
**클립보드 번역**을 쓰거나, GNOME 키보드 단축키(설정 → 키보드 → 사용자 지정 단축키)에 `quicktranslate --translate` 를
지정하세요. 어떤 감시 방식을 쓰는지는 로그(`trigger: clipboard watcher = …`)에 나옵니다.

**스크립트에서 열기.** 앱이 실행 중일 때 `quicktranslate --translate` 는 클립보드 번역 창을, `quicktranslate --settings` 는
설정 창을 엽니다(두 번째로 실행된 프로세스는 요청만 전달하고 종료). 명령은 `quicktranslate`(`.deb` 는
`/usr/bin/quicktranslate`), AppImage 파일 자체, Windows는 `quicktranslate.exe`(설치 프로그램은
`%LOCALAPPDATA%\QuickTranslate\`)입니다.

### 문제 해결

- **"`claude` CLI를 찾을 수 없습니다"**: `PATH` 와 흔한 설치 위치(npm 전역 폴더, Volta, fnm, nvm, bun, scoop, `~/.local/bin` 등)를
  찾아본 뒤 로그인 셸에 물어봅니다(Windows는 `where.exe`). 다른 곳에 있다면 설정의 **CLI 경로**를 지정하고 **테스트**를
  누르세요. 경로는 터미널에서 Windows는 `where claude`, Linux는 `command -v claude` 로 확인할 수 있습니다.
  Windows에서 npm은 `claude.cmd` 심(shim)을 만드는데, 이 파일이나 `claude.exe` 를 지정하면 됩니다.
- **터미널에서는 되는데 앱 메뉴로 실행하면 안 될 때**: 런처는 `.bashrc` 를 읽지 않아 거기서 설정한 `PATH` 가 빠집니다.
  앱은 CLI가 있는 폴더를 `PATH` 에 넣어 넘기므로 npm 심이 옆에 있는 `node` 를 찾을 수 있습니다(nvm, fnm). `node` 가
  다른 곳에 있다면 CLI 경로를 래퍼 스크립트로 지정하거나 Node를 시스템 전체에 설치하세요.
- **GNOME에서 트레이 아이콘이 안 보일 때**: AppIndicator 확장을 켜고(요구 사항 참고) 로그아웃 후 다시 로그인하세요.
  아이콘은 어두운 패널에 맞춘 흰색이라 밝은 패널에서는 잘 보이지 않습니다.
- **Ctrl+C Ctrl+C에 반응이 없을 때**: 로그에서 감시 방식을 확인하거나(위의 Wayland 참고) 단축키 방식으로 바꾸세요.
- **로그**: Windows는 `%LOCALAPPDATA%\QuickTranslate\QuickTranslate.log`, Linux는
  `~/.local/state/QuickTranslate/QuickTranslate.log`(`$XDG_STATE_HOME` 이 있으면 그 아래).
  설정 파일은 `%APPDATA%\dev.longfin.quicktranslate\settings.json`, `~/.config/dev.longfin.quicktranslate/settings.json` 입니다.

## 언어

앱 UI는 영어가 기본이며, 시스템 언어가 한국어·일본어면 자동으로 그 언어로 표시됩니다.
첫 실행 시 번역 기본 언어도 시스템 언어(그리고 두 번째 선호 언어)를 따릅니다. 다른 언어를 추가하려면
`macos/Resources/<lang>.lproj/Localizable.strings` 를 만들면 됩니다.

## 동작 방식

- ⌘C ⌘C는 `NSPasteboard.changeCount`를 60ms마다 확인해 감지합니다. 복사할 때마다 값이 오르므로 짧은 간격의 두 번 증가가
  곧 두 번 복사입니다. 다른 단축키는 CGEvent 탭(두 번 누르기) 또는 `RegisterEventHotKey`(한 번 누르기)를 씁니다.
  결과는 마우스 근처의 플로팅 패널(`NSPanel`)에 표시되며 현재 앱의 포커스를 뺏지 않습니다.
- Claude: `claude -p --output-format stream-json --include-partial-messages --tools "" --strict-mcp-config --setting-sources ""`
  로 실행해 토큰 단위 스트리밍으로 결과를 표시합니다. 사용자 설정·훅·MCP 서버를 로드하지 않아 빠르게 뜨고,
  확장 사고(thinking)는 꺼둡니다(`MAX_THINKING_TOKENS=0`).
- "Claude 상주"가 켜져 있으면 `claude -p --input-format stream-json` 프로세스 하나를 계속 살려둡니다. 번역 1건이 사용자 메시지 1개이고,
  결과를 받은 뒤 `/clear`를 보내 CLI가 로컬에서 대화를 비우므로 매번 새 컨텍스트로 시작하면서도 프로세스·연결·프롬프트 캐시는 warm 상태를 유지합니다.
  취소하거나 모델을 바꾸면 프로세스를 다시 띄웁니다.
- Codex: `codex exec -s read-only -o <file>` 로 실행하고 마지막 메시지를 읽습니다 (스트리밍 없음).

## 프로젝트 구조

`macos/` 는 macOS 앱입니다. `desktop/` 은 Windows·Linux용 Tauri 앱이며, 구조는
[`desktop/README.md`](desktop/README.md) 에 있습니다.

```
macos/Sources/QuickTranslate/
  main.swift                 앱 진입점 (메뉴바 전용, Dock 아이콘 없음)
  AppDelegate.swift          상태 표시줄 메뉴, 권한 처리, 클립보드 번역 트리거
  Hotkey.swift               단축키 모델, 키 이름, ⌘C 자동 전송
  PasteboardWatcher.swift    클립보드 변화로 ⌘C ⌘C 감지
  HotkeyMonitor.swift        그 외 두 번 누르기 감지 (CGEvent 탭)
  CarbonHotkey.swift         한 번 누르기 단축키 (RegisterEventHotKey)
  TranslationPanel.swift     플로팅 패널 (위치, 바깥 클릭 닫기, Esc)
  TranslationView.swift      패널 UI (SwiftUI)
  TranslationViewModel.swift 번역 상태 / 스트리밍 반영
  TranslationEngine.swift    claude / codex CLI 호출 및 출력 파싱
  ClaudeWorker.swift         상주 claude 프로세스 (stream-json 입출력, 턴 사이 /clear)
  ProcessJob.swift           자식 프로세스 실행 + stdout 라인 스트리밍
  CLILocator.swift           CLI 바이너리 탐색 (Finder에서 실행 시 PATH 보정)
  Settings.swift             UserDefaults 기반 설정
  SettingsView.swift         설정 창
```

## 기여

[CONTRIBUTING.md](CONTRIBUTING.md) (영어)를 참고하세요.

## 라이선스

[MIT](LICENSE)
