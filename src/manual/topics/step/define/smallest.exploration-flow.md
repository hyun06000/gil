---
id: step/define/smallest
aliases:
  - define
title: 지금 실행 가능한 가장 작은 문제를 정의한다
summary: Define 은 Chain 목표나 제안서를 복사하지 않고, 이 Experiment 가 연결된 승인 요구(탐색 계약) 또는 묶인 항목(항목 큐 계약)을 근거로 지금 수행하고 판정할 수 있는 최소 문제 하나를 적는다.
offered: false
related:
  - cycle/interview/clarify
  - action/open-contract
  - current
---

[언제 읽는가]
  Experiment 를 열어 Define 의 problem 과 success_condition 을 적을 때.
  한 실험이 너무 많은 일을 하거나, 반대로 가치 없는 준비 동작까지 쪼개질 것 같을 때.

[어느 계약인가]
  실행 계약은 Chain 을 열 때 기록되고 바뀌지 않는다. gil_plan_read 로 확인한다.
      contract: exploration  탐색 계약 — Experiment 는 열 때 승인 요구 하나에 연결된다
      (contract 칸 없음)     항목 큐 계약 — Experiment 는 열 때 첫 미실행 항목에 자동으로 묶인다
  이 Core 가 새로 여는 Chain 은 탐색 계약이고, 그 전에 열린 Chain 은 항목 큐 계약 그대로다.
  한 계약의 규칙으로 다른 계약을 흉내 내지 않는다. 과거 승인·완료 기록을 바꾸지 않는다.

[지금 할 일]
      problem            해결할 질문과 범위 — 확보한 입력, 첫 행동, 멈출 지점이 보이게
      success_condition  같은 문제의 관측 가능한 판정 조건 — 예/아니오를 구분하는 기준이며 조사 완료 자체가 아님
  탐색 계약 — gil_plan_read 의 current_experiment(연결된 요구와 선택 근거)와 인터뷰 기록을 읽는다.
  그 요구에 기여하며 지금 수행하고 판정할 수 있는 가장 작은 문제 하나를 적는다. 요구 전체를 복사하지 않는다.
  다음 문제는 이번 관측에서 정한다. 같은 요구에 여러 작은 Define 이 차례로 기여할 수 있다.
  앞선 실험의 근거는 원본에서 읽는다 — gil_plan_read 의 experiments[].cycle_ref 를 gil_report_read 로 열어 Step 주소를 받고,
  필요한 Report 원문 하나씩과 gil_evidence_read 의 저장 증거(그 Verify 의 Snapshot)를 본다. 되돌아간 실패 실험도 같다.
  항목 큐 계약 — 묶인 항목(current_item)과 같은 문제·완료 범위 안에서 문제 하나를 적는다.
  작업 방법의 가설은 Hypothesis 에, 실제 실행은 Verify 에 둔다.

[불변식]
  problem 은 반드시 예/아니오로 답할 단일 명제를 물음으로 적는다. 「살펴본다」「확인한다」는
  활동 설명을 문제로 쓰지 않는다. 판정 기준을 미리 고정하고 결과를 본 뒤 질문을 뒤집지 않는다.
  답할 근거가 부족한 상태를 「아니오」로 단정하거나 조사 완료를 명제 충족으로 세지 않는다.
  하나의 구체적인 질문만 갖는다. 필요한 입력·선행 결과·권한이 지금 있거나, 그 확인이 이번 문제다.
  더 나눠도 독립적인 검증 결과가 생기지 않는 준비 동작은 별도 Cycle 로 만들지 않는다.
  「가장 작다」는 문장 길이·파일 수·실행 시간이 아니다. 좁히는 추론이 새 목표·권한을 더하지 않는다.
  탐색 계약 — 실험 하나의 성공은 요구의 완료가 아니다. 요구별 답은 Closing Interview 에서 사용자가 승인한다.
  요구별 「현재 답」·잠정 정리를 따로 쌓아 다음 Define 의 전제로 삼지 않는다. 기록에 있다는 것은 참이라는 판정이 아니다.
  충족되지 않으면 failure 로 닫고 되돌아간 뒤(cycle/revisit) 다른 작은 질문을 연다.
  세분화·방법·순서 변경으로 계획을 다시 제안하지 않는다. 요구·범위·권한·판단 기준 자체의 변경은
  Define 이 아니라 Interview 에서 합의한다. Core 는 연결의 참조만 검사하고 기여 여부의 뜻은 검사하지 않는다.
  항목 큐 계약 — 항목과 Define 은 같은 문제를 가리킨다. 일부만 수행하고 항목 전체를 완료 처리하지 않는다.
  승인 항목의 분할·대체는 이 계약에 없다. 막힘을 그대로 알린다(gil_plan_request_interview 가 요청을 보존한다).

[올바른 예]
  탐색 계약 — 요구 R1 「제공된 매출 표로 월별 추이를 믿고 읽을 수 있는가」에 연결된 첫 Experiment.
      problem: sales.csv 의 날짜 열에 허용 형식(YYYY-MM-DD)을 벗어나는 값이 존재하는가?
      success_condition: sales.csv 의 날짜 열에서 YYYY-MM-DD 형식을 벗어난 값이 하나 이상 존재하고, 해당 값과 행 번호가 증거에 기록된다
  성공해도 R1 은 완료가 아니다. 관측(형식 이탈 3건)에 따라 다음 Experiment 를 같은 R1 에 연결한다.
      problem: 형식을 벗어난 3건을 정보 손실 없이 YYYY-MM-DD 로 변환할 수 있는가?
  항목 큐 계약 — 항목 A 「제공된 표의 날짜·금액 열이 월별 집계에 쓸 수 있는가」에 묶인 Experiment.
      problem: 제공된 표 sales.csv 의 날짜·금액 열이 월별 집계에 쓸 수 있는가?

[흔한 실패]
  problem 에 Chain 목표나 승인 요구의 문장을 그대로 적는다 — 한 실험으로 판정할 수 없는 크기가 된다.
  (탐색 계약) 작은 실험 하나가 성공했다고 요구가 끝났다고 보고한다.
  (탐색 계약) 요구에 없던 목표를 「작은 Define」이라며 연다 — 연결은 승인된 요구만 가리킬 수 있다.
  (항목 큐 계약) 승인 항목의 일부만 작게 정의해 수행한 뒤 항목 전체를 완료로 적는다.
  검증할 결과가 없는 준비를 실험으로 쪼개거나, success_condition 을 판정할 수 없게 적는다.

[관련 주제]
  cycle/interview/clarify
  action/open-contract
  current
