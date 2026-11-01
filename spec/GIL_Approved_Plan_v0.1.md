# M6-D — 승인된 계획 개발 계약

상태: 개발 프로필 `approved-plan` (`chain-core` 포함). 기본 배포에 자동 활성화하지 않는다.
Cycle Model §12, §16–17, §21의 미정 저장 계약을 이 문서로 한정한다.

## 승인과 실행

- 새 Chain의 `approved_plan`은 제안 이력과 실행 바인딩을 보존한다. 기존 format 4/5/6/7 프로젝트는 자동 변환하지 않는다. format 6/7의 선택적 확장 필드이며 이 확장을 모르는 기존 writer는 unknown-field 오류로 거절한다. 기능을 끈 새 writer도 거절한다.
- 열린 Interview Synthesis에서만 제안한다. 제안은 Synthesis 주소, 순번, 기준 승인 판, 범위(goal/in_scope/out_of_scope/constraints/success_criteria), 순서 있는 항목(id/question/done_when), approved를 제외한 전체 Synthesis Report를 묶는다. SHA-256은 이 정규 JSON 전체를 묶는다.
- 같은 내용 재전송은 같은 제안을 돌려준다. 열린 Synthesis의 편집은 새 제안이며 이전 판의 승인은 재사용하지 않는다. 이전 제안과 응답은 이력에 그대로 보존하고, 새 제안에 다시 응답하기 전에는 Synthesis를 닫을 수 없다. 수정 요청을 그대로 닫으면 다음 Question으로 간다.
- Monitor는 전체 제안과 판·digest를 표시하고 실제 버튼 클릭으로 승인/수정 요청을 전송한다. 승인 데이터는 정확한 scope·판·digest에 묶인다. 수정 요청은 approved:no로만 닫을 수 있다. 에이전트가 approved:yes만 제출해서 닫을 수 없다.
- App 전용 도구의 visibility는 인증이 아니다. 앱에만 전달되는 `_meta`의 256-bit 임시 capability를 추가로 검증한다. 토큰은 모델 응답/Context/저장소에 넣지 않는다. 서버 재시작 시 재발급한다. 이것은 정상 MCP Host에서의 입력 경로 구분이지 물리적 개인 신원 인증이나 악성 로컬 프로세스 방어가 아니다.
- 승인은 Step을 자동으로 닫지 않는다. 버튼 처리 성공 후 앱은 사용자 동작에 따른 계속 요청을 Host 채팅에 보내며, 전달 실패 시 저장된 승인과 별개로 수동 계속 안내를 보여 준다. 자동 재조회는 메시지를 보내지 않는다. 에이전트는 승인 결과와 동일한 Report로 Synthesis, Outcome, Interview Cycle을 닫는다. 성공적으로 닫힌 Interview의 마지막 승인 Synthesis만 실행 가능한 계획이 된다.
- Experiment를 열 때 현재 승인 판의 첫 미실행 항목을 자동 바인딩한다. 과거 항목 정의와 실행 Cycle을 고치지 않는다. 새 Interview는 항목 추가·순서 변경을 허용하지만 기존 항목의 누락·정의 변경은 거절한다(취소/대체 의미는 별도 계약).
- `[A,B,C]` 승인 후 A 실행, 다음 Interview에서 `[A,D,B,C]` 승인 시 pending은 `[D,B,C]`다. executed는 열렸다는 뜻이며 성공/실패는 연결된 Cycle Report가 진실원이다.
- 읽기 응답(`gil_plan_read`/`gil_plan_review`)의 `supported:true`에는 read-only `current_cycle {ref, state}`가 있다. `ref`는 실제 현재 Cycle의 typed ref, `state`는 실제 닫힘 여부에 따른 `open`/`closed`다. `current_item`은 Cycle이 닫혀도 남는 바인딩이므로 열림의 증거가 아니다. 이 필드가 없는 응답(옛 Core, `supported:false`)에서 Monitor는 열림을 추정하지 않고 미확인으로 표시한다. 이 필드는 저장·승인·입력 의미를 바꾸지 않는다.
- 열린 Experiment 중 `gil_plan_request_interview`로 범위 변경 이유를 보존한다. 열린 Step/Cycle은 그대로 두며, 현재 Cycle을 사실대로 닫고 다음 Interview에서 승인하기 전에는 추가 Experiment를 열지 않는다.
- 일반 destructive restore와 승인 없는 제품 revisit는 차단한다. 닫힌 실패의 재시도는 [M4-R 승인 복구](GIL_Revisit_Recovery_v0.1.md)를 따른다. 이전 실행 바인딩은 보존하고 직전 실패의 revisit 계보에서만 같은 항목을 다시 바인딩한다. 최신 실패 항목은 pending으로 돌아오며 읽기 응답의 retry_pending에 표시한다. 취소·abort는 여전히 M6-E 별도 계약이다.

