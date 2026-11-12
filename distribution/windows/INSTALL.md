# GIL Windows 친구용 preview 설치

이번 **owner.15**는 macOS preview.29와 같은 최신 공통 Core·Monitor 개선을 포함한다.
계획 근거의 입력 조건을 호출 전에 안내하고 잘못된 모든 줄을 한 번에 표시하며, PNG 제한값과 실제 초과 크기를 안내한다.
시각자료 typed close·계획 안내·Monitor 표시 상태 개선, 훅 없는 프로젝트별 새 세션 재개도 포함한다.
참조 자동 이동 없이 기존 승인·안내 파일·기록·Snapshot을 보존한다. 새 대화·새 빈 폴더에서 필드테스트한다.

> **구형 채널 안내 (2026-11-12).** 현행 Windows 설치는 루트 [INSTALL.md](../../INSTALL.md)의 단일 실행 파일 경로다.
> 이 문서는 NSIS Companion `0.1.1-owner.15` 사용자를 위한 과거 절차이며, 새 설치에는 쓰지 않는다.

전체 안내의 시작점은 [저장소 설치 절차서](../../INSTALL.md)다. 이 문서는 구형 Windows Companion 채널의 상세 절차다.

현재 설치본은 **Windows 11 x64 친구 테스트용 unsigned prerelease**다. 정식 공개판이나 서명된
publisher 배포가 아니다. ARM64 Windows, 기업 배포와 일반 사용자 지원 완료를 주장하지 않는다.

