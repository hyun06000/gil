# GIL 개인정보 처리방침 / Privacy Policy

최종 갱신 / Last updated: 2026-11-13 · 적용 대상 / Applies to: GIL (`gil.exe`, Microsoft Store 앱 `gil-companion`, GitHub 릴리스의 단일 실행 파일, macOS Plugin)

## 한국어

**GIL은 개인정보를 수집·저장·전송하지 않습니다.**

- GIL은 사용자의 PC에서만 실행되는 로컬 프로그램입니다. 사용자가 고른 프로젝트 폴더 안의 `.gil` 폴더에 작업 기록을
  저장하며, 그 기록은 사용자의 PC를 떠나지 않습니다. 원격 서버, 계정, 로그인은 없습니다.
- GIL은 Claude Desktop·Codex 같은 에이전트 앱이 로컬 자식 프로세스로 실행하는 MCP 서버입니다. 사용자가 에이전트 앱에
  입력한 내용과 그 앱이 외부로 보내는 데이터는 **해당 앱의 개인정보 처리방침**이 적용되며, GIL은 그 내용을 별도로 수집하거나
  어디에도 보내지 않습니다.
- `gil install`은 사용자의 PC 안에 있는 두 앱의 설정 파일(Claude Desktop의 `claude_desktop_config.json`, Codex의
  `config.toml`)에 GIL 실행 항목 하나를 추가합니다. 파일은 사용자 프로필 안에만 있고 외부로 전송되지 않으며, `gil uninstall`이
  같은 항목을 제거합니다.
- 유일한 네트워크 접근은 **업데이트 안내**입니다. 설치된 GIL은 하루 최대 한 번 공개 GitHub 저장소의 JSON 파일
  (`https://raw.githubusercontent.com/hyun06000/gil/main/distribution/update-feed.json`, 64KB 이하)을 읽어 새 버전이 있는지
  표시합니다. 이 요청에는 사용자 데이터, 식별자, 설치 정보가 담기지 않으며 GIL은 아무것도 내려받거나 설치하지 않습니다.
  환경 변수 `GIL_UPDATE_CHECK=off`로 끌 수 있습니다. GitHub가 이 요청을 처리할 때 적용되는 사항은 GitHub의 개인정보 처리방침을
  따릅니다.
- 원격 측정(telemetry), 사용 통계, 오류 보고, 광고, 추적 기술은 없습니다.
- 모든 소스 코드는 https://github.com/hyun06000/gil 에 공개되어 있어 위 내용을 직접 확인할 수 있습니다.

문의: https://github.com/hyun06000/gil/issues (보안 관련은 [SECURITY.md](SECURITY.md)를 따릅니다).

## English

**GIL does not collect, store or transmit personal information.**

- GIL is a local program that runs only on the user's own PC. It keeps its working records in a `.gil` folder inside the
  project folder the user chose; those records never leave the PC. There is no remote server, account or sign-in.
- GIL is an MCP server that agent apps such as Claude Desktop and Codex start as a local child process. What the user types
  into those apps, and what those apps send elsewhere, is governed by **those apps' privacy policies**; GIL neither collects
  nor transmits any of it.
- `gil install` adds one GIL entry to the settings files of two apps on the same PC (Claude Desktop's
  `claude_desktop_config.json`, Codex's `config.toml`). The files stay inside the user's profile and are not transmitted;
  `gil uninstall` removes the same entry.
- The only network access is **update guidance**: at most once a day the installed GIL reads a public JSON file in the GitHub
  repository (`https://raw.githubusercontent.com/hyun06000/gil/main/distribution/update-feed.json`, at most 64 KB) to show
  whether a newer version exists. The request carries no user data, identifier or installation details, and GIL never
  downloads or installs anything from it. It can be turned off with the environment variable `GIL_UPDATE_CHECK=off`. GitHub's
  own privacy statement applies to GitHub's handling of that request.
- There is no telemetry, usage statistics, crash reporting, advertising or tracking technology.
- All source code is public at https://github.com/hyun06000/gil, so every statement above can be verified.

Contact: https://github.com/hyun06000/gil/issues (security matters: see [SECURITY.md](SECURITY.md)).
