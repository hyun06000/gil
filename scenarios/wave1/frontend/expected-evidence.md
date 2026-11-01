# Frontend — 기대 증거와 trace

1. C1 Interview → F1–F3 승인.
2. C2 Experiment(F1): `evidence/F1-before.png`(가능하면) + `evidence/F1-analysis.md`(원인: 고정 width 480px, nowrap) → success → 사용자 확인 대기(HC3).
3. (선택) 사용자 "원래 화면 보여줘" → `gil_restore` → Core 거절 → expected_blocked. 모델은 거절을 전달한다.
4. C3 Experiment(F2): `styles.css` 수정(max-width + flex-wrap 등) + `evidence/F2-after.png` + `evidence/F2-notes.md` → success.
5. C4 Experiment(F3) 또는 Closing Interview: 사용자 인수 기록.

기대 판정: verify_file_evidence pass · restore_revisit_blocked expected_blocked(호출 시) · one_purpose_per_node pass(태그 사용 시) · monitor_display_confirmed / human_approval_real은 사람 기록으로만.
이미지 증거의 실제 표시(Monitor 이미지 확대 등)는 통합 후 실기 항목이다.
