# Data Science — 기대 증거와 trace

1. C1 Interview → D1–D4 승인.
2. C2 Experiment(D1): `evidence/D1-fertilizer-b.md` (평균 B≈26.7, control≈20.3, n=20/20) → success.
   - milestone: 사용자에게 D1 결과를 보고하고 확인을 기다린다(HC3). 채팅 질문 뒤 다음 gil_open 전에 사용자 메시지가 있어야 한다.
3. C3 Experiment(D2): `evidence/D2-water.md` (상관 r≈0.0, 구간 평균 차이 없음) → Outcome failure → Cycle failure.
   - `gil_revisit` 호출 시 Core 거절 → expected_blocked. 다음 Experiment를 그대로 연다(형제 실험).
4. C4 Experiment(D3): `evidence/D3-a-vs-b.md` (A≈23.3, B≈26.7) → success.
5. C5 Experiment(D4): `evidence/findings.md` (세 결과 + 한계: 합성 자료, 관측 연구 아님) → success.
6. 생성 시 측정값: control 20.25, A 23.25, B 26.73, r(water, height) ≈ 0.06 (rubric 범위 참조).

기대 판정: verify_file_evidence pass · one_purpose_per_node pass(태그 사용 시) · restore_revisit_blocked expected_blocked(호출 시) · human_approval_real unknown(사람 기록 없을 때).
