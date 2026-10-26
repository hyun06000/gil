# Core + Companion 검수용 portable 묶음 — 미게시

정식 설치 프로그램이 아닙니다. 기존 GIL/Codex Plugin 등록·cache·설정·Project를 교체하지 않습니다.
업데이트 버튼/worker, Codex Desktop 내부 연결, 실제 설치/업데이트 성공도 이번 검수 대상이 아닙니다.
묶음의 실제 Core를 직접 MCP로 실행해 새 Project에서 Companion 자동 시작 → 트레이 → 마지막 연결
종료를 확인합니다. Core는 runtime-testbed feature가 켜진 검수판이고 Companion은 별도 앱 ID
`dev.ariadne.gil.companion.review`입니다. 기존 앱과의 single-instance/설정/IPC 충돌을 피합니다.

## 실행 전

- Windows x64 또는 ARM64 **OS CPU**와 일치하는 묶음을 선택합니다. OS/보안 정책이 막으면 멈춥니다.
- 아직 게시 URL·최종 archive hash는 없습니다. 제작자가 CI artifact의 source commit, 크기,
  전체 SHA-256, 검증 결과를 전달하기 전에는 실행하지 않습니다. 문서의 빈 값을 추측하지 않습니다.
- 서명되지 않은 시험판입니다. hash는 바이트 일치이지 publisher 인증이 아닙니다. SmartScreen,
  Defender, AppLocker 또는 회사 정책을 끄거나 우회하지 않습니다. 관리자 실행도 하지 않습니다.
- WebView2가 없거나 GUI 실행이 거절되면 BLOCKED로 보고합니다. 자동 설치/다운로드는 하지 않습니다.
- 기존 GIL 프로젝트가 **아닌**, 사용자가 승인한 새 로컬 비동기화 폴더에 압축을 풉니다.
  OneDrive·네트워크·reparse 경로에서는 Core가 거절할 수 있습니다. 권한을 바꾸거나 자동으로 다른
  위치를 찾아 재시도하지 않습니다. 경로 선택이 필요하면 사용자에게 확인합니다.
- Agent는 다운로드 시 제품의 공식 네트워크 승인만 사용합니다. 해시를 검증한 뒤 멈추고 사용자의
  실행 동의를 받습니다. 기존 GIL 제거/설치, `config.toml` 편집, 프로세스 종료는 금지합니다.

## 사용자 조작 (한 번 실행)

1. 압축을 푼 폴더의 **START-REVIEW.cmd**를 더블클릭합니다. 평문 파일이며 실행 정책 우회나
   인코딩된 스크립트가 없습니다. 차단되면 그대로 중단합니다. Node/Rust/npm은 필요 없습니다.
2. 콘솔의 변경 범위를 읽고 `y`를 입력합니다. 묶음 안 `review-<고유값>`에만 시험 Project·로그·
   설정을 만들며 기존 폴더를 재사용하지 않습니다. Core가 성공한 Project 응답 뒤에만 Companion을 시작합니다.
3. 트레이의 숨겨진 아이콘도 펼쳐 **GIL Companion 검수** tooltip을 확인합니다. 오른쪽 클릭으로 메뉴만 확인합니다.
   왼쪽 클릭이나 'GIL Companion 열기'는 별도 Monitor 창을 열므로 이번에는 누르지 않습니다.
   Project 선택/로그인 시작/기존 Project 열기는 하지 않습니다. 별도 Monitor 창은 자동으로 열리지 않습니다.
4. 콘솔에 `y`를 입력하면 시험 Core의 stdin을 닫습니다. 아이콘이 사라졌는지 확인해 답합니다.
   시험이 열린 **정확한 Companion 프로세스 핸들**의 종료를 기다리며 PID/이름으로 종료하지 않습니다.
   트레이 확인은 사용자 관측, 프로세스 종료는 OS 관측으로 따로 보고합니다.
5. 결과와 `review-…/report.json`, `core-stderr.log`를 보존합니다. 실패해도 재실행·ACL 수정·
   프로세스 종료·기존 GIL 조작을 하지 않습니다. 종료 미확인 프로세스가 있으면 그 사실을 보고합니다.

PASS는 실제 Core 응답 성공 + 트레이 표시 사용자 확인 + Core 정상 종료 + Companion 핸들 종료 확인
+ 트레이 소멸 사용자 확인을 모두 만족해야 합니다. 프로세스 시작이나 도구 응답만으로 PASS가 아닙니다.
초기 검증/실행 불가는 BLOCKED(2), 흐름 실패는 FAIL(1), 위 조건 충족은 PASS(0)입니다.
실행 중인 Core에서 stderr에 IPC 오류가 나도 Project 요청 자체가 성공할 수 있으므로 readiness와
별도로 대조합니다. 로그에는 개인 경로/SID가 있을 수 있어 공개 게시하지 않습니다.

## 생성 범위와 한계

검수 도구는 기존 App Server나 Codex 프로세스를 사용하지 않습니다. 실제 Codex 제한 토큰에서의
GUI 실행 가능성은 별도 인수입니다. 창을 수동으로 열면 WebView2가 검수 앱 ID용 사용자 데이터를
만들 수 있습니다(기존 GIL ID와 별도). 이번 안내에서는 창을 열지 않고 tray 메뉴만 확인합니다.
로그인 항목 등록·레지스트리 설치·Plugin 등록·자동 정리·강제 종료는 없습니다.
해당 폴더 삭제는 종료 확인 뒤 사용자 선택 사항이며 도구는 삭제하지 않습니다.

## 제작자 게이트

Windows 두 CPU 네이티브 빌드, 패키징/receipt/hash/고지 검증, CI 결과와 source SHA 연결을 확인한
뒤 별도 게시 승인을 받습니다. `review-bundle.json`의 `publishable:false`는 이 준비 단계를 의미합니다.
고지에는 Core의 기존 검증된 notices와 Companion까지 포함한 Cargo 원문을 별도로 보존합니다.
새 Companion 의존성의 법적 고지 검토와 실제 Windows tray 인수는 미완이며 자동 통과로 간주하지 않습니다.
