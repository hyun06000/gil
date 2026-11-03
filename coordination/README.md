# 개발 도구와 과거 검토 기록

Wave 1·2의 분업과 통합은 완료됐다. 당시 작업 배정·로컬 branch 지시·결과 초안·모의 화면은
현재 작업 지침으로 사용하지 않는다. 원문은 [정리 전 이력](https://github.com/hyun06000/gil/tree/a992efa43c10ef4dc3ad97699cb504663d07288a/coordination)에 보존된다.
현재 개발 절차는 [CONTRIBUTING](../CONTRIBUTING.md), 제품 계약과 남은 설계는 [spec](../spec)을 따른다.

현재 사용하는 도구:

- [빌드 큐](parallel-wave-2/cargo-queue.py): worktree마다 target을 분리하고 저장소의 빌드/시험을 직렬화한다.
  시나리오 실행기의 `--build` 경로에서 사용한다. [도구 안내](../tools/scenarios/README.md)를 따른다.
- [큐 격리 시험](parallel-wave-2/queue_test.py): `python3 coordination/parallel-wave-2/queue_test.py`.
- [이전 진입점](parallel-wave-1/cargo-queue.py)은 같은 큐로 위임하는 호환 경로다.

[M6-D 검증](M6D-VALIDATION-20261101.md), [Wave 1 독립 검토](parallel-wave-1/review/REVIEW.md),
[Wave 2 통합 검토](parallel-wave-2/review/REVIEW.md)와 재현 자료는 당시 실패·보완·검증 범위를 추적하기 위해 남긴다.
이 기록의 미실행·미해결 상태를 최신 릴리즈의 상태로 해석하지 않는다. 최신 설치·검증 범위는 [INSTALL](../INSTALL.md)을 따른다.
