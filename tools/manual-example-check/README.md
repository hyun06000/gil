# tools/manual-example-check — Chain·승인 계획 예시의 실행 계약 검사

공통 workflow(`src/mcp/workflow.md`), 내장 설명서 Topic, 승인 계획 명세에 실린 **Chain·승인 계획 예시**가
현재 Core/MCP 계약으로 실제 실행 가능한지 검사하는 **개발용** 도구다. Node 표준 라이브러리만 쓰고,
[`tools/scenarios/lib`](../scenarios/README.md)의 stdio 클라이언트·격리 환경·기록 경로·가림 코드를 읽기 전용으로 재사용한다.

이 도구는 새 `gil help` 명령이나 전체 Manual Example 공개 체계(Roadmap M3.5-B **Example 실체 검증**)를 구현하지 않는다.
검사하는 것은 예시의 **구조적 실행 가능성**이다. 질문의 깊이, 제안의 명료함, 항목 크기, 모델 행동은 검사하지 않는다
(그것은 [`tools/chain-interview-eval`](../chain-interview-eval/README.md)과 실제 모델·사용자 평가의 몫이다).

이미 있는 시험을 다시 만들지 않는다. Topic 주소 존재는 `tests/manual.rs`·`tests/help_ref.rs`, 승인 상태 전이 전반은
`tests/approved_plan_wire_wave1.rs`·`tools/scenarios/wire-runner.mjs`가 맡는다. 이 도구는 **문서의 특정 구간 ↔ 그 구간이
말하는 입력·거절** 을 연결하고, 원문이 바뀌면 그 연결이 끊어졌다는 것을 알린다.

## 실행

```sh
# 문서만 읽는 검사: 매핑 지문·구간·미매핑 영역·문서 간 일치 (Core 불필요)
node tools/manual-example-check/check.mjs sources

# 실제 Core 실행: 이 branch를 approved-plan,visual-authoring,product-safety로 빌드한 직후 실행
node tools/manual-example-check/check.mjs run --build

# 이미 빌드한 실행 파일로 (PATH·설치본은 읽지 않는다)
node tools/manual-example-check/check.mjs run --gil <target>/debug/gil [--out <새 폴더>] [--case CX-01,CX-05] [--keep]

# 다른 checkout에서 수정 중인 안내 원문을 미리 확인 (그 원문으로 빌드한 Core를 함께 준다)
node tools/manual-example-check/check.mjs sources --docs-root <다른 worktree>

# 자체시험 (기본은 Core 없이; 실제 Core 시험은 환경 변수로 켠다)
node --test tools/manual-example-check/*.test.mjs
GIL_EXAMPLE_CHECK_GIL=<target>/debug/gil node --test tools/manual-example-check/*.test.mjs
```

**제품 프로필 전제:** `run`은 `approved-plan,visual-authoring,product-safety`로 빌드한 Core에서만 의미가 있다. 사례마다 먼저 `tools/list`에
`gil_plan_*` 도구가 있는지 확인하며, 기본 프로필 실행 파일이면 모든 사례를 `profile` 이유로 실패시킨다(성공으로 세지 않는다).
Verify는 typed close(`expected_owner`·`visuals`)로 닫으므로 `visual-authoring` 없는 승인 프로필 빌드도 지원하지 않는다.
일반 `cargo test`에는 포함되지 않는다. 승인 프로필은 승인 없는 합성 bootstrap을 거절하므로 기존 전체 시험에 `--all-features`를 붙이는 방식으로도 대신할 수 없다.

종료 코드: `0` 모든 실행형 예시 통과·매핑 최신 · `1` 실패·원문 변경·누락·미매핑·미실행 사례가 있음 · `2` 사용법·도구 오류.
`--case`로 일부만 실행하면 나머지 예시는 `not_run`이고 종료 코드는 1이다(일부 실행을 성공으로 세지 않는다).

결과는 `--out`(없으면 임시 폴더)에 `report.json`, `report.md`, `events.jsonl`로 쓴다. 출력 폴더는 저장소 밖의
새 폴더나 빈 폴더여야 하며 기존 파일을 덮어쓰지 않는다. 절대 경로와 App capability는 가린다.

## 파일

