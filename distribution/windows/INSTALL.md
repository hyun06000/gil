# GIL Windows 친구용 preview 설치

전체 안내의 시작점은 [저장소 설치 안내](../../INSTALL.md)다. 이 문서는 Windows 전용 상세 절차다.

현재 설치본은 **Windows 11 x64 친구 테스트용 unsigned prerelease**다. 정식 공개판이나 서명된
publisher 배포가 아니다. ARM64 Windows, 기업 배포와 일반 사용자 지원 완료를 주장하지 않는다.

- release: [windows-owner-update-0.1.1-owner.5](https://github.com/hyun06000/gil/releases/tag/windows-owner-update-0.1.1-owner.5)
- installer: `GIL.Companion.Unsigned.Test._0.1.1-owner.5_x64-setup.exe`
- 크기: `5,545,097` bytes
- SHA-256: `ce4b91c56164e4705cd054e486b54def06d9e86f174fc08fe49632599ef9a866`
- 표시 이름: **GIL Companion (Unsigned Test)**

이 파일은 Authenticode 서명이 없다. hash 일치는 바이트 무결성 확인이며 publisher 인증은 아니다.
사용자는 테스트 목적과 출처를 알고 설치한다. Windows 보안 기능을 끄거나 Defender·SmartScreen·
실행 정책·ACL을 변경하지 않는다. 정상적인 OS 확인 절차로도 실행할 수 없으면 중단한다.

## 사용자 설치

1. 위 release에서 정확한 x64 installer를 다운로드한다. 브라우저의 임시
   `release-assets.githubusercontent.com` URL을 공유하지 말고 고정 release 페이지를 공유한다.
2. 파일명·크기·SHA-256을 확인한 뒤 더블클릭한다.
3. 설치 창이 완료되면 시작 메뉴의 **GIL Companion (Unsigned Test)**를 연다.
4. Companion에서 **Codex 연결**을 누른다.
5. 이전 GIL Plugin 또는 `gil` MCP 등록이 있으면 Companion의 **이전 GIL 연결 교체** 내용을 확인하고
   승인한다. 다른 Plugin과 Project는 대상이 아니다.
6. 연결이 완료되면 Codex를 다시 열고 새 대화에서 테스트 폴더를 선택한다.
7. **“지금 폴더에서 GIL 프로젝트를 시작해 줘”**라고 말하고 실제 Monitor·첫 Interview·파일 작업을 확인한다.

테스트 폴더는 우선 OneDrive 등 동기화 폴더 밖의 로컬 폴더를 사용한다. `.gil` 권한 거절이 나오면
ACL 상속을 강제로 바꾸거나 Project를 Agent가 자동 이동하지 않는다. 사용자가 로컬 폴더를 만들고 그
폴더를 Codex 작업 폴더로 선택한 뒤 다시 시작한다.

## 설치를 돕는 Agent의 규칙

- OS가 Windows이고 기계 CPU가 x64인지 먼저 확인한다.
- GitHub release의 자산명·크기·SHA-256을 대조한다. 임시 서명 URL이나 이름이 비슷한 옛 installer를
  대신 사용하지 않는다.
- 다운로드·실행·OS 확인처럼 사용자가 해야 할 클릭을 분명히 구분한다.
- 설치 후 Companion 창이 실제로 나타나는지 확인한다. 프로세스가 존재한다는 사실만으로 UI 성공을
  주장하지 않는다.
- Codex 등록은 Companion의 버튼과 공식 `codex mcp` 명령만 사용한다. `config.toml`, registry,
  Plugin cache를 손으로 편집하지 않는다.
- 기존 GIL을 발견하면 출처·command·args를 보여 주고 사용자 승인 전에는 제거하지 않는다.
- 설치·연결 성공과 Project/Monitor 동작 성공을 별도로 확인한다.

사용자가 Codex에 도움을 요청할 때는 다음처럼 말할 수 있다.

```text
Windows 11 x64용 GIL Companion unsigned test 0.1.1-owner.5 설치를 도와줘.
고정 release https://github.com/hyun06000/gil/releases/tag/windows-owner-update-0.1.1-owner.5 의
GIL.Companion.Unsigned.Test._0.1.1-owner.5_x64-setup.exe만 사용해.
크기 5545097 bytes와 SHA-256 ce4b91c56164e4705cd054e486b54def06d9e86f174fc08fe49632599ef9a866를
확인하고, 내가 해야 할 다운로드·실행·OS 승인과 네가 확인할 단계를 구분해 줘.
기존 GIL이 있으면 출처와 버전을 먼저 보여 주고 교체 전에 물어봐.
보안 설정, registry, Codex 설정 파일이나 Plugin cache를 직접 수정하지 마.
프로젝트나 .gil 기록을 삭제·이관하지 마.
```

## 복구 설치와 기존 GIL

같은 installer를 다시 실행하는 것은 지원되는 복구 설치다. 이미 존재하는 동일 버전의 불변 Core를
거부 오류로 끝내지 않고, Core bytes를 보존한 채 Companion과 선택 기록을 복구해야 한다.

Companion의 **이전 GIL 연결 교체**는 이름이 같은 항목을 무조건 삭제하지 않는다. 기존 Plugin 또는
MCP 등록이 실제 GIL인지 확인하고, 사용자가 승인한 경우에만 공식 Codex CLI로 제거한 뒤 설치된 고정
launcher를 등록한다. 고정 launcher는 `%LOCALAPPDATA%\GIL\runtime\gil-launcher.exe`이며 버전별
Core나 Host Plugin cache의 실행 파일을 직접 등록하지 않는다.

## 사용자 주도 업데이트

자동 업데이트는 사용하지 않는다.

1. Companion에서 **업데이트 확인**을 누른다.
2. 새 버전·변경 설명을 읽는다.
3. 사용자가 **업데이트**를 눌렀을 때만 다운로드와 검증을 진행한다.
4. 검증이 통과하면 설치 창이 바로 열린다.
5. 기존 Codex와 GIL을 미리 닫을 필요는 없다. 기존 연결은 기존 Core를 계속 사용하고 새 연결부터
   새 버전을 사용한다.
6. 새 Codex 대화에서 실제 Core가 바뀌었는지 확인한다.

다운로드·서명·version binding·설치 중 하나라도 실패하면 오류 원문을 보존하고 기존 선택과 Project를
유지한다. 설정이나 update manifest를 손으로 고쳐 성공처럼 만들지 않는다.

`0.1.1-owner.3` updater는 사용자가 누른 설치와 version-bound updater signature 흐름을 실제 Windows
x64 PC에서 확인한 친구용 build다. Authenticode publisher 신뢰나 SmartScreen 평판을 통과했다는 뜻은
아니다.

`0.1.1-owner.5`는 PR #51의 Interview 종료 안내와 Verify 증거 필수 조건을 포함한다.
Verify마다 실제 파일을 저장하고 `evidence_files`로 지정해야 하며, 해당 Snapshot에서 해석과 배움으로
이어지는 참조를 기록한다. 채팅만으로 종료하거나 변하지 않은 이전 증거만 제출하면 거절한다.
기존 기록은 유지한다.

[빌드 및 설치 CI](https://github.com/hyun06000/gil/actions/runs/40551619152)에서 설치·재설치·기존 MCP
연결을 유지한 버전 전환·다음 연결의 새 Core 선택을 확인했다. 합성 자료의 Luna 시험에서는 직접
MCP가 실험 두 개·문서 두 개, Plugin 지침 경로가 실험 세 개·문서 세 개를 남기고 종료했다.
실제 Windows Codex 대화에서의 인수는 이번 업데이트 후 사용자 시험 대상이다.
업데이트 후 Companion 표시가 `0.1.1-owner.5`인지 확인하고, 새 GIL 연결·새 대화에서 Luna로 시험한다.
기존 연결을 새 Core로 강제 교체하지 않는다. macOS Plugin은 이 Windows 업데이트로 교체되지 않는다.

## 연결 해제와 제거

제거 순서는 다음과 같다.

1. Companion의 **Codex 연결 해제**를 누른다.
2. 열린 GIL 대화를 닫는다.
3. Windows의 앱 제거에서 **GIL Companion (Unsigned Test)**를 제거한다.

연결 해제는 command가 설치된 고정 launcher이고 args가 정확히 `mcp --serve`인 소유 GIL 등록만
제거한다. 이름이 같은 다른 등록은 지우지 않는다. 현재 uninstaller는 실행 중인 연결과 기록을 보호하기
위해 `%LOCALAPPDATA%\GIL\runtime`과 모든 Project의 `.gil`을 보존한다. 완전 runtime 삭제는 아직
일반 사용자 기능으로 제공하지 않는다.

## 확인한 범위와 남은 한계

확인한 범위:

- current-user NSIS 설치와 같은 버전 복구 설치
- 이전 runtime 보존, 버전별 Core와 고정 launcher 선택
- Companion 창 표시
- Companion의 Codex 연결·이전 GIL 교체 흐름
- 실행 중 연결을 유지한 side-by-side 업데이트와 새 연결의 새 Core 선택
- 사용자가 누르는 업데이트에서 설치 창 열기
- 연결 해제·Companion 제거 후 runtime/Project 보존
- 실제 Windows x64 사용자 설치·연결·간단한 파일 작업·업데이트

남은 한계:

- Authenticode code signing과 publisher 표시
- SmartScreen 평판
- 일반 외부 사용자 지원과 clean-PC 반복 인수
- Windows ARM64 제품 installer
- 완전 runtime 정리 버튼

개발용 `windows-preview-*` tar archive를 일반 사용자 installer로 사용하지 않는다. CI 통과만으로 실제
Codex Desktop 화면 성공을 주장하지 않는다. 구현·게이트 상세는 [Windows 배포 상태](README.md)와
[Companion 설치·업데이트 계약](../../spec/GIL_Companion_Install_Update_v0.1.md)을 따른다.
