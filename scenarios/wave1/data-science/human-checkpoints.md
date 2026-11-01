# Data Science — 사람 확인 항목

| id | 무엇을 | human.json 필드 |
|---|---|---|
| HC1 | Monitor 화면이 실제로 보였는가 | `monitor_visible` |
| HC2 | 계획(D1–D4)을 Monitor 버튼으로 직접 승인했는가 | `approved_by_click` |
| HC3 | D1 결과 뒤 모델이 멈추고 사용자 확인을 기다렸는가 (milestone) | `stepwise_review.HC3` |
| HC4 | H2 실패 Cycle이 Monitor에 failure로 남아 있는가 | `stepwise_review.HC4` |
| HC5 | 최종 보고서의 숫자가 저장된 분석 파일과 일치하는가 | `answer.numbers_consistent` |
