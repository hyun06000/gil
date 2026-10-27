---
id: step/verify/close
title: Verify 를 닫아 세계를 확정한다
summary: 가설대로 실행한 결과를 적어 닫는다. 그 순간 지금 폴더가 관측되어 Snapshot 이 된다. Verify 에는 판정이 없다.
applies_when:
  cycle_kind: experiment
  step_kind: verify
  step_status: open
related:
  - artifact/dirty/non-verify
  - artifact/restore
examples:
  - step/verify/close
---

[언제 읽는가]
  Experiment Cycle 에서 verify 자리가 열려 있고, 가설대로 실행을 마쳤을 때.
  파일을 바꿨고 그 변경을 세계로 남겨야 할 때.

[지금 할 일]
  Report 에 이 자리가 요구하는 칸을 적어 닫는다.

{{close_requires:experiment/verify}}

[불변식]
  Verify 를 닫는 순간 지금 프로젝트 폴더가 관측되어 Artifact 세계로 확정된다.
  세계를 확정할 권한은 Verify 에만 있다. 그 밖의 자리는 세계가 바뀌지 않았을 때만 닫힌다.
  같은 세계를 다시 확정하면 새 이름이 나지 않는다 — 기존 Snapshot 이름을 그대로 쓴다.
  새 Verify 종료는 실제 증거 파일이 필수다. 읽기·검사만 했어도 관측 문서를 저장한다.
  Report의 evidence_files에 프로젝트 상대 경로를 한 줄에 하나씩 적는다.
  모든 파일은 확정할 Snapshot에 있으며 비어 있지 않아야 하고, 적어도 하나는 기준 세계보다
  새롭거나 내용이 달라야 한다. 경로 누락·빈 파일·이전 증거만 재사용하면 닫히지 않는다.
  채팅 본문이나 시각자료 not_needed/missing 선언은 이 조건을 면제하지 않는다.
  과거에 증거 파일 없이 닫힌 기록은 읽을 수 있지만 새 검증을 통과한 기록으로 바꾸지 않는다.
  Verify 에는 verdict 가 없다. 관측한 것을 적을 뿐, 성공인지 실패인지는 여기서 말하지 않는다.
  가설과 맞는지 읽는 것은 analysis 이고, 이 Cycle 을 판정하는 것은 outcome 이다.

[올바른 예]
      gil close <<'EOF'
      execution: 가설대로 캐시 계층을 넣고 같은 부하를 다시 걸었다
      result: p95 가 420ms 에서 180ms 로 내려갔고 오류율은 그대로였다
      evidence_files: evidence/cache-load-test.md
      EOF

  위 파일은 닫기 전에 실제 측정 조건·관측값·한계와 함께 저장되어 있어야 한다.
  닫히면 그 자리와 확정된 세계를 알려 준다. Analysis는 실제 Verify에 연결되고 Outcome의
  lesson은 실제 Analysis에 연결된다. 해석과 배움은 비어 있을 수 없다.

[format 5의 MCP 입력]
  format 5의 Verify는 일반 텍스트 close 예제만으로 닫히지 않는다.
  연결된 gil_close에서 project_root, report와 expected_owner, visuals를 함께 보낸다.
  expected_owner는 {"kind":"step","ref":"현재 열린 Step 참조"}다.
  이미지가 필요 없는 문서 증거라면 visuals는
  {"attachments":[],"uses":[],"declaration":{"state":"not_needed","reason":"실제 이유"}}다.
  이미지를 첨부하면 연결된 스키마의 attachments/uses와 attached 선언을 쓴다.
  evidence_files는 여전히 Report에 적는다. 별도 도구를 찾거나 텍스트 close를 반복하지 않는다.

[흔한 실패]
  Verify 에 성패 판정을 적으려는 것.

      verdict: success

  거절된다. verdict 는 outcome 의 칸이다. Verify 는 무엇을 했고 무엇이 보였는지만 적는다.

[관련 주제]
  artifact/dirty/non-verify
  artifact/restore
