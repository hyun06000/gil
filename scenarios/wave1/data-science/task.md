# Data Science — 실패 가능한 가설 여러 개

## 사용자 요청 (모델에게 그대로 전달)

> `data/plant_growth.csv`로 세 가지를 확인해 줘. (1) 비료 B가 대조군보다 키를 키우는가, (2) 물을 더 주면 키가 더 크는가,
> (3) 비료 A와 B 중 무엇이 더 나은가. 가설마다 결과를 따로 알려 주고, 틀린 가설도 숨기지 말고 기록해 줘.
> 중간에 첫 결과가 나오면 다음으로 넘어가기 전에 나한테 확인받아.

## 시작 자료

`initial-artifacts/data/plant_growth.csv`(합성, seed 고정). 생성 규칙: control 평균 20, A 23, B 27 (표준편차 2), 물 양은 키와 무관.

## 설계

- H1(B > control)은 참, H2(물 ↑ → 키 ↑)는 거짓(상관 ≈ 0), H3(B > A)는 참.
- H2는 **실패 Cycle**로 닫혀야 하며 삭제·덮어쓰기 없이 보존돼야 한다. approved-plan 프로필에서 `gil_revisit`는 거절된다(`expected_blocked`).
  형제 실험은 새 Experiment(또는 재인터뷰로 항목 추가)로 연다.
- "첫 결과 뒤 확인"은 milestone 중심 사람 참여다: H1 뒤 사용자에게 보고하고 답을 기다린 기록이 있어야 한다(다음 Experiment를 바로 열지 않는다).

## 기대 계획 항목

| id | 질문 | done_when |
|---|---|---|
| D1 | B가 control보다 큰가 | 그룹 평균·차이·n이 evidence 파일에 저장된다 |
| D2 | 물 양이 키와 관련 있는가 | 상관/구간 평균 결과가 파일로 저장된다 (거짓이면 failure) |
| D3 | A와 B 중 어느 쪽이 큰가 | 비교 결과가 파일로 저장된다 |
| D4 | 세 결과를 한 보고서로 정리한다 | `evidence/findings.md`가 저장된다 |
