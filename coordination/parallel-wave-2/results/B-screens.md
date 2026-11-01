# B 합성 화면 검증 (fixture DOM)

`node coordination/parallel-wave-2/results/B-screens.mjs`가 생성한다. mcp-app/plans.mjs를 시험용 DOM으로 렌더한 텍스트이며 실제 Host 화면 인수가 아니다.

## 1 제안 없음 (idle)

banner: (숨김)
panel state=idle

```text
    <span> 승인 계획 없음
  <p> 에이전트가 Interview의 제안 단계에서 계획을 제안하면 여기서 승인하거나 수정을 요청합니다.
```

## 2 실행 항목 있음: Experiment 열림

banner: (숨김)
panel state=executing

```text
    <span> 승인된 계획
    <span>  · 계획 v1
  <p cycle=open> 지금 열린 Cycle cycle:C2에 묶인 항목: A — Question A ⏎ 확인 조건: File A
  <p> 실행함 0개 · 지금 열린 Cycle 1개 · 남음 1개
    <li status=current> 지금 열린 Cycle (cycle:C2) · A: Question A ⏎ 확인: File A
    <li status=pending> 남음 · B: Question B ⏎ 확인: File B
  <small> GIL에 기록된 사실입니다. “실행함”은 Cycle이 열렸다는 기록이며 성공 여부는 해당 Cycle Report가 진실원입니다. 에이전트가 지금 작업 중인지는 여기서 알 수 없습니다.
```

## 3 Cycle 닫힘 (R3 fixture: current_item 바인딩 유지)

banner: (숨김)
panel state=executing

```text
    <span> 승인된 계획
    <span>  · 계획 v1
  <p cycle=closed> 현재 Cycle cycle:C2은 닫혔습니다. 지금 열린 Cycle은 없습니다. 마지막으로 연결된 항목: A — Question A ⏎ 확인 조건: File A ⏎ 결과는 그 Cycle Report가 진실원이며 여기서 성공으로 보지 않습니다.
  <p> 실행함 1개 · 지금 열린 Cycle 0개 · 남음 1개
    <li status=executed> 실행함 (cycle:C2) · A: Question A ⏎ 확인: File A
    <li status=pending> 남음 · B: Question B ⏎ 확인: File B
  <small> GIL에 기록된 사실입니다. “실행함”은 Cycle이 열렸다는 기록이며 성공 여부는 해당 Cycle Report가 진실원입니다. 에이전트가 지금 작업 중인지는 여기서 알 수 없습니다.
```

## 4 다음 Cycle 열림

banner: (숨김)
panel state=executing

```text
    <span> 승인된 계획
    <span>  · 계획 v1
  <p cycle=open> 지금 열린 Cycle cycle:C3에 묶인 항목: B — Question B ⏎ 확인 조건: File B
  <p> 실행함 1개 · 지금 열린 Cycle 1개 · 남음 0개
    <li status=executed> 실행함 (cycle:C2) · A
    <li status=current> 지금 열린 Cycle (cycle:C3) · B: Question B ⏎ 확인: File B
  <small> GIL에 기록된 사실입니다. “실행함”은 Cycle이 열렸다는 기록이며 성공 여부는 해당 Cycle Report가 진실원입니다. 에이전트가 지금 작업 중인지는 여기서 알 수 없습니다.
```

## 5 재인터뷰: 닫힌 Experiment 뒤 새 판 제안

banner:   <span> 새 판 재승인 필요 · 계획 v2 — 이전 승인(v1)은 이 판에 적용되지 않습니다. 다시 확인해 주세요. |   <button> 승인 화면으로
panel state=reapproval

```text
    <span> 새 판 재승인 필요
    <span>  · 계획 v2 (이전 승인 판 v1)
  <p> 이 제안은 실행할 항목 2개를 담은 계획입니다. 전체 결과(Chain)를 끝내자는 제안은 아닙니다.
  <p> Proposed scope
    <dt> 목표
    <dd> goal
    <dt> 포함 범위
    <dd> in
    <dt> 제외 범위
    <dd> out
    <dt> 제약
    <dd> none
    <dt> 성공 조건
    <dd> proof
  <h3> 실행할 항목과 확인 조건
  <p> 실행함 0개 · 지금 열린 Cycle 1개 · 남음 1개
    <li status=current> 지금 열린 Cycle (cycle:C2) · A: Question A ⏎ 확인: File A
    <li status=pending> 남음 · B: Question B ⏎ 확인: File B
  <p> 재인터뷰 요청 기록 (cycle:C2): 범위가 바뀌었다 — 현재 Cycle을 닫은 뒤 다음 Interview에서 확인합니다.
  <p> 이 내용에 동의하는 경우 승인하세요. 내용을 바꾸려면 수정 요청 후 채팅으로 알려 주세요.
  <button decision=approve> 이 계획 승인하고 계속
  <button decision=request_changes> 수정 요청
    <summary> 승인할 제안문 전체 (참조·점검표 포함)
      <dt> statement
      <dd> Proposed scope
      <dt> basis_refs
      <dd> step:C1/S1
      <dt> exploration_checklist
      <dd> A ⏎ B
    <pre> { ⏎   "statement": "Proposed scope", ⏎   "basis_refs": "step:C1/S1", ⏎   "exploration_checklist": "A\nB" ⏎ }
    <summary> 승인 기록 정보 (판·제안 주소·digest)
    <p> 기준 승인 판: 1 · 제안: step:C4/S3 · SHA-256: aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
```

## 6 Closing 이후: 마지막 Cycle 닫힘

banner: (숨김)
panel state=executing

```text
    <span> 승인된 계획
    <span>  · 계획 v1
  <p cycle=closed> 현재 Cycle cycle:C3은 닫혔습니다. 지금 열린 Cycle은 없습니다. 마지막으로 연결된 항목: B — Question B ⏎ 확인 조건: File B ⏎ 결과는 그 Cycle Report가 진실원이며 여기서 성공으로 보지 않습니다.
  <p> 실행함 2개 · 지금 열린 Cycle 0개 · 남음 0개
    <li status=executed> 실행함 (cycle:C2) · A
    <li status=executed> 실행함 (cycle:C3) · B: Question B ⏎ 확인: File B
  <small> GIL에 기록된 사실입니다. “실행함”은 Cycle이 열렸다는 기록이며 성공 여부는 해당 Cycle Report가 진실원입니다. 에이전트가 지금 작업 중인지는 여기서 알 수 없습니다.
```

## 7 옛 Core: current_cycle 없음

banner: (숨김)
panel state=executing

```text
    <span> 승인된 계획
    <span>  · 계획 v1
  <p cycle=unknown> 현재 Cycle에 연결된 항목: A — Question A ⏎ 확인 조건: File A ⏎ 이 서버 응답에는 Cycle 열림 여부가 없어 열려 있다고 보지 않습니다.
  <p> 실행함 0개 · 연결됨(열림 미확인) 1개 · 남음 1개
    <li status=bound> 현재 Cycle에 연결됨 (열림 여부 미확인) · A: Question A ⏎ 확인: File A
    <li status=pending> 남음 · B: Question B ⏎ 확인: File B
  <small> GIL에 기록된 사실입니다. “실행함”은 Cycle이 열렸다는 기록이며 성공 여부는 해당 Cycle Report가 진실원입니다. 에이전트가 지금 작업 중인지는 여기서 알 수 없습니다.
```
