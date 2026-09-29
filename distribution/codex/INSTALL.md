# GIL preview 설치

대상: **macOS Apple Silicon의 Codex** · `0.2.1-preview.2`.
Apple Developer ID 서명·공증은 없으며, 개발 도구 없는 새 Mac 검수는 아직 하지 않았다.
macOS가 실행을 막으면 중단한다. Gatekeeper 해제·quarantine 제거·설치 cache 수정으로 우회하지 않는다.
Windows·Intel Mac·Claude 작업 모드는 이번 배포의 지원 대상이 아니다.

## Codex에게 설치를 부탁하기

아래 요청을 Codex에 전달한다. 사용자에게 Node·npm·Cargo·Homebrew 설치나 소스 빌드를 요구하지 않는다.

> GIL의 unsigned preview 0.2.1-preview.2를 설치해 줘.
> 공식 Plugin 관리 기능으로 https://github.com/hyun06000/gil-marketplace.git 을
> commit 076bb49719ffd94225c2c0adf479607e42fb09e2 에 고정해서 등록하고,
> gil-companion-prototype@gil-preview-macos-arm64 를 설치해 줘.
> 기존 GIL 설치가 있다면 중복 등록하지 말고 출처와 버전을 먼저 확인해 줘.
> 내 프로젝트를 초기화하거나 기록을 바꾸지는 마.

이는 검수된 Git marketplace 설치다. 범용 Plugins Directory 검색 목록에 등재됐다는 뜻은 아니다.
소스 저장소 `hyun06000/gil`이 아니라 **`hyun06000/gil-marketplace`가 설치 출처**다.
앱에서 설치 승인을 요청하면 대상과 버전을 확인한다. 설치 후에는 **새 대화**를 연다.
기존 연결은 이전 프로세스를 유지할 수 있다.

## 시작하기

1. Codex의 폴더 선택 UI에서 작업할 폴더를 선택한다.
2. 새 프로젝트라면 **“지금 폴더에서 GIL 프로젝트를 시작해 줘”**라고 말한다.
   이미 GIL 기록이 있다면 **“기록을 바꾸지 말고 GIL Monitor를 열어 줘”**라고 말한다.
3. 원하는 일을 이야기한다. Monitor의 가로 그래프와 채팅을 함께 본다.
   fullscreen이 자동으로 열리지 않으면 앱의 **모니터 펼치기**를 누른다.
4. 노드를 눌러 상세가 보이는지 직접 확인한다. 도구의 성공 응답만으로 화면 성공을 판정하지 않는다.

Plugin 하나에 Rust Core·MCP server·Monitor UI가 들어 있다. **Companion을 따로 설치할 필요는 없다.**
Monitor는 읽기 전용이며, 프로젝트 변경은 사용자가 요청한 Agent의 GIL 행동을 통해서만 한다.
프로젝트 기록은 선택한 폴더의 `.gil`에, 재연결용 폴더 선택은 로컬 GIL 설정에 남는다.
GIL은 별도 클라우드 서비스에 기록을 업로드하지 않지만, Agent가 호출한 도구 응답은 사용 중인
Host/AI의 대화로 전달된다. 그 서비스의 데이터 정책과 폴더 권한을 함께 확인한다.

## 업데이트·제거·복구

unsigned preview는 자동으로 최신 main을 따라가지 않는다. 새 버전의 변경 내용·출처·검수 범위를
확인하고 승인한 뒤 공식 Plugin 관리 기능으로 갱신한다. 같은 버전의 파일을 덮어쓰지 않는다.

제거는 Codex의 Plugin 관리 화면에서 한다. **프로젝트 폴더나 `.gil`을 지우지 않는다.**
화면이 사라져도 프로젝트의 Journey가 삭제됐다는 뜻은 아니다. 다시 설치한 뒤 같은 폴더를 고른다.

이전 검증판은 `0.2.1-preview.1`, marketplace commit
`e63963db63f0bfaf11be9d7939873e7a31fe05be`다. 복구가 필요하면 Codex에게 현재 설치를 확인한 뒤
같은 marketplace를 이 commit으로 교체해 달라고 요청한다. 이는 **Plugin 버전 복구**이지
프로젝트 Artifact를 되돌리는 `gil restore`가 아니다. 사용자 동의 없이 실행하지 않는다.
preview.2 → preview.1 → preview.2 공식 설치와 Project·선택 설정 보존을 기존 Mac에서 확인했다.

오류가 나면 OS/CPU, Codex 버전, Plugin 버전과 실제 화면을 아는 만큼만
[GIL Issues](https://github.com/hyun06000/gil/issues)에 알려 준다. 개인 경로·대화·token·`.gil` 기록은
첨부하지 않는다. 보안 취약점은 공개 issue 대신 [보안 안내](../../SECURITY.md)를 따른다.

## Agent·유지관리자용 공식 명령

앱에 제공된 Codex CLI를 사용한다. 사용자의 시스템에 별도 CLI 설치를 요구하지 않는다.
새 설치의 명령 형태는 다음과 같고, 기존 marketplace가 있으면 먼저 등록 상태와 동의를 확인한다.

```sh
codex plugin marketplace add https://github.com/hyun06000/gil-marketplace.git --ref 076bb49719ffd94225c2c0adf479607e42fb09e2 --json
codex plugin add gil-companion-prototype@gil-preview-macos-arm64 --json
```

공식 CLI가 실패하면 설치 cache나 Host 설정을 손으로 고쳐 성공처럼 만들지 않는다.
[Codex 공식 Plugin 안내](https://developers.openai.com/plugins/build/plugins)를 따른다.

원본 CI archive의 SHA-256:
`b1b969185e69e9c4e0549d7a37188f05e4ae60c1c9f8edc8d84b36f7b67fc6a8`.
이 값은 GitHub가 생성하는 Source ZIP의 해시가 아니다. 해시는 파일 일관성이지 Apple 서명·
공증이나 publisher 인증의 대체물이 아니다. 공개 게시 상태와 증거는
[배포 장부](PREVIEW-2-PUBLICATION-20260930.md)를 확인한다.