## 문서와 증거

Interview에서는 프로젝트 파일을 쓰지 않는다. 승인 계획의 결정적 Markdown은 read/export로 제공한다. 해당 Experiment의 Verify에서 파일로 저장하고 `plan_document`에 안전한 상대 경로를 적는다. 확정될 Snapshot의 bytes가 바인딩된 승인 판의 export와 같아야 한다. 별도의 새/변경된 실제 작업 증거를 `evidence_files`에 기록해야 하며 계획 문서만으로 실행 증거를 대신하지 못한다. 계획·문서·증거·Analysis·Outcome의 기존 연결을 함께 보존한다.

## 범위와 수용 검증

Core/저장 roundtrip, bare approval 거절, revision 교체, 중복 입력, 수정 요청, Interview 미종료 실행 거절, 큐 재정렬·과거 보존, 문서 bytes·실행 증거 검증, scope/token/digest 격리, UI 재연결을 자동 시험한다. 실제 Host 표시·물리적 버튼 클릭·Luna 실사용은 별도의 실제 환경 시험이다. 기본 배포 변경, 기존 기록 이전, M6-E 종료/취소 정책, M3 restore 안전성은 이 변경의 완료로 주장하지 않는다.

## 개발 검증 실행

```sh
cargo test --locked -p gil --features approved-plan,visual-authoring,product-safety --lib
cargo test --locked -p gil --features approved-plan,visual-authoring,product-safety --test context --test context_profiles --test guidance_wave1 --test guidance_wave2 --test approved_plan_wire_wave1
cargo build --locked -p gil --all-targets --features approved-plan,visual-authoring,product-safety
node --test mcp-app/plans.test.mjs
```

일반 `chain-core` 회귀와 기본 format 4/5 시험도 따로 실행한다. 이 프로필은 승인 없는 기존 합성 bootstrap을 의도적으로 거절하므로, 기존 전체 suite에 무조건 `--all-features`를 붙이는 것은 인수 기준이 아니다. 전용 suite의 사용자 응답은 명시적인 합성 fixture이며 실제 사람이 승인했다는 증거로 취급하지 않는다.

2026-11-02 문맥 인수 보완: 기존 context 투영 시나리오는 format 4 또는 승인 계획 확장 없는
format 6의 합성 기록을 실제 loader로 읽는다. 기본/Chain/승인 프로필 모두에서 옛 기록의
format·Chain 포함 관계·승인 ledger 부재를 보존하며, 새 승인 프로젝트의 맨 `approved: yes`
거절도 별도로 검사한다. 기존 시나리오를 새 승인 프로젝트로 오인해 부트스트랩하거나
프로필 시험을 건너뛰지 않는다. 실제 stdio 시험은 제안 대기·승인·Verify·Cycle 종료 후
Context의 read-only 성질, 종료 상태 안내, 서버 재시작 뒤 같은 문맥 복원을 검증한다.
합성 App 입력은 물리적 사용자 승인이나 Luna 실사용의 증거가 아니다.

## Monitor 조회 상태 (2026-11-02 보완)

승인 저장 결과와 화면의 최신 조회 상태를 구분한다. 최초 조회는 확인 중, 연속 1~2회 조회 실패는
중립적인 재확인 안내, 3회 이상은 지속적인 조회 실패와 원인·다시 읽기 버튼을 표시한다.
마지막으로 확인한 승인/수정 요청의 판과 기록은 조회 실패가 지우지 않는다. 다만 과거 확인 사실임을
명시하며 새 제안의 승인이나 현재 실행 상태로 해석하지 않는다. 조회 실패 시 capability와 승인 버튼은
비활성화하고 새로 읽은 제안에 대해서만 다시 표시한다.

성공한 조회만 30초 조용한 갱신 간격을 사용한다. 실패는 2·4·8·15초(상한)로 재시도하고 성공 후
초기화한다. 진행 중이라 생략한 조회를 성공으로 세지 않으며, 조회 도중 받은 변경 힌트를 잃지 않는다.
숨겨진 App의 polling 중단은 유지한다. 승인 저장 응답이 없거나 일치하는 receipt가 없으면 별도
‘저장 결과 불확실’을 유지하고 실제 기록을 다시 읽는다. 자동 승인 재전송이나 polling에 의한 채팅 전송은 하지 않는다.
