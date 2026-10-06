# Git 없는 Mac 설치 — preview.3 VM 검수

대상은 macOS Apple Silicon의 Codex다. Windows에 이 파일을 사용하지 않는다.
GIL 실행 파일은 바꾸지 않는다. Git clone 대신 **이미 공개한 고정 압축파일**을 HTTPS로 받고,
검증 후 로컬 마켓플레이스로 등록한다. 사용자에게 Git·Apple Command Line Tools·Node·Rust·
Homebrew·Companion 설치를 요구하지 않는다. Apple 서명·공증은 여전히 없으며 보안 차단은 우회하지 않는다.

## 사용자가 할 일

1. VM 안에 빈 작업 폴더를 만들고 Codex에서 선택한 뒤 새 대화를 연다.
2. 아래 요청을 복사한다. 개발 도구 설치나 보안 해제를 제안하면 진행하지 않는다.
3. 설치 뒤 새 대화에서 GIL을 시작한다. Agent의 성공 문장과 실제 그래프 표시를 구분한다.

### VM의 Codex에 보낼 요청

```text
Git이나 Apple Command Line Tools 없이 GIL preview.3를 설치해 줘.
앱에 포함된 Codex CLI와 macOS 기본 도구만 사용해 줘.

먼저 현재 GIL 플러그인과 gil-preview-macos-arm64 마켓플레이스 등록을 확인해 줘.
이미 등록되었으면 자동으로 교체·제거하지 말고 출처와 버전을 알려준 뒤 승인을 받아 줘.

내가 승인하는 배포물은 macOS Apple Silicon용 unsigned preview 0.2.1-preview.3야.
아래 HTTPS 파일을 내려받고, 압축 해제 전에 SHA-256이 정확히 일치하는지 검사해 줘.
https://github.com/hyun06000/gil-marketplace/releases/download/v0.2.1-preview.3/gil-codex-macos-arm64-0.2.1-preview.3.tar.gz
SHA-256: f484c09919c3c58e722ef12a466158bcb478e56bada2e216b7cbbcdfc3102d1c

일치하면 내 사용자 Library/Application Support/GIL 아래의 새 전용 폴더에 풀어 줘.
기존 폴더는 덮어쓰지 마. .agents/plugins/marketplace.json이 들어 있는 폴더가 등록할 루트야.
그 절대 경로를 codex plugin marketplace add에 전달하고,
codex plugin add gil-companion-prototype@gil-preview-macos-arm64로 설치해 줘.
마켓플레이스 등록에는 GitHub 주소나 --ref를 사용하지 마. 로컬 폴더 경로를 사용해야 해.

설치 후 실제 등록 출처가 local인지, 버전이 0.2.1-preview.3인지 확인해 줘.
Git·Node·Rust·Homebrew·Companion을 설치하지 마. 설정이나 설치 cache를 손으로 수정하지 마.
보안 검사를 끄거나 quarantine을 제거하지 마. 네트워크 승인이 필요하면 요청해 줘.
해시 불일치나 설치 오류가 나면 멈추고 원문 오류를 알려줘.
프로젝트를 초기화하거나 기존 기록을 바꾸지 마. 화면이 보이기 전에는 Monitor가 열렸다고 말하지 마.
```

## 유지관리자용 준비 스크립트

Agent가 사용자 대신 위 순서를 수행하는 것이 목표다. 수동 shell 입력은 사용자 필수 절차가 아니다.
저장소의 `prepare-gitless-macos.sh`는 고정 URL·SHA-256·대상 CPU를 확인하고 새 폴더에만 압축을 푼다.
임의 archive를 받는 `--archive` 시험 옵션도 **동일한 고정 해시**만 허용한다.
준비 스크립트 자체는 실행 파일을 시작하거나 Plugin을 설치하지 않는다.

```sh
# 부모 폴더를 준비하고, 아직 존재하지 않는 전용 목적지를 지정한다.
mkdir -p "$HOME/Library/Application Support/GIL"
/bin/sh prepare-gitless-macos.sh --destination "$HOME/Library/Application Support/GIL/preview.3-local"
# 성공 후에만, 기존 등록과 충돌하지 않는지 확인하고 공식 명령으로 설치한다.
codex plugin marketplace add "$HOME/Library/Application Support/GIL/preview.3-local" --json
codex plugin add gil-companion-prototype@gil-preview-macos-arm64 --json
```

받아 둔 마켓플레이스 폴더는 임시 다운로드가 아니라 **설치 출처**이므로 유지한다.
업데이트는 검증한 새 버전·새 폴더를 준비한 후 사용자 승인과 공식 명령으로 출처를 바꾼다.
로컬 출처에 Git ref 갱신을 적용하거나 같은 버전의 파일을 덮어쓰지 않는다.
제거도 공식 Plugin 관리 기능으로 한다. `.gil`과 사용자 작업 폴더는 삭제하지 않는다.
실패한 준비 폴더나 다운로드는 자동 삭제하지 않는다. 무엇이 생성되었는지 확인한 뒤 별도로 정리한다.

## 확인한 범위와 남은 검수

- 공식 문서는 [로컬 마켓플레이스 경로 등록](https://developers.openai.com/plugins/build/plugins#add-a-marketplace-from-the-cli)을 지원한다.
- Codex CLI 0.160.1에서 Git·xcrun·xcode-select·개발 도구 실행을 거부하는 추가 sandbox로 검증했다.
  Git 실행이 실제로 거부되는 음성 대조도 확인했다.
- 기존 공개 archive의 HTTPS 다운로드·해시 검증·한글/공백 경로 압축 해제·로컬 catalog 조회 성공.
- 도구와 hook이 없는 별도 진단용 Plugin으로 공식 marketplace 등록 → 설치 → Plugin 제거 →
  marketplace 제거에 성공했다. 기존 GIL의 설치 출처나 cache는 교체하지 않았다.
- 위 대조는 개발 Mac에서 **Git 실행을 금지한 시험**이지 Command Line Tools가 물리적으로 없는 VM의
  설치 인수를 대신하지 않는다. 실제 GIL VM 설치·최초 실행·fullscreen·노드 상세는 사용자 검수 대기다.
- 제품 payload·version·hash·저장 형식·MCP 도구와 공개 release asset은 변경하지 않았다.

## 이번에 드러난 설치 안내 결함

빈 Codex 대화에 로컬 실행 도구가 없을 수 있고, Git-backed 마켓플레이스는 Git 없는 Mac에서
복제하지 못했다. GIL runtime이 개발 도구를 요구하지 않는다는 사실을 설치 경로 전체의 무의존성으로
일반화하면 안 된다. 설치 완료 기준에는 이 최초 다운로드/등록 단계와 실제 화면을 함께 포함한다.
