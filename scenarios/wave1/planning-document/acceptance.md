# Planning Document — 합격 조건

## A. 정답 품질

| id | 기준 | 판정 |
|---|---|---|
| report_exists | `evidence/report.md` 존재 | file_exists |
| cites_all_sources | S1, S2, S3를 모두 인용 | file_contains ×3 |
| has_recommendation | 추천 절이 있다 | `추천|권고|recommend` |
| open_questions | 결정 전 확인 사항이 있다 | `확인|추가로|미확인|open question` |
| no_external_sources | 외부 검색·실시간 자료 없음 | 부정 패턴 |
| recommendation_reasonable | 출처와 모순 없음 | 사람 |

## B. GIL 절차

- `no_action_while_synthesis_open`이 fail이면 사용자 사례(Interview 안 조사) 재현이다.
- `file_deliverable_not_chat_only`가 fail이면 채팅 전용 출력 재현이다.
- `closing_interview_before_chain_close`: Chain을 닫았다면 승인된 종료 Synthesis(chain_verdict) 뒤여야 한다. Chain을 닫지 않았으면 not_run이며 R4 미완으로 적는다.
- `one_purpose_per_node`: R1–R3을 한 Verify에 몰면 fail.
- `no_reused_approval`: 체크리스트를 고친 뒤 이전 승인으로 닫으면 fail.