| 파일 | 역할 |
|---|---|
| `examples.json` | 검토된 매핑: 원문 경로·구간 locator·SHA-256 지문·예시 종류·payload 파서·연결 사례·주장 |
| `fixtures/synthetic-inputs.json` | 사용자 답과 부분 예시의 빈 칸을 대신하는 합성 입력. 문자열 데이터로만 보낸다 |
| `lib/sources.mjs` | 정확한 문자열 locator(정규식 아님)로 구간을 찾고 지문·줄 범위를 계산, 예시 모양 영역 목록 |
| `lib/extract.mjs` | 구간 → payload(JSON 템플릿, 항목 줄, 필드 줄, 코드 span, ID 목록). 선언된 placeholder만 치환 |
| `lib/registry.mjs` | 매핑 검증과 원문 대조: `source_changed`/`missing`/`ambiguous`/`payload_invalid`/미매핑/문서 간 불일치 |
| `lib/rig.mjs` | 사례마다 새 임시 프로젝트 하나와 `gil mcp --serve` 하나. 호출 허용 목록, 기록·현재 위치 불변 확인 |
| `lib/cases.mjs` | 검토된 사례 CX-00–07. 명시된 도구 호출만 수행한다 |
| `lib/report.mjs` | 예시별 상태·판정·Markdown 보고서 |
| `check.mjs` · `check.test.mjs` | CLI와 자체시험 |

## 예시 종류

| 종류 | 뜻 | `pass` 가능 |
|---|---|---|
| `executable_template` | 완전한 도구 입력. 선언된 placeholder만 임시 프로젝트가 발급한 값으로 치환한다 | 예 |
| `executable_fragment` | Report·계약·참조 줄을 그대로 검토된 호출에 넣는다 | 예 |
| `illustrative_partial` | 설명용 부분 예시. 선언된 필드만 추출하고 나머지는 합성 입력으로 채워 표시한다 | 예 |
| `rule_statement` | 규범 문장. 사례가 그 문장이 말하는 허용·거절 입력을 만든다 | 예 |
| `semantic_only` | Core가 판정하지 않는 품질 안내. 관측만 하며 **절대 pass로 세지 않는다** | 아니오 |
| `not_executed` | 셸·CLI·범위 밖 블록. 문서의 코드는 실행하지 않는다 | 아니오 |

남은 `<…>` 꼴 placeholder가 있으면 실행하지 않고 실패한다. 부분 예시를 완전한 명령처럼 실행해 잘못된 실패를 만들지 않는다.

## 원문 ↔ 사례 대응

