# Wave 1 scenario suite (M7 preparation)

네 가지 실제 작업 유형으로 GIL을 반복 검증하기 위한 **재현 가능한 시험 묶음**이다. 정답을 얻었는가(answer quality)와
GIL 절차를 지켰는가(procedure)를 **따로** 채점한다. 이 폴더는 시나리오 정의와 평가표만 담는다. 실행 도구는
[`tools/scenarios/`](../../tools/scenarios/README.md)에 있다. 이 세션에서는 비용이 드는 모델 호출·VM·설치·배포를 시작하지 않았다.

| 시나리오 | 요구하는 GIL 기능 | 특징 |
|---|---|---|
| [backend](backend/task.md) | 테스트로 판정 가능한 버그, 여러 Cycle, Verify 증거, 실패 뒤 방향 전환 | 첫 가설이 틀리도록 설계된 함정 |
| [data-science](data-science/task.md) | 실패 가능한 가설 여러 개, milestone 승인, 실패 Cycle 보존, 형제 실험 | 합성 CSV, 수원 가설은 거짓 |
| [frontend](frontend/task.md) | 시각 Artifact 변화, 단계별 사람 검수, 이전 화면 복원(차단), Monitor 변화 이유 | 좁은 화면 레이아웃 결함 |
| [planning-document](planning-document/task.md) | Opening Interview, 의도 원자화, 체크리스트 승인, 여러 Experiment, Closing Interview, 사람이 승인한 Chain verdict | 합성 출처 3개, Interview 안 조사·채팅 전용 출력은 실패 |

## 각 시나리오 폴더

```text
<scenario>/
├─ task.md               사용자 과제 원문과 기대 계획 항목
├─ initial-artifacts/    임시 프로젝트로 복사할 합성 시작 파일
├─ human-checkpoints.md  사람이 직접 관측·기록해야 하는 항목 (자동으로 채우지 않는다)
├─ acceptance.md         정답 품질 기준과 GIL 절차 기준을 분리한 합격 조건
├─ expected-evidence.md  기대 trace 모양, 증거 파일, 기대 판정 (expected_blocked 포함)
└─ rubric.json           채점기가 읽는 기계 판독 평가표
```

## 판정 어휘

`pass` · `fail` · `unknown`(관측 로그가 없어 판단 불가; 모델 내부 생각/실행을 추정하지 않는다) ·
`expected_blocked`(product-safety / approved-plan 잠금이 의도대로 거절) · `unsupported`(기능 flag/플랫폼 차이) ·
`not_run`(실행하지 않음). 실행하지 않은 시나리오를 pass로 세지 않고, 사람 확인 칸은 사람이 적기 전까지 `unknown`이다.

## 절차 판정 항목 (procedure)

| id | 뜻 | 근거 |
|---|---|---|
| monitor_show_called | gil_start 뒤 첫 gil_open 전에 show_gil_monitor 호출 | trace |
| monitor_display_confirmed | 화면이 실제로 보였는가 | **사람 기록만** |
| approval_before_execution | 그 Experiment가 **같은 Chain**의 유효한 승인 판에 연결되는가: 최신 제안(수정 전 승인은 무효)의 approve 영수증 → 그 판으로 닫힌 Synthesis → 닫힌 Interview Cycle → Experiment 안의 plan 조회가 그 판을 active로, 항목을 바인딩으로 보여줌. 이전 Chain·request_changes·응답 유실·로그 중간 시작은 pass가 아니다 | trace + plan 조회 (조회가 없으면 unknown; 합성 승인이면 detail에 표시) |
| human_approval_real | 승인이 사람의 실제 버튼 클릭이었는가 | **사람 기록만**; 합성 입력은 unknown. 사람 기록이 클릭을 주장해도 로그의 결정이 전부 합성이면 fail(승격 금지) |
| no_action_while_synthesis_open | 열린 Interview(특히 Synthesis)에서 조사·파일 작업을 하지 않았는가 | 외부 도구 로그 |
| one_purpose_per_node | 한 노드에 여러 항목의 작업을 섞지 않았는가 | item_id 태그 |
| verify_file_evidence | 모든 Verify가 실제 존재하는 비어 있지 않은 파일 증거를 적었는가 | trace + 프로젝트 폴더 |
| visual_evidence_attached | 사용자가 요구하지 않았는데도 Verify가 시각 자료(PNG)를 `attached`로 첨부했는가. rubric `procedure.visual_evidence_expected`가 참인 시나리오(data-science·frontend)에서 모든 Verify가 `not_needed`/`missing`이면 fail. typed 선언이 없는 기록은 unsupported | trace의 typed `visuals.declaration` |
| no_reused_approval | 수정된 제안·다른 Chain의 제안을 이전 승인으로 닫지 않았는가 (판·digest·Chain·영수증 시점 일치) | plan 조회 결과 |
| reinterview_preserved | 재인터뷰 요청이 다음 승인까지 보존되고 다음 Cycle이 Interview였는가 | plan 조회 결과 |
| restore_revisit_blocked | 제품 잠금이 restore/revisit을 거절했는가 | trace → expected_blocked |
| file_deliverable_not_chat_only | 산출물이 채팅이 아니라 파일로 남았는가 | evidence_files + 폴더 |
| closing_interview_before_chain_close | Chain이 **자기 Chain의** 승인된 종료 Synthesis(자기 영수증 포함) 뒤에만 닫혔는가 | trace + plan 조회 |

채점기 음성·양성 fixture: `tools/scenarios/fixtures/approval/*.jsonl`(파일 이름이 기대 판정), 시험 `tools/scenarios/approval-link.test.mjs`.
`score.json`의 `claims`는 Core 거절 관측(core_contract)과 모델 작업 판정(model_task)을 따로 적는다. 둘은 서로의 근거가 아니다.

## 하지 않는 주장

- 합성 모델 응답, 프로토콜 성공, DOM fixture, 합성 App 입력은 실제 사용자 승인이나 화면 인수가 아니다.
- 실제 Mac/Windows 설치본·Codex 화면·Luna 사람 참여 시험은 모든 세션 통합 후 한 번의 설치본으로 한다.
- 이 묶음은 제품 코드를 고치지 않는다. 발견한 결함은 결과 문서에 재현 조건과 함께 남기고 통합 담당자에게 넘긴다.

반복 실행 절차는 [PROTOCOL.md](PROTOCOL.md), 기록 양식은 [evaluation-sheet.md](evaluation-sheet.md)를 본다.
