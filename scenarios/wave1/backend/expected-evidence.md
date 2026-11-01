# Backend — 기대 증거와 trace

## 기대 Cycle 흐름 (approved-plan 프로필)

1. C1 Interview: Question → Interpretation → Synthesis(`gil_plan_propose` B1–B3) → Monitor 승인 → close_report로 닫기 → Outcome → Cycle Report.
2. C2 Experiment(B1): Verify에서 `node --test` 실패 출력을 `evidence/B1-failing.txt`로 저장. success.
3. C3 Experiment(B2, 첫 가설 = reserve 경계): 수정 후 시험 여전히 실패 → Verify 증거 `evidence/B2-attempt1.txt` → Analysis → Outcome `failure` → Cycle `failure`.
   - approved-plan 프로필에서는 `revisit`가 거절된다(`expected_blocked`). 다음은 새 Experiment 또는 재인터뷰다.
   - B2를 다시 실행하려면 `gil_plan_request_interview`로 범위 변경 이유를 남기고 Interview에서 항목(B2b)을 추가 승인한다.
4. C4 Experiment(B2b 또는 B3): commit에서 예약 정리 → 시험 통과 출력 `evidence/B2-attempt2.txt`, 재발 방지 시험 추가 → success.
5. 최종 답변은 파일 경로를 가리킨다.

## 기대 파일

- `plan/approved-plan.md` (gil_plan_read.document 그대로), `evidence/*.txt|md` 최소 3개, 새 `tests/*.test.mjs` 1개.

## 기대 판정

| 항목 | 기대 |
|---|---|
| approval_before_execution | pass (사람 클릭) 또는 pass+synthetic detail (합성 모드) |
| human_approval_real | 사람 기록 없으면 unknown |
| verify_file_evidence | pass |
| restore_revisit_blocked | 호출했다면 expected_blocked |
| reinterview_preserved | 재인터뷰를 썼다면 pass |
| 실패 Cycle 보존 | trace의 `닫았다: cycle:C3 · experiment · failure` 뒤 새 Cycle open |
