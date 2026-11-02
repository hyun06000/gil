---
id: artifact/dirty/non-verify
title: Verify 가 아닌 자리에서 Artifact 가 바뀐 경우
summary: 이 자리에는 세계를 확정할 권한이 없다. 변경을 사람이 정리해 기준 세계로 맞춘 뒤 같은 행동을 이어서 닫는다.
applies_when:
  project: present
  world_state: dirty
  artifact_confirmation: unavailable
related:
  - step/verify/close
---

[언제 읽는가]
  Verify 가 아닌 자리를 닫으려는데 「이 자리에서는 Artifact 변경을 확정할 수 없다」로 거절됐을 때.
  gil status 가 `상태: dirty` 이고 열린 자리가 verify 가 아닐 때.
  Verify 를 닫은 뒤 Cycle Report 전에 통합 보고서나 전달물 파일을 만든 경우가 흔한 원인이다.

[지금 할 일]
  이 빌드는 Artifact 되돌리기(restore)를 거부한다 — 복원 명령을 불러도 거절된다. 거절문과 `gil status` 가 보여 준
  변경 목록(새로 생김 · 바뀜 · 사라짐)을 보고 사람이 정리한다. GIL 은 파일을 지우거나 되돌리지 않는다.

  새로 생긴 파일은 프로젝트 밖의 새 보존 위치로 옮긴다. 그 내용을 남기려면 지금 자리를 닫은 뒤
  다음 Experiment 의 Verify 에서 다시 만들고 Report 의 evidence_files 에 적는다.
  바뀐 기존 파일은 현재 내용을 별도로 보존한 뒤 기준 내용으로 되돌린다. 사라진 파일은 기준 내용대로 다시 둔다.

  그 다음 원래 하려던 대로 이 자리를 닫는다.

      gil close

[불변식]
  Artifact 세계를 확정할 권한은 Verify 에만 있다.
  거절돼도 현재 Step 과 Active Will 은 그대로 열려 있다. 정리한 뒤에도 그대로다.
  정리는 행동을 취소하지 않는다. 하려던 일을 마치고 같은 자리를 정상 Report 로 닫는다.
  지금 이 자리에서 Verify 로 건너뛰는 길은 없다. Verify 는 Experiment Cycle 안의 자리이고,
  이 Cycle 을 닫는 것도 같은 판정에 막힌다 — 그러니 먼저 기준 세계로 맞추는 수뿐이다.

[올바른 예]
      사용자가 선택한 새 보존 폴더에 report.md 를 보관한다. 기존 파일을 덮어쓰지 않는다.
      gil close

[흔한 실패]
  바꾼 파일을 남겨 둔 채 계속 닫으려 하는 것. 같은 이유로 계속 거절된다.
  복원 명령을 반복해 부르는 것. 이 빌드에서는 언제나 거절된다.
  변경이 실험의 결과라면 그 변경은 다음 Verify 자리에서 다시 만들어 확정한다.

[관련 주제]
  step/verify/close