- release: [windows-owner-update-0.1.1-owner.15](https://github.com/hyun06000/gil/releases/tag/windows-owner-update-0.1.1-owner.15)
- installer: `GIL.Companion.Unsigned.Test._0.1.1-owner.15_x64-setup.exe`
- 크기: `6,113,782` bytes
- SHA-256: `fefcd6c22cf787130db0a63151af869c64caac393d1a15d22dcfe9b11f3aeaf2`
- 표시 이름: **GIL Companion (Unsigned Test)**

이 파일은 Authenticode 서명이 없다. hash 일치는 바이트 무결성 확인이며 publisher 인증은 아니다.
사용자는 테스트 목적과 출처를 알고 설치한다. Windows 보안 기능을 끄거나 Defender·SmartScreen·
실행 정책·ACL을 변경하지 않는다. 정상적인 OS 확인 절차로도 실행할 수 없으면 중단한다.

## 사용자 설치

1. 위 release에서 정확한 x64 installer를 다운로드한다. 브라우저의 임시
   `release-assets.githubusercontent.com` URL을 공유하지 말고 고정 release 페이지를 공유한다.
2. 파일명·크기·SHA-256을 확인한 뒤 더블클릭한다.
3. 설치 창이 완료되면 시작 메뉴의 **GIL Companion (Unsigned Test)**를 연다.
4. 쓰는 Host의 버튼을 누른다. 버튼은 그 Host 하나만 바꾼다.
   - **Codex 연결** — 공식 `codex mcp add` 명령으로 고정 launcher를 등록한다.
   - **Claude Desktop 연결** — Claude Desktop을 **완전히 종료한 뒤** 누른다. Companion이
     `%APPDATA%\Claude\claude_desktop_config.json`의 `mcpServers.gil`에 같은 고정 launcher를 적는다.
     Desktop이 실행 중이면 거절한다(실행 중 바깥에서 바꾼 설정은 Desktop이 되돌릴 수 있다). 바꾸기 전
     같은 폴더에 `claude_desktop_config.json.gil-backup-<시각>` 백업을 남기고, 다른 MCP 서버 항목은 그대로
     둔다. 이 버튼은 `0.1.1-owner.10`부터 제공한다. owner.8/owner.9에서는
     [상세 통합 안내](../INSTALL-GUIDE.md)의 “직접 등록(임시)” 항목을 따른다.
5. 이전 GIL Plugin 또는 `gil` MCP 등록이 있으면 Companion의 **이전 GIL 연결 교체** 내용을 확인하고
   승인한다. 다른 Plugin·다른 MCP 서버·Project는 대상이 아니다. Claude Desktop에서는 **GIL 실행 파일을
   가리키는** `gil` 항목만 교체 대상이고, 사용자가 직접 만든 다른 `gil` 등록은 “다른 gil 등록 있음”으로
   표시한 채 바꾸지 않는다.
6. Agent는 선택된 Core의 버전·해시와 아래 **기존 GIL 훅 확인**을 마친다. 구형 훅이 남아 있다면 해당 Host의 GIL 항목만 정리한다.
7. 연결이 완료되면 Host(Codex 또는 Claude Desktop)를 다시 열고 새 대화에서 테스트 폴더를 선택한다.
8. **“지금 폴더에서 GIL 프로젝트를 시작해 줘”**라고 말하고 실제 Monitor·첫 Interview·파일 작업을 확인한다.

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
- Claude Desktop 등록은 Companion의 **Claude Desktop 연결** 버튼을 사용한다. Agent가
  `claude_desktop_config.json`을 직접 고치는 것은 owner.8/owner.9처럼 버튼이 없는 설치본에서 사용자가 승인하고
  Claude Desktop이 완전히 종료된 경우에만 한다. 그때도 다른 항목은 그대로 두고 `mcpServers.gil`만 고정
  launcher로 적으며, 먼저 백업을 남긴다.
- 기존 GIL을 발견하면 출처·command·args를 보여 주고 사용자 승인 전에는 제거하지 않는다.
- 설치·연결 성공과 Project/Monitor 동작 성공을 별도로 확인한다.

사용자가 Codex에 도움을 요청할 때는 다음처럼 말할 수 있다.

```text
Windows 11 x64용 GIL Companion unsigned test 0.1.1-owner.15 설치를 도와줘.
고정 release https://github.com/hyun06000/gil/releases/tag/windows-owner-update-0.1.1-owner.15 의
GIL.Companion.Unsigned.Test._0.1.1-owner.15_x64-setup.exe만 사용해.
크기 6113782 bytes와 SHA-256 fefcd6c22cf787130db0a63151af869c64caac393d1a15d22dcfe9b11f3aeaf2를
확인하고, 내가 해야 할 다운로드·실행·OS 승인과 네가 확인할 단계를 구분해 줘.
기존 GIL이 있으면 출처와 버전을 먼저 보여 주고 교체 전에 물어봐.
보안 설정, registry, Codex 설정 파일이나 Plugin cache를 직접 수정하지 마.
프로젝트나 .gil 기록을 삭제·이관하지 마.
```

## 기존 GIL 훅 확인

이번 설치본은 전역·프로젝트 로컬·Plugin·Skill 훅을 등록하지 않는다. 다른 프로젝트의 도구 호출이나
턴 종료에 관여하지 않는다. 새 세션은 선택한 GIL 프로젝트의 안내와 MCP 응답으로 재개한다.
이는 OS 차단이 아니며 모델의 안내 준수는 필드테스트로 확인한다.

installer와 Companion 연결 버튼은 과거 Host 훅 설정을 자동 삭제하지 않는다. 이전 Core를 가리키는
GIL 훅이 있었다면 Agent가 [정확한 훅 제거 절차](../codex/WRITE-GUARD.md)에 따라 별도로 확인한다.
`current.json`으로 검증한 **owner.15 Core**의 `write-guard uninstall --auto --host codex` 또는
`--host claude`를 사용하고, 요청받은 Host만 정리한다. Claude 설정 변경은 앱을 완전히 종료한 뒤 진행한다.
이 CLI는 환경 변수 `HOME` 아래의 `.codex/hooks.json`·`.claude/settings.json`을 검사하므로, Agent는
그 위치가 실제 Host 사용자 설정 위치와 같은지 먼저 확인한다. Windows에서 `HOME`이 없거나 실제 위치와
다르면 경로를 추측하여 성공으로 처리하지 않는다. 원본 백업과 정확한 GIL 항목 diff를 준비하고 Host의
파일 편집 도구로 승인된 항목만 정리한다. 다른 훅·권한·MCP 등록·프로젝트 기록은 보존한다.
이 확인을 사용자에게 PowerShell 스크립트 실행이나 전역 훅 전체 삭제로 떠넘기지 않는다.

`not_installed`는 검사한 JSON의 결과다. 앱 재시작·새 세션 로딩 확인까지 완료했다는 뜻이 아니다.

## 복구 설치와 기존 GIL

같은 installer를 다시 실행하는 것은 지원되는 복구 설치다. 이미 존재하는 동일 버전의 불변 Core를
거부 오류로 끝내지 않고, Core bytes를 보존한 채 Companion과 선택 기록을 복구해야 한다.

Companion의 **이전 GIL 연결 교체**는 이름이 같은 항목을 무조건 삭제하지 않는다. 기존 Plugin 또는
MCP 등록이 실제 GIL인지 확인하고, 사용자가 승인한 경우에만 공식 Codex CLI로 제거한 뒤 설치된 고정
launcher를 등록한다. 고정 launcher는 `%LOCALAPPDATA%\GIL\runtime\gil-launcher.exe`이며 버전별
Core나 Host Plugin cache의 실행 파일을 직접 등록하지 않는다.

Core의 정확한 CLI 출력은 `gil 0.1.1-owner.15`이며, 새 연결의 MCP `serverInfo.version`과 agent probe도
`0.1.1-owner.15`여야 한다. PATH의 임의 실행 파일 대신 `current.json`이 선택한 Core와 해시를 대조한다.
owner.14 이하의 `gil 0.1.0`은 구형 Cargo 패키지 표기일 수 있으므로 버전 문자열만으로 설치 실패를 단정하지 않는다.

## 사용자 주도 업데이트

자동 업데이트는 사용하지 않는다.

1. Companion에서 **업데이트 확인**을 누른다.
2. 새 버전·변경 설명을 읽는다.
3. 사용자가 **업데이트**를 눌렀을 때만 다운로드와 검증을 진행한다.
4. 검증이 통과하면 설치 창이 바로 열린다.
5. 기존 Codex·Claude Desktop과 GIL 대화를 미리 닫을 필요는 없다. 기존 연결은 기존 Core를 계속 사용하고 새 연결부터
   새 버전을 사용한다.
6. 쓰는 Host의 새 연결·새 대화에서 실제 Core가 바뀌었는지 확인한다. Claude Desktop을 처음 연결하거나 등록을 바꿀 때만 Desktop을 완전히 종료한다.

다운로드·서명·version binding·설치 중 하나라도 실패하면 오류 원문을 보존하고 기존 선택과 Project를
유지한다. 설정이나 update manifest를 손으로 고쳐 성공처럼 만들지 않는다.

이번 `0.1.1-owner.15`는 [PR #73](https://github.com/hyun06000/gil/pull/73)의 승인 성공·조회 재시도 구분을 포함한다.
확인된 승인 판을 표시한 채 일시적 조회 실패를 회색 안내로 구분하고 2/4/8/15초 간격으로 다시 읽는다.
저장 응답이 불확실하면 성공으로 표시하거나 자동 재승인하지 않는다. 새 제안은 별도 승인이 필요하다.
기존 Claude Desktop 연결 버튼, 확대·미니맵·업데이트 안내와 복구 후 채팅 전달은 유지한다.
Mac preview.29와 같은 `approved-plan,visual-authoring,product-safety` 시험 프로필이며 [PR #86](https://github.com/hyun06000/gil/pull/86)과
[보완 PR #87](https://github.com/hyun06000/gil/pull/87)의 계획 입력·Closing 안내를 포함한다.
새 대화에서 **새 빈 테스트 폴더**를 선택한다. Monitor에서 제안 내용을 읽고 승인 버튼을 누른 뒤
Interview가 닫히고, 계획 항목별 Experiment와 실제 Verify 증거 파일로 진행되는지 확인한다.
새 프로젝트는 형식 7을 사용한다. 기존 기록의 형식은 자동 변환하지 않으며, 구버전으로 재설치해도
기록 형식은 내려가지 않는다. 닫힌 실패 Cycle의 revisit만 Monitor 승인으로 진행하며 일반 restore와 Step revisit는 계속 차단한다. 실사용 Luna 검수는 사용자 시험이다.
정확한 파일·CI·검증 범위는 [게시 장부](PUBLICATION-OWNER-15-20261110.md)를 따른다.

`0.1.1-owner.3` updater는 사용자가 누른 설치와 version-bound updater signature 흐름을 실제 Windows
x64 PC에서 확인한 친구용 build다. Authenticode publisher 신뢰나 SmartScreen 평판을 통과했다는 뜻은
아니다.

이전 `0.1.1-owner.6`는 [PR #53](https://github.com/hyun06000/gil/pull/53)의 공통 작업 지침을 포함한다.
Mac Plugin preview.7과 같은 소스 `64de8e5c1207dda329600ef12a86f39cf7781ee3`에서 빌드했다.
Monitor 열기, 승인 후 Interview 닫기, 작은 Experiment, 조사 전에 Verify 열기, 실제 증거 파일과
해석·배움을 남기는 규칙을 MCP와 Plugin이 같은 원본에서 전달한다. 기존 기록은 변경·이관하지 않는다.
Verify마다 실제 파일을 저장하고 `evidence_files`로 지정해야 한다. 지정한 파일 중 최소 하나가
새로 생성되거나 바뀌어야 하며, 채팅만으로 종료하거나 변하지 않은 이전 증거만 제출하면 거절한다.
증거 Snapshot에서 해석과 배움으로 이어지는 참조를 기록한다.

[빌드 및 설치 CI](https://github.com/hyun06000/gil/actions/runs/40568942122)에서 설치·재설치·기존 MCP
연결을 유지한 버전 전환·다음 연결의 새 Core 선택을 확인했다. Luna 합성 실제 작업은 런타임/진입
네 조합에서 모두 열 개 검증을 통과하며 각각 실험 두 개·파일 두 개를 남겼다. 별도 다음 행동 시험은
61/64 통과이며, 채팅만 있는 Verify에서 세 번 잘못된 행동 라벨을 선택한 한계가 남는다.
이 수치는 이전 owner.6에 대한 기록이며 이번 owner.15의 모델 시험 결과가 아니다.
업데이트 후 Companion 표시가 `0.1.1-owner.15`인지 확인하고 새 GIL 연결·새 대화에서 Luna로 시험한다.
기존 연결을 새 Core로 강제 교체하지 않는다. macOS Plugin은 이 Windows 업데이트로 교체되지 않는다.
Codex와 Claude Desktop이 모두 같은 고정 launcher를 등록했다면 업데이트 뒤 재등록할 필요가 없다.
각 Host의 새 연결부터 같은 선택 버전을 사용한다. Companion 머리줄의 `Core <버전>`은 다음 새 연결이 받을
Core다.

기존 실패 프로젝트는 [공통 복구 절차](../INSTALL-GUIDE.md#실패-후-revisit가-막힌-기존-프로젝트-이어가기)를 따른다.
Windows에서도 같은 Monitor 버튼을 쓰며 작업 폴더와 기록 저장소가 같은 볼륨이어야 한다.

## 연결 해제와 제거

제거 순서는 다음과 같다.

1. Companion의 **Codex 연결 해제**와 **Claude Desktop 연결 해제**를 누른다(쓰던 Host만). Claude Desktop은
   완전히 종료한 상태에서 누른다.
2. 열린 GIL 대화를 닫는다.
3. Windows의 앱 제거에서 **GIL Companion (Unsigned Test)**를 제거한다.

연결 해제는 두 Host 모두 command가 설치된 고정 launcher이고 args가 정확히 `mcp --serve`인 소유 GIL
등록만 제거한다. 이름이 같은 다른 등록은 지우지 않는다. Claude Desktop 설정의 다른 MCP 서버 항목도
그대로 둔다. 현재 uninstaller는 실행 중인 연결과 기록을 보호하기
위해 `%LOCALAPPDATA%\GIL\runtime`과 모든 Project의 `.gil`을 보존한다. 완전 runtime 삭제는 아직
일반 사용자 기능으로 제공하지 않는다.

## 확인한 범위와 남은 한계

이번 owner.15의 Windows CI에서 확인한 범위(사용자 PC 필드테스트와 별개):

- current-user NSIS 설치와 같은 버전 복구 설치
- 이전 runtime 보존, 버전별 Core와 고정 launcher 선택
- Companion 창 표시
- 격리된 공식 Codex CLI 등록·해제와 launcher MCP
- 실행 중 연결을 유지한 side-by-side 업데이트와 새 연결의 새 Core 선택
- 연결 해제·Companion 제거 후 runtime/Project 보존
- 패키지에 포함된 실제 Core의 승인 계획 wire 시나리오

이전 owner.3의 실기기 설치·연결·파일 작업·업데이트 경험은 과거 기록이다. 이번 owner.15의
사용자 버튼 updater, 실제 owner.14→owner.15 전환, Codex/Claude 화면·fullscreen·모델 작업은 사용자 확인 대기다.
CI의 버전 전환은 합성 이전 버전과 같은 후보 Core bytes를 사용하므로 실제 이전 판 인수로 주장하지 않는다.

남은 한계:

- Authenticode code signing과 publisher 표시
- SmartScreen 평판
- 일반 외부 사용자 지원과 clean-PC 반복 인수
- Windows ARM64 제품 installer
- 완전 runtime 정리 버튼

개발용 `windows-preview-*` tar archive를 일반 사용자 installer로 사용하지 않는다. CI 통과만으로 실제
Codex Desktop 화면 성공을 주장하지 않는다. 구현·게이트 상세는 [Windows 배포 상태](README.md)와
[Companion 설치·업데이트 계약](../../spec/GIL_Companion_Install_Update_v0.1.md)을 따른다.