| 예시 ID | 원문 | 종류 | 사례 |
|---|---|---|---|
| WF-PROPOSE-WRAPPER | workflow.md rule 7 ```json | executable_template | CX-00·01·02·03·05 |
| WF-PROPOSE-SHAPE · WF-PROPOSE-FIELDS | workflow.md rule 7 입력 모양 문장 | rule_statement | CX-02 |
| WF-BASIS-REPLACE · WF-BASIS-RULE | workflow.md rule 7 basis_refs 문장 | rule_statement | CX-01·02·05 |
| WF-TABLE-SYNTH-WAIT · WF-TABLE-SYNTH-APPROVED | workflow.md rule 2 표 | rule_statement | CX-03 |
| WF-APPROVE-CLOSE · WF-CLOSE-REPORT-OWNER | workflow.md rule 7 | rule_statement | CX-03·04 |
| WF-RECOVER-PLAN-STATE | workflow.md rule 8 표 | rule_statement | CX-03·06 |
| WF-ONE-ITEM-BINDING · WF-DEFINE-SMALLEST | workflow.md rule 7·3 | rule_statement | CX-04·05 |
| WF-DOCUMENT-VERIFY | workflow.md rule 7 Accepted/Refused | executable_fragment | CX-04 |
| WF-EXTEND | workflow.md rule 7 | rule_statement | CX-05 |
| WF-REINTERVIEW · WF-TABLE-REINTERVIEW | workflow.md rule 7·8 | rule_statement | CX-06 |
| SPEC-REORDER | Approved Plan 명세 `[A,B,C]`→`[A,D,B,C]` | illustrative_partial | CX-05 |
| SPEC-EXTEND-REFUSALS · SPEC-BASIS-JSON | Approved Plan 명세 | rule_statement · executable_fragment | CX-05 |
| MAN-CLARIFY-SHAPE · MAN-CLARIFY-GOOD | `cycle/interview/clarify` | rule_statement · illustrative_partial | CX-04·05·06 |
| MAN-DEFINE-GOOD | `step/define/smallest` [올바른 예] | executable_fragment | CX-04·06 |
| MAN-OPEN-GOOD | `action/open-contract` [올바른 예] | executable_fragment | CX-04 |
| MAN-CLARIFY-FAIL · MAN-OPEN-FAIL | 각 Topic [흔한 실패] | semantic_only | CX-07 (관측) |
| MAN-DEFINE-FAIL | `step/define/smallest` [흔한 실패] | semantic_only | — |
| MAN-OPEN-CLI · WF-VISUALS-TEXT · SPEC-DEV-SH | CLI heredoc · 시각 Report · 개발 셸 | not_executed | — |

| 사례 | 확인 |
|---|---|
| CX-00 | 실행한 Core의 `initialize.instructions`·`gil_help(topic)`가 검토한 구간을 그대로 담는다(오래된 실행 파일 감지) |
| CX-01 | Question·Interpretation 뒤 발급된 참조로 문서 wrapper가 그대로 받아들여진다 |
| CX-02 | schema(-32602)·Core 거절(빈 scope 칸, 0/65 항목, checklist 순서, basis 쉼표·설명·Cycle·열린 Step·중복·다른 Cycle·빈 값, approved 직접 기입). 거절마다 기록 불변, 끝에 Context·계획 조회 불변 |
| CX-03 | 승인 전 Synthesis close·맨 `approved: yes` 거절(승인 상태 표지와 `gil_plan_read`), 합성 승인 뒤 변조 close_report 거절·정확한 close_report 성공, Outcome에서 close_report 재사용 거절 |
| CX-04 | 설명서 항목 A/B/C → Experiment가 A에 바인딩 → 설명서 계약으로 Define 열기·설명서 Define 닫기 → 문서의 `plan/A.md`·`evidence/A-result.md` 경로로 Verify(계획 파일만·다른 bytes는 거절) |
| CX-05 | `[A,B,C]` 승인·A 실행 → 다음 Interview에서 문서 literal 참조 거절, B 누락·새 항목만·B.question/done_when 변경 거절 → 복사+D 삽입 승인 → pending `[D,B,C]` |
| CX-06 | 열린 Define에서 재인터뷰 요청 보존·아무것도 닫지 않음, Interview·Experiment 열기 거절, 취소 도구 없음, Cycle을 사실대로 닫은 뒤 추가 Experiment는 승인 상태로 거절, Interview는 열림 |
| CX-07 | 관측 전용: `[흔한 실패]`의 계약(`next_action: gil close`)과 묶음 항목을 Core가 받아들이는지 기록 |

## 원문이 바뀌었을 때

다른 세션이 안내 원문을 고치는 중일 수 있다. 이 도구는 원문을 고치지 않고 다음처럼 알린다.

- `source_changed`: 구간의 지문이 다르다. 출력에 파일·현재 줄 범위·연결 사례·갱신할 `examples.json → <ID>.sha256`이 나온다.
  원문과 사례를 **다시 검토한 뒤에만** 지문을 고친다. 지문만 고쳐도 계약이 깨졌다면 사례가 실제 Core에서 실패한다
  (`check.test.mjs`의 실제 Core 시험이 이것을 보인다).
- `missing`/`ambiguous`: locator 기준 문자열이 없거나 둘 이상이다. `<ID>.source.locator`를 고친다.
- `payload_invalid`: 구간은 검토됐지만 파서가 선언된 필드를 읽지 못한다. `<ID>.payload`를 고친다.
- 미매핑 영역: 매핑된 구간 밖에 새 코드 블록이나 Manual 예시 절이 생겼다. 실행 사례를 더하거나 `semantic_only`/`not_executed`로 이유와 함께 등록한다.
- 문서 간 불일치: `consistency` 항목. 이미 확인한 불일치는 `acknowledged`에 현재 값을 적어 보고만 하고, 값이 바뀌면 다시 실패한다.

원문이 최신 매핑과 다르면 `run`은 어떤 사례도 실행하지 않는다(모든 사례 `not_run`, 종료 코드 1).

## 격리와 신뢰 경계

- 사례마다 `mkdtemp` 아래 새 프로젝트·HOME·(Windows) LOCALAPPDATA를 만든다. 저장소 안의 임시 경로, 저장소의 `.gil` 생성, 실행 중 감시 원문 변경을 실패로 본다.
- Core 자식 환경은 `isolatedEnv`가 만든다. 실행 파일은 `--gil` 또는 `--build` 결과뿐이다. 일반 Cargo 환경으로 빌드하며 필요하면 `CARGO_TARGET_DIR`로 빌드 위치를 지정한다. `--unqueued`는 기존 호출 호환용으로만 허용한다.
- 호출은 허용 목록(모델 도구·App 도구·표시 요청)만 가능하다. 기록 파일은 읽기만 하고 Verify의 작업 파일만 임시 프로젝트 안에 쓴다.
- **합성 승인**: `gil_plan_decide`는 harness가 App 전용 경로로 보낸 합성 입력이다. Core는 이를 `monitor_button` 영수증으로 기록하지만,
  보고서는 `human_approval: not_observed`로 적고 사건 기록에 `actor: harness, synthetic: true`를 남긴다. 사람의 승인·화면 표시의 증거가 아니다.
- 문서 구간과 fixture 문장은 데이터다. 문서의 코드 블록이나 셸 명령을 실행하지 않는다.

## 범위 밖

abandoned branch basis 참조(revisit 계보 필요), 승인 항목 분할·대체(설계 미결), Closing Interview·`chain_verdict`, 다른 언어 locale의 표지,
Windows의 실제 Core 사례 실행은 미확인이다. 자동 검사에는 Mac의 실제 사례·회귀 시험과 Mac·Windows의 원문·보고서 자체시험을 연결했다. 의미 품질은 `semantic_only`로 남긴다.
