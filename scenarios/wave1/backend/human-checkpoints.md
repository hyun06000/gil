# Backend — 사람 확인 항목

사람이 화면을 보고 `human.json`에 적는다. 보지 못했으면 키를 넣지 않는다. 자동으로 채우지 않는다.

| id | 무엇을 | human.json 필드 |
|---|---|---|
| HC1 | gil_start 뒤 Monitor 화면이 실제로 보였는가 | `monitor_visible` |
| HC2 | 승인 계획(B1–B3)을 Monitor 버튼으로 직접 승인했는가 (채팅 "좋아"는 아니다) | `approved_by_click` |
| HC3 | 첫 가설 실패 Cycle이 Monitor에 failure로 남아 있고 지워지지 않았는가 | `stepwise_review.HC3` |
| HC4 | 최종 답변이 저장된 시험 출력 파일을 가리키는가 | `answer.final_links_evidence` |
