---
id: artifact/dirty/non-verify
title: Verify 가 아닌 자리에서 Artifact 가 바뀐 경우
summary: 이 자리에는 세계를 확정할 권한이 없다. 기준 세계를 복원한 뒤 같은 행동을 이어서 닫는다.
applies_when:
  project: present
  world_state: dirty
  artifact_confirmation: unavailable
related:
  - artifact/restore
  - step/verify/close
---

[언제 읽는가]
  Verify 가 아닌 자리를 닫으려는데 「이 자리에서는 Artifact 변경을 확정할 수 없다」로 거절됐을 때.
  gil status 가 `상태: dirty` 이고 열린 자리가 verify 가 아닐 때.

[지금 할 일]
  닫기 전에 변경점과 영향을 보고하고 출처·의도를 사용자에게 확인한다. 에이전트가 현재 맥락으로
  설명하지 못하는 수정은 작성자를 단정하지 않는다. 필요한 변경이면 내용과 의도를 먼저 보존한다.
  확인 없이 폐기·복원하지 않는다. 필요한 변경은 기준 세계 복원과 현재 노드 종료 뒤 다음 사이클의
  Verify에서 수행한다. 사용자의 필요 확인이 지금 단계의 변경 확정 권한을 만들지는 않는다.
  안전한 보존·정리 수단이 없으면 한계를 알리고 종료를 보류한다.

  현행 preview에서는 restore를 자동으로 실행하지 않는다. Project가 폐기 가능한 격리 fixture가
  아니면 멈추고 변경을 별도로 보존한다. 현재 restore는 .gitignore를 존중하지 않고 목표
  Snapshot에 없는 일반 파일을 삭제할 수 있다.

  사용자가 그 범위를 이해하고 격리 fixture의 복원을 명시적으로 요청한 경우에만 실행한다.

      gil restore

  그 다음 원래 하려던 대로 이 자리를 닫는다.

      gil close

[불변식]
  Artifact 세계를 확정할 권한은 Verify 에만 있다.
  거절돼도 현재 Step 과 Active Will 은 그대로 열려 있다. 되돌린 뒤에도 그대로다.
  복원은 행동을 취소하지 않는다. 하려던 일을 마치고 같은 자리를 정상 Report 로 닫는다.
  지금 이 자리에서 Verify 로 건너뛰는 길은 없다. Verify 는 Experiment Cycle 안의 자리이고,
  이 Cycle 을 닫는 것도 같은 판정에 막힌다 — 그러니 먼저 되돌리는 수뿐이다.

[올바른 예]
      gil restore
      gil close

[흔한 실패]
  바꾼 파일을 남겨 둔 채 계속 닫으려 하는 것. 같은 이유로 계속 거절된다.
  변경이 실험의 결과라면 그 변경은 Verify 자리에서 다시 만들어 확정한다.

[관련 주제]
  artifact/restore
  step/verify/close
