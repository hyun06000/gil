# M4-R — 승인된 실패 Cycle 복구

상태: 2026-11-02 양 OS CI 통과, Mac preview.9 / Windows owner.8 시험판 게시.
실제 Monitor 클릭·Luna 인수는 별도다. [게시 장부](../distribution/codex/PREVIEW-9-PUBLICATION-20261102.md)를 따른다.
사용자 결정: Monitor에서 변경 목록을 확인하고 **보존 후 되돌아가기** 버튼으로 승인한다.

## 문제와 제한된 범위

preview.8은 `failure → revisit`을 정상 저장하고 다음 행동으로 안내하지만 `product-safety`가
그 실행을 일괄 차단했다. 승인 계획의 branch 금지도 있어 실패 뒤 더 이상 진행하지 못했다.

이 계약은 **macOS와 Windows의 닫힌 실패 Cycle**에 한정된 기존 기록 복구다. 일반 `restore`, Step-level
revisit, 전체 추적 정책 활성화 또는 저장 형식 migration을 허용하지 않는다.
Artifact Model §3.6a의 일반 제품 복원 게이트를 완료했다고 주장하지 않는다. Windows는 별도
핸들 기반 transaction을 사용하며 기존 경로 문자열 기반 writer로 돌아가지 않는다.

## 기존 Snapshot의 정체성

- 기존 whole-tree Snapshot을 필터링하거나 새 정책 ID를 붙이지 않는다. 실패·대상 Manifest를
  그대로 읽고 최종 작업 세계도 대상 Manifest 전체와 같아야 한다.
- 기본 보호 대상은 모든 깊이의 `.git`, `.env`, `.env.*`, `*.pem`, `*.key`, `.ssh/`, `.aws/`,
  `node_modules/`, `target/`, `dist/`, `build/`, `.cache/`, `.venv/`, `__pycache__/`이다.
  before와 target 각각에 저장된 `.gitignore` 원문을 기존 부분집합 엔진으로 해석한다.
- 어느 한쪽 정책이 제외하는 경로의 생성·변경·제거는 **전체 복구 거절**이다. 변경 없는 보호
  파일은 그대로 둔다. 예전 Snapshot의 Git/secret을 조용히 삭제하거나 과거 값으로 되돌리지 않는다.
- 현재 세계가 실패 Snapshot과 다르면 승인 전 무변경 거절한다. 경로 충돌·링크·비정규 항목·
  경로 길이·내용 객체 손상도 사전 거절한다. 승인 실행 직전에 다시 관측하고, restore의 실제
  계획 생성에도 승인한 전체 before Manifest를 전달해 새 파일로 쓰기 범위를 넓히지 않는다.
- policy ID가 없는 preview를 일반 apply 후보로 승격하지 않는다. 이 경로는 별도의
  `legacy-whole-tree-protected-difference-v1` 계약이며, 바뀌는 보호 경로가 없다는 제한된 증명을 쓴다.

## 사용자 승인과 보존

`gil_revisit_review`/`gil_revisit_apply`는 App 전용이다. review는 잠근 기록과 파일을 읽기만
하며 생성·복구·reap을 하지 않는다. 전체 변경 목록(create/replace/preserve), 실패 Cycle,
대상 Cycle, before/target Snapshot, 보존 위치를 표시한다. 표시 목록과 다음 논리 상태를
묶는 digest 및 scope별 256-bit 임시 토큰을 검증한다. 토큰은 `_meta`에만 보내며 기록이나
모델 content에 넣지 않는다. 읽기·자동 갱신·채팅 동의는 승인이 아니다.

승인 후 실제 기록 저장소의 `.gil/revisit-preserved/<pending-state-sha256>/`에 before 세계의 원본 파일을
`file-N`으로 저장하고 `files.json`에 경로·보관 이름·SHA-256을 적는다. `approval.json`은 승인한
계획을 남기며 토큰은 담지 않는다. 파일은 private mode, no-follow 디렉터리 핸들로 원자적으로
게시하고 fsync한다. 보존본은 transient rollback backup과 달리 자동 삭제하지 않는다.
미완의 임시 보존 파일은 실행 권한이나 완료 증거가 아니다.

실패 Report·Will·Journey·Snapshot 객체·승인 이력은 변경하거나 지우지 않는다. 작업 폴더에서
빠지는 파일은 보존본과 원래 Snapshot 양쪽에 남는다. 새로운 Snapshot을 만들거나 실패를
success로 바꾸지 않는다. 보존 파일을 작업 폴더에 다시 넣는 것은 이 명령의 일부가 아니다.

## 실행·중단 복구

1. 변경 목록 재검증 → 원본·승인 receipt의 영속 보존.
2. 기존 Cycle 전이로 대상 ancestor와 pending source를 저장.
3. 승인된 before에 한정한 각 OS의 핸들 기반 transaction으로 대상 세계를 복원.
4. 완료 후 별도 `gil_open`으로 새 Cycle을 연다.

② 뒤 중단되면 기존 transaction recovery가 중간 파일 변경을 rollback한다. `gil_revisit`은
현재 pending 상태와 정확히 일치하는 보존 승인 receipt가 있을 때만 같은 복원을 재개한다.
이 호출은 새 승인을 만들 수 없다. ③이 이미 끝났다면 같은 요청은 no-op으로 수렴한다.
승인 뒤 외부 편집이 생기면 덮어쓰지 않고 거절한다. 프로젝트 잠금은 외부 편집기를 잠그지
못하므로 복원 중 동시 편집까지 전역 원자적으로 보장한다고 주장하지 않는다.

