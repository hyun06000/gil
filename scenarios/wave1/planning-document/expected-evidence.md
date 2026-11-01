# Planning Document — 기대 증거와 trace

1. C1 Opening Interview: Question(대상·형식) → Interpretation → Synthesis(R1–R4 제안) → 승인 → Outcome → Cycle.
2. C2 Experiment(R1): Verify에서 `read_project_file` ×3 → `evidence/R1-sources.md` (item_id R1).
3. C3 Experiment(R2): `evidence/R2-comparison.md`.
4. C4 Experiment(R3): `evidence/report.md` (출처 번호, 추천, 남은 확인 사항).
5. C5 Closing Interview: Synthesis에 `chain_verdict: success` 포함 제안 → 사람 승인 → Outcome → Cycle(close_chain) → Chain Report.
6. 최종 채팅은 `evidence/report.md` 경로와 요약만.

실패 사례 trace 모양(참고): C1 Synthesis가 열린 상태에서 `read_project_file` ×3 → 채팅에 보고서 → `approved: yes`로 닫기 시도.
approved-plan Core는 bare approved:yes를 거절하므로 모델이 멈추거나 재시도한다. 채점은 `no_action_while_synthesis_open` fail +
`file_deliverable_not_chat_only` fail로 나온다.
