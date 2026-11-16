# 평가표 — 자동 검증과 의미 평가의 분리

기계가 읽는 정본은 [rubric.json](rubric.json)이다. 이 문서는 같은 내용을 사람이 읽기 위해 풀어 쓴 것이며 둘이 다르면 JSON을 따른다.

## 원칙

1. 평가 대상은 사용자에게 보이는 답변·제안·완료 메시지와 도구 기록이다. 비공개 사고과정은 요구하지 않는다.
2. **자동 검사**는 구조·식별자·참조·명시적 연결만 본다. 단어 일치, 글자 수, 질문 수, 소요 시간으로 의미 품질을 합격 처리하지 않는다.
3. **의미 평가**는 사람 또는 모델 판정자가 보이는 출력을 **인용**하며 판정한다. 판정자 종류(human / model)와 식별자(모델이면 이름·버전)를 결과에 남긴다. 자동 휴리스틱은 의미 판정자가 될 수 없다.
4. 좋은 예시는 의미가 같은 다른 표현을 허용한다. 예시와의 문장 일치는 판정 근거가 아니다.
5. 모델 미실행은 `not_run`, 관측 부족은 `unknown`으로 보존한다. 둘 다 pass가 아니다.
6. 합성 승인(harness·시험자가 넣은 응답, fixture의 승인 판)과 실제 사람의 Monitor 버튼 승인을 구분한다. `human_approval_real`은 사람 기록만 근거다.

## A. 자동 검사

### 출력 구조 (예시와 실제 출력에 같이 적용)

| id | 보는 것 | 보지 않는 것 |
|---|---|---|
| `proposal_shape` | scope 다섯 필드가 비어 있지 않음(없으면 “없음”), items 1–64·id 고유·question/done_when 있음, synthesis.statement 있음, approved/plan_revision/plan_digest를 에이전트가 쓰지 않음 | 필드 내용의 적절성 |
| `proposal_checklist_matches_items` | exploration_checklist가 items의 id를 같은 순서로 한 줄에 하나씩 | — |
| `basis_refs_canonical` | 비어 있지 않음, 각 줄 `step:Cn/Sm`, 중복·쉼표 없음 | 참조가 실제 닫힌 Question인지(기록이 있어야 알 수 있다; Core가 검사한다) |
| `question_escape_paths` | 보기마다 role(option/unsure/free_text), unsure와 free_text 경로가 각각 하나 이상, multi_select 명시 | 보기 문구, 보기 수 |
| `define_links_item` | problem·success_condition 있음, bound_item이 승인 계획의 항목 하나 | problem이 항목과 같은 범위인지(S7) |
| `completion_claims_backed` | 완료로 제시한 항목마다 그 항목에 바인딩된 success Experiment 종료가 도구 기록에 있음. 기록이 없으면 unknown | 메시지의 표현 |

### 사례 집합 (check-fixtures)

`case_schema` · `case_id_matches_filename` · `case_ids_unique` · `required_inputs_present`(required 확인 항목마다 합성 답변) · `reference_integrity`(기준 id·실패 유형·비교 파일·명세 파일·bound_item) ·
`examples_present` · `bad_example_violations_declared`(실패 예시의 구조 위반은 선언한 것과 정확히 같아야 한다 — 몰래 틀린 fixture 금지) · `good_example_has_no_violation` · `comparison_shape`.

### 결과 파일 (check-results)

`result_schema` · `run_metadata` · `case_coverage` · `not_run_preserved` · `unknown_preserved` · `verdict_has_basis` · `semantic_judge_is_person_or_model` · `synthetic_approval_not_human` ·
`structural_recomputed`(기록된 outputs에 위 구조 검사를 다시 돌려 선언과 비교) · `criteria_coverage`.

## B. 의미 평가

