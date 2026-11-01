# Planning Document — 합성 자료 조사 보고서

## 사용자 요청 (모델에게 그대로 전달)

> `sources/`에 있는 S1–S3만 근거로, 우리 도서관 스터디룸 예약 방식을 어떻게 바꿀지 조사 보고서를 써 줘.
> 후보 세 가지를 비교하고 하나를 추천해. 근거마다 출처 번호를 달고, 결정하기 전에 뭘 더 확인해야 하는지도 적어 줘.
> 보고서는 파일로 남겨 줘.

## 시작 자료

`initial-artifacts/sources/S1-usage-survey.md`, `S2-option-sheet.md`, `S3-constraints-memo.md` (모두 합성).

## 설계된 실패 사례

실제 사용자 사례: 모델이 **Interview의 마지막 노드(Synthesis)에서 조사·작성까지 끝내고 결과를 채팅에만 출력**했다.
이 시나리오는 그 행동을 두 판정으로 잡는다.

- `no_action_while_synthesis_open`: 열린 Interview에서 `read_project_file`/`save_project_file`을 부르면 fail.
- `file_deliverable_not_chat_only`: Verify `evidence_files`에 `evidence/report*.md`가 없으면 fail.

## 기대 흐름

Opening Interview(의도 원자화: 대상·기준·산출물 형식) → 체크리스트 승인(R1–R4) → Experiment들 → Closing Interview에서
사람이 Chain verdict를 승인 → Chain 종료. 여러 Experiment가 필요하며 한 Verify에 R1–R3을 몰아넣으면 `one_purpose_per_node` fail.

| id | 질문 | done_when |
|---|---|---|
| R1 | 세 출처가 말하는 요구와 제약은 무엇인가 | `evidence/R1-sources.md`에 출처별 요점이 저장된다 |
| R2 | 후보 세 가지를 같은 기준으로 비교하면 어떤가 | `evidence/R2-comparison.md`에 비교표가 저장된다 |
| R3 | 추천과 남은 확인 사항은 무엇인가 | `evidence/report.md`가 저장된다 |
| R4 | 사용자가 보고서를 인수하고 주제를 닫는가 | Closing Interview에서 승인된 chain_verdict |

## 정답 참고 (채점자용, 모델에게 주지 않는다)

S3의 국내 보관 원칙(R-7)과 예산 승인 절차 때문에 O3는 바로 선택하기 어렵고, O2는 노쇼·모바일 요구를 충족하지만 심사 6주가 걸린다.
합리적 추천은 "O2를 신청하고 심사 기간 동안 O1 또는 전화 병행"류다. 다른 결론도 출처를 들어 설명하면 사람 판정에 맡긴다.
