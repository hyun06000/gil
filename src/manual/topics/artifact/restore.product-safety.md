---
id: artifact/restore
title: 제품판에서 직접 파일 복원은 차단되어 있다
summary: 이 제품판은 gil restore 를 거절하고 파일을 바꾸지 않는다. 실패 Cycle 의 승인된 revisit 과 직접 restore 는 다른 경로다.
applies_when:
  world_state: dirty
related:
  - artifact/dirty/non-verify
  - cycle/revisit
  - current
---

[언제 읽는가]
  제품판에서 직접 restore 가 거절됐거나 dirty 상태의 복구 방법을 확인할 때.

[지금 할 일]
  gil status 로 변경 목록과 현재 위치를 읽는다. 직접 restore 를 반복 실행하지 않는다.
  실패 Cycle 의 revisit 이라면 Monitor 의 변경 검토·승인 복구 경로와 현재 응답을 따른다.
  그 밖의 비-Verify 변경은 artifact/dirty/non-verify 안내에 따라 원본을 보존하고 사람이 정리한다.

[불변식]
  이 구성은 인수 유무와 관계없이 직접 restore 를 거절한다. 파일·Graph·Journey 를 바꾸지 않고
  복원 transaction 을 시작하지 않는다. --force 나 추가 인수로 우회할 수 없다.
  이 차단은 실패 Cycle 의 승인된 revisit 을 금지한다는 뜻이 아니다. 복구 대상과 승인 검사는 유지한다.
  기존에 중단된 transaction 의 읽기·복구 규칙과 새 직접 복원 요청의 거절을 혼동하지 않는다.

[올바른 예]
  직접 복원이 거절되면 현재 변경·실패 Cycle 여부를 확인하고 허용된 복구 경로를 따른다.
  파일이 복원됐다고 말하거나 실패를 성공으로 바꾸지 않는다.

[흔한 실패]
  기본 개발 구성의 gil restore 예시를 제품판에서 반복 실행한다.
  직접 restore 차단을 우회하려고 프로젝트 기록을 수정한다.

[관련 주제]
  artifact/dirty/non-verify
  cycle/revisit