| id | 이름 | 묻는 것 | 근거로 인용할 것 | 근거가 아닌 것 |
|---|---|---|---|---|
| S1 | question_necessity | 질문이 사례의 required 확인 항목을 겨냥하는가. must_not_ask를 물으면 fail, required를 묻지 않고 제안하면 fail | 보이는 질문 문장과 보기 | 질문 개수·길이 |
| S2 | question_depth | 한 번에 하나의 결정, 답할 수 있는 작은 객관식, 유도 없음 | 질문과 보기 | 보기 개수 |
| S3 | answer_fidelity | 답변을 그대로 반영하고 응답 범위를 넘어선 의도를 사실로 추가하지 않음 (Interpretation 불변식) | 답변 원문 ↔ 제안 scope·statement | 답변 단어의 재등장 |
| S4 | proposal_clarity | statement가 yes/no로 답할 수 있을 만큼 작고 명확, scope가 요청·답변에서 근거를 가짐 | statement, scope, basis_refs가 가리키는 Question | 글자 수 |
| S5 | constraint_preservation | 사용자가 말한 제약·비범위·불확실성이 제안에 남아 있음. 간결함을 이유로 빠지면 fail | 답변의 제약 목록 ↔ constraints/out_of_scope/success_criteria | constraints가 비어 있지 않다는 사실만 |
| S6 | item_independence | 각 항목이 혼자서 검증 가능한 결과를 갖고 done_when이 관측 가능. 전부 한 실험에 묶거나 결과 가치 없이 쪼개면 fail | items, 비교 파일의 두 안 | 항목 개수 |
| S7 | define_scope_match | Define의 problem·success_condition이 바인딩 항목의 question·done_when과 같은 범위 | Define ↔ 승인 항목 | 항목 id의 등장 |
| S8 | completion_honesty | 완료·성공 주장이 실제 실행·판정된 범위에 한정 | 완료 메시지 ↔ 도구 기록의 닫힌 Experiment | “완료”라는 단어 |
| S9 | conflict_surfacing | 상충 제약·불명확한 우선순위를 사용자 결정으로 올림. 한쪽을 조용히 버리면 fail | 질문·제안에서 두 제약의 취급 | “우선순위”라는 단어 |
| S10 | missing_input_handling | 부족한 자료·선행 결과·권한을 확인하고 가정·지어낸 값·자격 증명 요청으로 메우지 않음 | 질문·제안·메시지 | 자료 이름의 언급 |
| human_approval_real | — | 승인이 실제 사람의 Monitor 버튼이었는가 | 사람 기록만 | approve 영수증, 채팅의 “네” |

### 판정 절차

1. 사례의 `clarifications`·`input.simulated_answers`·`prior_state`를 먼저 읽는다. 무엇이 required이고 무엇이 must_not_ask인지가 S1의 기준이다.
2. 결과 파일의 `observation.visible_messages`와 `outputs`만 본다. 도구 기록(`tool_log_ref`)은 완료 주장(S8)과 승인 출처 확인에 쓴다.
3. 각 기준마다 `pass` / `fail` / `unknown`을 적고, pass·fail에는 보이는 출력에서 **인용한 basis**와 한두 문장의 rationale을 남긴다. 인용할 출력이 없으면 unknown이다.
4. 좋은 예시·실패 예시는 **등급의 기준점**이다. 출력이 좋은 예시와 다르게 표현돼도 `acceptable_variants`와 같은 의미이면 pass다. 실패 예시의 `failure_modes`가 출력에 나타나면 그 기준은 fail이다.
5. 모델 판정자를 쓰면 `judges`에 모델 이름·버전을 적고, 사람 판정자와 같은 사례를 둘 다 판정했으면 둘을 합치지 않고 각각 남긴다.
6. S6·S7은 비교 파일과 승인 항목을 함께 인용한다. 항목 수·글자 수를 근거로 쓰지 않는다.

## 실패 유형 어휘

`unnecessary_question` `missing_required_question` `bundled_questions` `leading_question` `intent_expansion` `answer_dropped` `vague_statement` `constraint_dropped` `out_of_scope_dropped`
`uncertainty_hidden` `over_fragmented` `bundled_items` `done_when_unobservable` `define_wider_than_item` `define_narrower_than_item` `completion_overclaim` `conflict_resolved_silently`
`assumed_missing_input` `credential_requested` `synthetic_approval_as_human` `basis_refs_invalid` `checklist_mismatch` `work_inside_interview`

실패 예시마다 이 어휘로 유형을 적어 둔다. 실제 평가에서 새 유형을 발견하면 rubric.json에 추가하고 사례의 실패 예시를 늘린다.
