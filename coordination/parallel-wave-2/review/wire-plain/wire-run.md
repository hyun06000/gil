# Approved-plan wire regression — 2026-11-01

- HEAD: `628da7020f8cf317905393d32f26d3a054f39e51` · features: `approved-plan,product-safety` · binary sha256: `942b745ad5d59780…` · built in this command: true
- platform: darwin-arm64 · node v26.5.0 · queue: cargo-queue.py
- verdict: **pass** · pass=20, expected_blocked=1
- Monitor display: unknown (protocol only). Human button click: none; every App decision was a synthetic harness call and earns no user-consent score.
- claims: Core contract **pass** · model task: not_run (no model was called; synthetic App input only) · native Windows: not_run (host is not Windows) · record layout: inline

| case | status | title | detail |
| --- | --- | --- | --- |
| W01 | pass | tools/list exposes the approval contract with App/model visibility | 25 tools; plan tools present with correct visibility |
| W02 | pass | gil_start once, then the returned show_gil_monitor; display stays unconfirmed | {"storage_format":null,"record_layout":"inline","record_storage":null,"display_confirmed":null,"monitor_state":"opening","note":"protocol success only; visibility is unknown without a human"} |
| W03 | pass | Interview Question → Interpretation → open Synthesis | {"basis":["step:C1/S1","step:C1/S2"],"synthesis":"step:C1/S3"} |
| W04 | pass | bare approved:yes close is refused; App review without a proposal carries no capability | refused; state unchanged; no capability without a proposal |
| W05 | pass | propose; read shows revision 1 without receipt; identical re-propose is idempotent | {"revision":1,"digest":"e88d32bb5037…"} |
| W06 | pass | App review returns the private capability only in _meta and writes nothing | capability only in _meta; state bytes unchanged; stable within process |
| W07 | pass | decide with another scope, a wrong token, or an extra field is refused/rejected | all refused; no receipt; state unchanged |
| W08 | pass | editing the open proposal creates revision 2 and voids the earlier capability and close_report | revision 2 proposed; revision-1 decision and close refused |
| W09 | pass | process restart keeps the scope, reissues the capability, refuses the old one | scope restored without prepare; new capability accepted; old refused |
| W10 | pass | duplicate approve is idempotent; flipping to request_changes is refused | idempotent; flip refused; no capability after receipt; no token in record |
| W11 | pass | Synthesis closes only with the exact close_report; a tampered statement is refused | closed with the recorded report only |
| W12 | pass | Experiment is refused until Outcome and the Interview Cycle are closed, then binds item A | {"experiment":"C2","bound":"A","pending":["B"]} |
| W13 | pass | Verify needs the bound plan document bytes plus separate fresh evidence | C2 closed success; four refusals before the valid close |
| W14 | expected_blocked | gil_restore / gil_revisit are blocked in this profile (expected_blocked) | restore: 거절: 이 제품판에서는 `gil restore` 를 사용할 수 없다. \| revisit: GIL 거절: 되돌아가지 않았다. 이유가 dirty 세계·승인 계획 바인딩·적어 둔 방향 없음이면 그 조건을 사용자에게 알린다. gil_revisit을 반복하거나 restore를 자동 실행하지 않는다. |
| W15 | pass | reinterview request is preserved through the current Experiment and forces an Interview next | {"experiment":"C3","interview":"C4","preserved":["항목 D가 B 다음에 필요하다"]} |
| W16 | pass | request_changes closes with approved:no, re-proposal must keep approved items, then re-approval | {"interview":"C4","revisions":[3,4],"pending":["D"]} |
| W17 | pass | the inserted item D executes under revision 4 with the revision-4 document | {"experiment":"C5","executed":"A@2,B@2,D@4"} |
| W18 | pass | closing Interview needs its own approval; Chain closes; the next Chain cannot reuse it | {"closingInterview":"C6","nextChainInterview":"C7"} |
| W19 | pass | clean shutdown: EOF exits 0 and stdout carried only JSON-RPC frames | {"frames":138,"generations":2} |
| W20 | pass | proxy with explicit synthetic App decision: recorded as harness input, never as a person | {"harnessEvents":2,"human_approval_real":"unknown","no_action_while_synthesis_open":"fail"} |
| W21 | pass | scorer replays this run: synthetic approval never scores as human consent or display | {"monitor_show_called":"pass","monitor_display_confirmed":"unknown","approval_before_execution":"pass","human_approval_real":"unknown","no_action_while_synthesis_open":"unknown","one_purpose_per_node":"unknown","verify_f |


## Reproduce

```sh
python3 coordination/parallel-wave-2/cargo-queue.py -- node tools/scenarios/wire-runner.mjs --build --out <dir>
```

Failures keep the temporary work dir; inspect wire-frames.jsonl (redacted frames, per generation) and wire-events.jsonl.