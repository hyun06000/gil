# Windows 배포 상태

> 상태: **친구·테스터용 unsigned x64 preview는 설치·연결·업데이트 인수 완료. 서명된 일반 공개판은 NO-GO.**

Windows x64와 ARM64용 native Core 후보는 CI에서 빌드·실행된다. 제품 경로는 우선 x64로 좁혔다.
사용자용 NSIS, 사용자가 누르는 서명 검증 updater, 고정 런처, 버전별 Core 설치와 Codex 연결 버튼이
연결됐다. 2026-10-27 Windows x64 실기기에서 unsigned `0.1.1-owner.3` 설치, Companion 실행,
Codex 연결·이전 GIL 연결 교체, 파일 생성과 사용자 버튼 업데이트를 확인했다. 친구용 설치 절차와
정확한 자산은 [INSTALL](INSTALL.md)을 따른다. Authenticode publisher, SmartScreen 평판,
clean standard-user PC 전체 인수와 ARM64는 완료하지 않았으므로 일반 지원 완료로 표시하지 않는다.
`candidate.mjs`가 만드는 `tar.gz`는 검토용
Plugin 묶음이며 receipt도 `publishable: false`다.

따라서 다음 표현은 현재 사실이 아니다.

- “서명된 Windows 일반 공개판을 지원한다.”
- “archive를 풀고 Plugin을 등록하면 설치가 끝난다.”
- “업데이트가 다음 연결부터 자동으로 적용된다.”
- “CI에서 통과했으므로 모든 일반 사용자 PC에서도 설치·화면·업데이트가 동작한다.”

## 현재 코드의 두 실행 경로

기존 `candidate.mjs` 검토 묶음의 Host manifest는 여전히 Plugin cache 안의 `gil.exe`를 직접
실행한다. 이 묶음은 제품 설치본이 아니며 Windows 파일 수명 충돌을 해결하지 않는다.

새 NSIS 제품 경로는 `%LOCALAPPDATA%\GIL\runtime\gil-launcher.exe`를 고정 진입점으로 두고,
`versions\<version>\gil.exe`의 SHA-256을 `current.json`과 대조해 실행한다. 설치 훅은 실행 중인
Core를 덮어쓰지 않고 새 버전 디렉터리를 만든 뒤 작은 선택 기록만 바꾼다. 새 기록이나 Core가
손상되면 `current.previous` 한 판까지만 검증해 사용한다. 런처는 Job Object로 자식 Core를
묶는다. Companion의 **Codex 연결** 버튼은 사용자가 확인한 뒤 공식 `codex mcp add` 명령으로
이 고정 진입점을 등록한다. 같은 installer를 다시 실행하면 이미 설치된 불변 Core bytes는
덮어쓰지 않고 Companion과 선택 기록을 복구 설치한다. 예전 `gil-companion-prototype` Plugin이나
다른 `gil` 등록이 함께 있으면 연결 완료로 표시하지 않으며, 사용자가 Companion의
**이전 GIL 연결 교체**를 확인한 경우에만 공식 Codex CLI로 그 GIL 항목을 제거하고 고정 런처를
등록한다. 다른 Plugin과 Project 파일은 변경하지 않는다.

코드와 CI만으로 출시를 주장하지 않는다. 위 사용자 인수는 unsigned friends-only x64 후보의
설치·연결·작업·업데이트 증거다. 서명된 NSIS, clean standard-user PC, SmartScreen publisher,
uninstall과 런처 자체를 바꾸는 업데이트 경로는 별도 후보에서 확인해야 한다.

`.github/workflows/windows-product.yml`은 이 간극을 좁히는 수동 제품 후보 작업이다. PR에서는
계약만 검사하고, 승인된 `workflow_dispatch`에서만 updater 개인키와 Authenticode PFX를 읽는다.
서명된 NSIS를 실제 설치한 뒤 receipt/digest, MCP 초기화, 실행 중 Core 쓰기 거절과 격리된
Codex CLI 등록을 확인한 경우에만 3일짜리 후보 artifact를 남긴다. 이 작업은 release를 만들거나
실제 Codex Desktop 화면·SmartScreen 평판·제거를 성공으로 선언하지 않는다.

수동 후보 작업은 GitHub Environment `windows-product-signing`의 승인과 다음 secrets를 요구한다.

- `GIL_UPDATER_PUBLIC_KEY`: 앱에 포함할 Tauri updater 공개키
- `GIL_UPDATER_PRIVATE_KEY`: 공개 저장소에 넣지 않는 대응 개인키
- `GIL_UPDATER_PRIVATE_KEY_PASSWORD`: 개인키 암호
- `GIL_UPDATER_ENDPOINT`: credentials가 없는 공식 HTTPS update JSON 주소
- `GIL_WINDOWS_CERTIFICATE_PFX_BASE64`: Authenticode code-signing PFX의 base64
- `GIL_WINDOWS_CERTIFICATE_PASSWORD`: PFX 암호
- `GIL_WINDOWS_TIMESTAMP_URL`: 인증기관이 안내한 RFC 3161 timestamp 주소