## 복구 후 채팅 이어가기 (후속 수정·미배포)

사용자가 Monitor 버튼을 누른 뒤 Core가 `completed`를 반환하면 App은 Host의 `sendMessage`로
같은 프로젝트의 `gil_context`를 읽고 이어가라는 요청을 한 번 보낸다. 개인 경로·변경 파일명·
승인 token은 메시지에 넣지 않는다. 채팅 메시지는 저장된 승인 receipt를 대체하지 않는다.
자동 새로고침·화면 재열기·불확실한 적용 응답에서는 메시지를 자동 전송하지 않는다.
완료 상태를 읽기로 확인한 뒤 사용자가 **채팅에 계속 요청 보내기**를 누를 수 있다.
전달 실패는 복구 실패가 아니며 재시도는 채팅 전달만 한다. Host의 메시지 수락은 모델 실행이나
실제 화면의 증거가 아니다. 전달 미지원 시 문맥 재조회 요청을 안내한다.
Agent는 승인 대기 뒤 사용자가 “계속”이라고 말하면 이전 안내를 반복하기 전에 `gil_context`를
한 번 읽는다. 여전히 승인이 필요하면 기다리고, 완료됐다면 현재 기록에 맞춰 진행한다.

2026-11-02 로컬 검증: Core 1,519 통과·2 기존 ignored, 실제 MCP 승인/복구 7 통과,
JS 153 통과·4 skipped, 배포/고지 99 통과·1 skipped, Core all-target 빌드 통과.
합성 App 입력 시험이며 실제 Host 메시지 전달·모델 재개·Windows 네이티브 검수는 별도다.

## 승인 계획의 재시도 바인딩

저장 schema를 바꾸지 않는다. executed_cycles는 모든 실행 시도를 보존한다. 최신 실행이
failure로 닫힌 항목은 다시 pending에 나타나며, 성공 또는 열려 있는 최신 실행은 중복하지 않는다.
같은 항목을 재바인딩하려면 직전 실패를 가리키는 `revisit_from`이 새 Cycle 또는 그 조상에 있어야
한다. 실패 Cycle 아래에 자식을 만들 수 없고 이전 승인 판으로 되돌아갈 수도 없다.
새 승인 판의 항목 순서와 기존 항목 정의 보존 검사는 그대로 적용한다.

재인터뷰 요청이 남아 있으면 Experiment를 바로 열지 못한다. revisit 뒤 Interview를 열어
범위를 다시 승인하고 재시도할 수 있다. 재시도의 `plan_document`는 새 바인딩의 실제 승인 판을
사용하며 `evidence_files`, Analysis, Outcome의 연결은 기존 계약대로 새로 남긴다.
`retry_pending`은 read 응답의 항목 ID 목록이며 저장 필드가 아니다.

## 인수 범위

- 실제 제품 기능 조합의 stdio: 이미 닫힌 실패, 승인 없는 거절, 위조·오래된 토큰 거절,
  변경된 파일·보호 경로·손상 객체 거절, 두 번 연속 재시도 후 성공, 원본 증거·Snapshot 보존.
- 상태 저장 뒤 실패 및 실제 복원 중 프로세스 종료 → 같은 승인 복구 재개.
- UI: 목록 표시, 명시적 클릭만 실행, 응답 유실 후 읽기로 확인, 중복 실행 억제.
- 실제 사용자의 Monitor 클릭·Luna 실사용·배포 설치본은 별도 인수이며 합성 입력으로 대체하지 않는다.

## 병렬 작업 경계

호스트 지원 작업의 clients/설치기/배포 설정/호스트 판별은 수정하지 않는다. 공용 MCP 등록과
Monitor bootstrap에는 recovery 모듈을 연결하는 최소 변경이 있다. 호스트 PR과 합칠 때 이
등록·공용 workflow 생성 영역·생성 HTML만 함께 재검증한다. 생성 HTML은 소스를 통합한 뒤 재생성한다.

## Windows 구현 경계

- 사용자의 단계·버튼·증거 보존·재시도 규칙은 Mac과 같다. 작업 폴더 밖에 기록을 두는 설치도
  실제 보존 위치를 Monitor에 표시한다. 일반 restore/Step revisit은 양쪽 모두 계속 차단한다.
- 디렉터리의 모든 조상을 pin하고 한 구성요소씩 no-reparse 핸들로 연다. junction/reparse,
  hard link, drive/prefix/ADS/reserved name, 대소문자 충돌은 거절한다.
- PLAN과 원본을 durable하게 기록한 뒤 active transaction으로 전환한다. 신규 파일은
  transaction의 work 디렉터리에서 완성·검증한 뒤 핸들 기반 rename으로 게시한다. 중단 시
  미완성 bytes가 사용자 폴더에 남지 않으며 backup 검증 후 rollback한다.
- working tree와 기록 저장소가 같은 볼륨이어야 원자적 게시가 가능하다. 서로 다른 볼륨이면
  미리보기 단계에서 거절하며 승인·논리 전이·파일 쓰기를 시작하지 않는다.
- cleanup은 알려진 전체 모양을 검사한 뒤에만 핸들로 삭제한다. 모르는 항목·링크·손상된
  backup·복원 중 외부 편집이 있으면 보존하고 거절한다. 전원 장애에 대한 모든 장치의
  내구성이나 실제 Host 화면 인수까지 보장하는 것은 아니다.
