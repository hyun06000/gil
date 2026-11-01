# Wave 1 공통 연결 경계

이 문서는 병렬 작업의 인터페이스 합의다. 기존 저장 형식/제품 정책을 변경하는 승인이나 완료 보고가 아니다.
공통 코드 기준은 `fcd6e5d90363ab880b27ccbd85e7c6936609ef93`이며 각 branch는 같은 준비 commit에서 시작한다.

## A → B: 정책 평가

A는 정책을 명시적 입력으로 구성하고 다음 두 사실을 제공한다.

```rust
// API 이름은 A가 이대로 제공한다. 구체적인 생성자/에러 타입/내부 표현은 A 소유.
fn includes_file(&self, relative_path: &str) -> Result<bool, String>;
fn policy_id(&self) -> String;
```

- true: 추적 후보, false: 정책 제외, Err: 해석/경로 거절. 경로는 `/`를 쓰는 프로젝트 상대 파일 경로다.
- 이 값은 symlink/reparse/TOCTOU 검증이나 파일 조작 권한을 대신하지 않는다.
- 정책 ID는 규칙 원문·상대 위치·평가 순서·버전에 묶인다. OS 경로 검증 결과와 혼동하지 않는다.
- B는 A 타입을 직접 import하지 않고 `impl Fn(&str) -> Result<bool, String>`을 받는다.
  자체 unit test에서는 명시적인 합성 callback을 쓴다. 통합 담당자가 `|path| policy.includes_file(path)`로 연결한다.
- B는 before/target의 정책 ID를 별도로 받는다. 미상/불일치하면 복원 준비 완료로 판정하지 않는다.
  기존 Manifest에 과거 정책 ID를 임의로 만들어 붙이지 않는다.
- A/B는 이 단계에서 Session의 기본 capture, 저장 schema, restore apply에 자동 연결하지 않는다.
  각자 실제 합성 입력에서 동작하는 읽기 API를 완성하고, 제품 연결은 통합 단계의 명시적 작업이다.

## C / D / E: 기존 M6-D 계약 고정

`spec/GIL_Approved_Plan_v0.1.md`, 실제 `tools/list`, `src/mcp/plans.rs`가 기준이다.

- 모델: `gil_plan_propose`, `gil_plan_read`, `gil_plan_request_interview`.
- App: `gil_plan_review`, `gil_plan_decide`. 임시 승인 token은 App `_meta`에만 있다.
- 조회 결과: proposal, active_revision, current_item, pending, executed_cycles,
  reinterview_requests, close_report, document. 실제 코드의 optional/null 의미를 읽는다.
- C는 UI, D는 안내 문구, E는 외부 시험을 담당한다. 서로의 파일이나 wire/schema/domain을 변경하지 않는다.
- legacy 프로젝트와 default 빌드는 approved-plan을 자동 지원하지 않는다. feature와 연결 도구로 판단한다.
- 사람 승인, 합성 App 입력, 전송 성공, 실제 화면 표시, 실제 모델 재개는 서로 다른 관측이다.
- 새 서버 필드가 필요하면 결과 문서에 제안한다. mock에만 존재하는 필드를 제품 기능처럼 사용하지 않는다.

## 통합 담당자 단독 소유

- Project/Chain/Session/저장 transaction, 전이 grammar, 새 feature/public export, 제품 활성화.
- `.github/**`, notice inventory 및 세 개 notice-policy, 설치/업데이트/게시 설정, 로드맵.
- A의 의존성 변경과 C의 생성 HTML을 통합한 뒤 고지 inventory/hash를 한 번 재생성하고 검증한다.
- M3의 정책 provenance 저장 및 안전 restore/revisit 연결, M6-E 설계 선택은 자동 병합 대상이 아니다.

공유 파일 수정이 필요하면 결과 문서에 함수 서명·호출 위치·예상 diff·실패 시험을 남긴다.
다른 세션 소유 파일을 고쳐서 일단 통과시키거나 실패 시험을 완화하지 않는다.