개인키/PFX를 파일로 commit하거나 workflow artifact에 넣지 않는다. Environment에는 필수 reviewer를
두고, `build_signed_candidate=true`인 수동 실행만 후보를 만든다. 성공 artifact도 release가 아니며
3일 뒤 만료된다.

## 연결 해제와 제거

Companion의 **Codex 연결 해제**는 `codex mcp list --json`으로 등록을 먼저 읽고, command가 설치된
고정 런처이며 args가 정확히 `mcp --serve`일 때만 공식 `codex mcp remove gil`을 실행한다. 이름이
같은 다른 등록은 삭제하지 않는다. 제거 순서는 **Codex 연결 해제 → 열려 있는 GIL 대화 닫기 →
Windows 앱 제거**다.

첫 제품판의 NSIS uninstaller는 Companion과 그 바로가기/등록만 제거한다. `%LOCALAPPDATA%\GIL\runtime`
과 모든 Project의 `.gil`은 보존한다. 이미 열린 대화가 Core를 사용 중일 수 있는데 재귀 삭제를
시도하면 반쯤 제거된 runtime을 만들 수 있기 때문이다. 완전 runtime 정리는 별도의 소유권·실행 중
프로세스·등록 부재 검증과 명시적 사용자 버튼이 구현되기 전까지 지원하지 않는다.

## 채택할 사용자 경로

첫 Windows 배포는 다음 한 경로만 제공한다.

1. 서명된 사용자 단위 installer가 Companion, 고정 실행 진입점과 초기 Core를
   `%LOCALAPPDATA%\GIL` 아래의 GIL 소유 영역에 설치한다.
2. Host 등록은 버전별 Core나 Host cache의 실행 파일이 아니라 고정 진입점만 가리킨다.
3. 업데이트 확인은 알림만 표시한다. 다운로드·설치는 자동으로 시작하지 않는다.
4. 사용자가 Companion에서 버전과 변경 사항을 보고 **업데이트** 버튼을 눌러야 시작한다.
5. 새 Core는 새 버전 디렉터리에 준비하고 검증한다. 실행 중인 Core는 덮어쓰거나 종료하지 않는다.
6. 검증된 선택 기록을 원자적으로 바꾼 뒤 새 MCP 연결부터 새 Core를 사용한다.
7. 새 연결의 실제 Core identity가 일치할 때만 적용 완료로 표시한다.
8. Companion·고정 진입점·Skill·Host 등록이 바뀌는 판은 Core-only 업데이트가 아니라 새 installer로
   처리한다.

“가장 최신 이름의 폴더”를 고르는 방식은 사용하지 않는다. 선택 기록은 정확한 release identity,
digest, architecture와 설치 cohort를 포함해야 한다. 실패 시 기존 선택과 Project를 보존하고,
자동 rollback·보안 설정 변경·Host cache 삭제·구버전 강제 종료를 하지 않는다.

## 출시를 막는 항목

- Windows x64 installer의 uninstall/repair 경로와 후속 ARM64 범위 결정
- Authenticode publisher와 설치·업데이트 서명 검증
- `%LOCALAPPDATA%\GIL`의 권한, 원자적 선택, 중단 복구와 정리 정책
- 고정 진입점·Job Object·이전 Core 폴백의 Windows 실기기 검증
- Companion의 Windows tray/UI와 서명 updater 실기기 검증
- `codex.exe`가 PATH에 없는 설치의 공식 연결 대안 및 clean standard-user PC 종단 시험
- 설치 중단, 디스크 고갈, 실행 중 업데이트, 재부팅, 제거/재설치 인수
- x64와 ARM64 각각의 실제 화면·재연결·Project 보존 인수

이 항목이 닫히기 전에는 사용자에게 PowerShell 스크립트, cache 삭제, `config.toml` 편집,
Defender/SmartScreen 우회, 관리자 실행을 설치 방법으로 안내하지 않는다.

## 개발자 검증 자료

- [`candidate.mjs`](candidate.mjs): 서명되지 않은 native Plugin 검토 묶음 생성
- [`spike.mjs`](spike.mjs): native MCP/Monitor smoke
- [`RELEASE-READINESS.md`](RELEASE-READINESS.md): 현재 출시 게이트
- [`../runtime-update/README.md`](../runtime-update/README.md): 제품과 분리된 update capability probe
- [`../../spec/GIL_Companion_Install_Update_v0.1.md`](../../spec/GIL_Companion_Install_Update_v0.1.md):
  사용자 주도 설치·업데이트 계약
- 날짜와 commit이 붙은 `PUBLICATION-*`, `CHECKPOINT-*`, `*-CHECK.md`: 당시 증거 장부.
  현재 사용자 설치 지침이 아니다.
