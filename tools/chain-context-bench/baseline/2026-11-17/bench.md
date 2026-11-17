# Chain context bench — complete

개발용 측정 기록이다. 크기는 바이트·코드 포인트이며 **토큰 수가 아니다.** 모델을 호출하지 않았다.

## 실행 조건

| 항목 | 값 |
| --- | --- |
| schema / tool | gil-chain-context-bench/0.1 / 0.1.0 |
| source commit | bcefbc42ab6909c9e5c59587d79aa37f4e9fbb10 · dirty 예 (10 entries) |
| Core binary | <main-checkout>/target/claude-wave-2/targets/bb2ebeab1e2819b607b19a38/debug/gil · sha256 eda298b6e49f2645cd1715812495ff41f6a4539e8c9a5b89bbf21659acfd7874 |
| binary 출처 | built by this run with cargo --features (build queue target) |
| features (선언) | approved-plan,visual-authoring |
| 관측 | tools 31 · plan 도구 예 · revisit 도구 예 |
| locale | ko-KR |
| 읽기 호출 | gil_context, gil_status, gil_plan_read |
| 측정 방식 | generation in one Core process, reads in a second fresh Core process (new session); isolated HOME/LOCALAPPDATA/settings per run |
| base | chains=0 cycles=0 failures=0 steps=2 report_lines=1 plan_items=1 |
| 반복 / 사례 | 2 / 13 |
| 예상·상한 호출 | 1038 / 3000 |
| 환경 | darwin-arm64 · node v26.5.0 |

## 축: chains — 이전 Chain 수 (base 0)

| level | case | 상태 | 생성 호출 | .gil B | context text B | context 문자 | context structured B | said⊂text | gil_status text B | gil_plan_read text B | 읽기 text 합 B | 반복 동일 | 읽기 전후 기록 동일 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | case-01 | complete | 21 | 11362 | 6230 | 3606 | 5246 | 예 | 1476 | 3350 | 11056 | 예 | 예 |
| 1 | case-02 | complete | 63 | 30461 | 6403 | 3720 | 5423 | 예 | 1478 | 3350 | 11231 | 예 | 예 |
| 2 | case-03 | complete | 105 | 49623 | 6574 | 3832 | 5598 | 예 | 1478 | 3350 | 11402 | 예 | 예 |

| 구간 | context text Δ B | Δ/단위 | structured Δ B | 읽기 text 합 Δ B | 늘어난 context 절 (B) |
| --- | --- | --- | --- | --- | --- |
| 0→1 | 173 | 173 | 177 | 175 | chain_previous +170, will +2 |
| 1→2 | 171 | 171 | 175 | 171 | chain_previous +170 |

| 해상도 점검 | level 0 | level 1 | level 2 |
| --- | --- | --- | --- |
| prev_cycle_projection | 맞음 | 맞음 | 맞음 |
| prev_cycle_inner | 맞음 | 맞음 | 맞음 |
| current_step | 맞음 | 맞음 | 맞음 |
| plan_current_item | 맞음 | 맞음 | 맞음 |
| plan_constraint | 맞음 | 맞음 | 맞음 |
| prev_chain_report | — | 맞음 | 맞음 |
| prev_chain_inner | — | 맞음 | 맞음 |

gil_context 단락 중 이 축의 모든 level에 똑같이 있는 것: 15개 (4101 B). level별 고정/변동: L0=4101/2040, L1=4101/2211, L2=4101/2380 B.

반복 근거(gil_context 한 응답 안): L0: 단락 23, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B) · L1: 단락 24, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B) · L2: 단락 25, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 2개 (338 B).

읽기 호출 사이에 똑같이 반복된 단락(context-metrics 반복 후보, 첫 등장 제외): L0=329 B, L1=329 B, L2=329 B. 40자 이상 같은 줄의 반복분(첫 등장 제외, 그중 다른 호출 사이): L0=947(947) B, L1=947(947) B, L2=947(947) B.

## 축: cycles — 현재 Chain의 닫힌 Experiment Cycle 수 (base 0)

| level | case | 상태 | 생성 호출 | .gil B | context text B | context 문자 | context structured B | said⊂text | gil_status text B | gil_plan_read text B | 읽기 text 합 B | 반복 동일 | 읽기 전후 기록 동일 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | case-01 | complete | 21 | 11362 | 6230 | 3606 | 5246 | 예 | 1476 | 3350 | 11056 | 예 | 예 |
| 1 | case-04 | complete | 34 | 18812 | 6687 | 3917 | 5731 | 예 | 1478 | 3733 | 11898 | 예 | 예 |
| 2 | case-05 | complete | 47 | 25447 | 7142 | 4226 | 6214 | 예 | 1478 | 4116 | 12736 | 예 | 예 |

| 구간 | context text Δ B | Δ/단위 | structured Δ B | 읽기 text 합 Δ B | 늘어난 context 절 (B) |
| --- | --- | --- | --- | --- | --- |
| 0→1 | 457 | 457 | 485 | 842 | previous_cycles +402, plan +53, will +2 |
| 1→2 | 455 | 455 | 483 | 838 | previous_cycles +402, plan +53 |

| 해상도 점검 | level 0 | level 1 | level 2 |
| --- | --- | --- | --- |
| prev_cycle_projection | 맞음 | 맞음 | 맞음 |
| prev_cycle_inner | 맞음 | 맞음 | 맞음 |
| current_step | 맞음 | 맞음 | 맞음 |
| plan_current_item | 맞음 | 맞음 | 맞음 |
| plan_constraint | 맞음 | 맞음 | 맞음 |

gil_context 단락 중 이 축의 모든 level에 똑같이 있는 것: 13개 (3334 B). level별 고정/변동: L0=3334/2807, L1=3334/3218, L2=3334/3573 B.

반복 근거(gil_context 한 응답 안): L0: 단락 23, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B) · L1: 단락 29, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B) · L2: 단락 35, 같은 응답 안 동일 단락 반복 +1 (54 B), 같은 틀 단락 12개 (712 B).

읽기 호출 사이에 똑같이 반복된 단락(context-metrics 반복 후보, 첫 등장 제외): L0=329 B, L1=329 B, L2=329 B. 40자 이상 같은 줄의 반복분(첫 등장 제외, 그중 다른 호출 사이): L0=947(947) B, L1=947(947) B, L2=947(947) B.

## 축: failures — 같은 계보의 누적 실패 Cycle 수 (base 0)

| level | case | 상태 | 생성 호출 | .gil B | context text B | context 문자 | context structured B | said⊂text | gil_status text B | gil_plan_read text B | 읽기 text 합 B | 반복 동일 | 읽기 전후 기록 동일 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | case-01 | complete | 21 | 11362 | 6230 | 3606 | 5246 | 예 | 1476 | 3350 | 11056 | 예 | 예 |
| 1 | case-06 | complete | 35 | 18527 | 7155 | 4156 | 6205 | 예 | 1531 | 3403 | 12089 | 예 | 예 |
| 2 | case-07 | complete | 49 | 24860 | 7702 | 4521 | 6781 | 예 | 1531 | 3456 | 12689 | 예 | 예 |

| 구간 | context text Δ B | Δ/단위 | structured Δ B | 읽기 text 합 Δ B | 늘어난 context 절 (B) |
| --- | --- | --- | --- | --- | --- |
| 0→1 | 925 | 925 | 959 | 1033 | failed_cycles +869, plan +53, will +2 |
| 1→2 | 547 | 547 | 576 | 600 | failed_cycles +494, plan +53 |

| 해상도 점검 | level 0 | level 1 | level 2 |
| --- | --- | --- | --- |
| prev_cycle_projection | 맞음 | 맞음 | 맞음 |
| prev_cycle_inner | 맞음 | 맞음 | 맞음 |
| current_step | 맞음 | 맞음 | 맞음 |
| plan_current_item | 맞음 | 맞음 | 맞음 |
| plan_constraint | 맞음 | 맞음 | 맞음 |
| failure_projection | — | 맞음 | 맞음 |
| failure_inner | — | 맞음 | 맞음 |

gil_context 단락 중 이 축의 모든 level에 똑같이 있는 것: 16개 (4132 B). level별 고정/변동: L0=4132/2009, L1=4132/2884, L2=4132/3318 B.

반복 근거(gil_context 한 응답 안): L0: 단락 23, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B) · L1: 단락 30, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B) · L2: 단락 36, 같은 응답 안 동일 단락 반복 +1 (65 B), 같은 틀 단락 12개 (892 B).

읽기 호출 사이에 똑같이 반복된 단락(context-metrics 반복 후보, 첫 등장 제외): L0=329 B, L1=329 B, L2=329 B. 40자 이상 같은 줄의 반복분(첫 등장 제외, 그중 다른 호출 사이): L0=947(947) B, L1=947(947) B, L2=947(947) B.

## 축: steps — 현재 Cycle의 닫힌 Step 수 (base 2)

| level | case | 상태 | 생성 호출 | .gil B | context text B | context 문자 | context structured B | said⊂text | gil_status text B | gil_plan_read text B | 읽기 text 합 B | 반복 동일 | 읽기 전후 기록 동일 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | case-08 | complete | 17 | 9466 | 6188 | 3456 | 5194 | 예 | 1476 | 3861 | 11525 | 예 | 예 |
| 2 | case-01 | complete | 21 | 11362 | 6230 | 3606 | 5246 | 예 | 1476 | 3350 | 11056 | 예 | 예 |
| 4 | case-09 | complete | 25 | 14729 | 6991 | 4227 | 6032 | 예 | 1477 | 3182 | 11650 | 예 | 예 |

| 구간 | context text Δ B | Δ/단위 | structured Δ B | 읽기 text 합 Δ B | 늘어난 context 절 (B) |
| --- | --- | --- | --- | --- | --- |
| 0→2 | 42 | 21 | 52 | -469 | current_cycle +392, plan +170, rules -9, next_call -511 |
| 2→4 | 761 | 380.5 | 786 | 594 | rules +640, current_cycle +458, here +1, next_call -168, plan -170 |

| 해상도 점검 | level 0 | level 2 | level 4 |
| --- | --- | --- | --- |
| prev_cycle_projection | 맞음 | 맞음 | 맞음 |
| prev_cycle_inner | 맞음 | 맞음 | 맞음 |
| plan_current_item | 맞음 | 맞음 | 맞음 |
| plan_constraint | 맞음 | 맞음 | 맞음 |
| current_step | — | 맞음 | 맞음 |

gil_context 단락 중 이 축의 모든 level에 똑같이 있는 것: 14개 (3914 B). level별 고정/변동: L0=3914/2199, L2=3914/2227, L4=3914/2919 B.

반복 근거(gil_context 한 응답 안): L0: 단락 21, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B) · L2: 단락 23, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B) · L4: 단락 28, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B).

읽기 호출 사이에 똑같이 반복된 단락(context-metrics 반복 후보, 첫 등장 제외): L0=329 B, L2=329 B, L4=329 B. 40자 이상 같은 줄의 반복분(첫 등장 제외, 그중 다른 호출 사이): L0=947(947) B, L2=947(947) B, L4=947(947) B.

## 축: report_lines — 현재 Cycle Report 칸당 줄 수 (base 1)

| level | case | 상태 | 생성 호출 | .gil B | context text B | context 문자 | context structured B | said⊂text | gil_status text B | gil_plan_read text B | 읽기 text 합 B | 반복 동일 | 읽기 전후 기록 동일 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | case-01 | complete | 21 | 11362 | 6230 | 3606 | 5246 | 예 | 1476 | 3350 | 11056 | 예 | 예 |
| 4 | case-10 | complete | 21 | 12018 | 6784 | 3884 | 5808 | 예 | 1476 | 3350 | 11610 | 예 | 예 |
| 16 | case-11 | complete | 21 | 14548 | 9026 | 5022 | 8074 | 예 | 1476 | 3350 | 13852 | 예 | 예 |

| 구간 | context text Δ B | Δ/단위 | structured Δ B | 읽기 text 합 Δ B | 늘어난 context 절 (B) |
| --- | --- | --- | --- | --- | --- |
| 1→4 | 554 | 184.7 | 562 | 554 | current_cycle +554 |
| 4→16 | 2242 | 186.8 | 2266 | 2242 | current_cycle +2242 |

| 해상도 점검 | level 1 | level 4 | level 16 |
| --- | --- | --- | --- |
| prev_cycle_projection | 맞음 | 맞음 | 맞음 |
| prev_cycle_inner | 맞음 | 맞음 | 맞음 |
| current_step | 맞음 | 맞음 | 맞음 |
| plan_current_item | 맞음 | 맞음 | 맞음 |
| plan_constraint | 맞음 | 맞음 | 맞음 |

gil_context 단락 중 이 축의 모든 level에 똑같이 있는 것: 20개 (5434 B). level별 고정/변동: L1=5434/707, L4=5434/1229, L16=5434/3375 B.

반복 근거(gil_context 한 응답 안): L1: 단락 23, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B) · L4: 단락 23, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B) · L16: 단락 23, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B).

읽기 호출 사이에 똑같이 반복된 단락(context-metrics 반복 후보, 첫 등장 제외): L1=329 B, L4=329 B, L16=329 B. 40자 이상 같은 줄의 반복분(첫 등장 제외, 그중 다른 호출 사이): L1=947(947) B, L4=947(947) B, L16=947(947) B.

## 축: plan_items — 승인 계획의 미실행 항목 수(현재 항목 포함) (base 1)

| level | case | 상태 | 생성 호출 | .gil B | context text B | context 문자 | context structured B | said⊂text | gil_status text B | gil_plan_read text B | 읽기 text 합 B | 반복 동일 | 읽기 전후 기록 동일 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | case-01 | complete | 21 | 11362 | 6230 | 3606 | 5246 | 예 | 1476 | 3350 | 11056 | 예 | 예 |
| 2 | case-12 | complete | 21 | 11596 | 6232 | 3608 | 5248 | 예 | 1476 | 3816 | 11524 | 예 | 예 |
| 4 | case-13 | complete | 21 | 11992 | 6240 | 3616 | 5256 | 예 | 1476 | 4750 | 12466 | 예 | 예 |

| 구간 | context text Δ B | Δ/단위 | structured Δ B | 읽기 text 합 Δ B | 늘어난 context 절 (B) |
| --- | --- | --- | --- | --- | --- |
| 1→2 | 2 | 2 | 2 | 468 | plan +2 |
| 2→4 | 8 | 4 | 8 | 942 | plan +8 |

| 해상도 점검 | level 1 | level 2 | level 4 |
| --- | --- | --- | --- |
| prev_cycle_projection | 맞음 | 맞음 | 맞음 |
| prev_cycle_inner | 맞음 | 맞음 | 맞음 |
| current_step | 맞음 | 맞음 | 맞음 |
| plan_current_item | 맞음 | 맞음 | 맞음 |
| plan_constraint | 맞음 | 맞음 | 맞음 |
| plan_pending_item | — | 맞음 | 맞음 |

gil_context 단락 중 이 축의 모든 level에 똑같이 있는 것: 21개 (5121 B). level별 고정/변동: L1=5121/1020, L2=5121/1023, L4=5121/1031 B.

반복 근거(gil_context 한 응답 안): L1: 단락 23, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B) · L2: 단락 23, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B) · L4: 단락 23, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B).

읽기 호출 사이에 똑같이 반복된 단락(context-metrics 반복 후보, 첫 등장 제외): L1=329 B, L2=329 B, L4=329 B. 40자 이상 같은 줄의 반복분(첫 등장 제외, 그중 다른 호출 사이): L1=947(947) B, L2=947(947) B, L4=947(947) B.

## 모든 사례에 공통인 gil_context 단락

13개 사례 모두에 똑같이 있는 단락 10개, 2921 B. 상태와 무관하게 매 context에 실리는 고정 안내 후보다(필요성 판단은 하지 않는다).

## gil_context 절별 크기 변화 (전체 사례)

| 절 | 나타난 사례 | 최소 B | 최대 B | 모든 사례에 같은 크기 |
| --- | --- | --- | --- | --- |
| text_part_1 | 13 | 329 | 329 | 예 |
| text_part_2 | 13 | 934 | 934 | 예 |
| preamble | 13 | 358 | 358 | 예 |
| next_call | 13 | 161 | 840 | 아니오 |
| file_scope | 13 | 720 | 720 | 예 |
| plan | 13 | 523 | 799 | 아니오 |
| chain_current | 13 | 376 | 376 | 예 |
| previous_cycles | 13 | 701 | 1505 | 아니오 |
| current_cycle | 13 | 226 | 3414 | 아니오 |
| here | 13 | 156 | 157 | 아니오 |
| will | 13 | 235 | 237 | 아니오 |
| next_moves | 13 | 108 | 108 | 예 |
| rules | 13 | 69 | 709 | 아니오 |
| report_principles | 13 | 477 | 477 | 예 |
| resume | 13 | 113 | 113 | 예 |
| chain_previous | 2 | 170 | 340 | 아니오 |
| failed_cycles | 2 | 869 | 1363 | 아니오 |

## 기록 계층별 도착 위치 (전체 사례, 태그 수)

| 계층 | context 기대 | 사례 | 태그 | gil_context text | gil_status text | gil_plan_read text | 판정 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| prev_cycle_projection | present | 13 | 35 | 35 | 0 | 0 | 맞음 13 |
| prev_cycle_inner | absent | 13 | 54 | 0 | 0 | 16 | 맞음 13 |
| current_step | present | 12 | 38 | 38 | 0 | 0 | 맞음 12 |
| plan_current_item | present | 13 | 13 | 13 | 0 | 13 | 맞음 13 |
| plan_constraint | observe | 13 | 13 | 0 | 0 | 13 | 맞음 13 |
| prev_chain_report | present | 2 | 3 | 3 | 0 | 0 | 맞음 2 |
| prev_chain_inner | absent | 2 | 57 | 0 | 0 | 0 | 맞음 2 |
| failure_projection | present | 2 | 12 | 12 | 0 | 0 | 맞음 2 |
| failure_inner | absent | 2 | 12 | 0 | 0 | 0 | 맞음 2 |
| plan_pending_item | observe | 2 | 4 | 0 | 0 | 4 | 맞음 2 |

## gil_context text 증가가 큰 구간 (단위당 바이트)

| 축 | 구간 | Δ/단위 B | Δ B |
| --- | --- | --- | --- |
| failures | 0→1 | 925 | 925 |
| failures | 1→2 | 547 | 547 |
| cycles | 0→1 | 457 | 457 |
| cycles | 1→2 | 455 | 455 |
| steps | 2→4 | 380.5 | 761 |
| report_lines | 4→16 | 186.8 | 2242 |
| report_lines | 1→4 | 184.7 | 554 |
| chains | 0→1 | 173 | 173 |

## 재현성

| case | 비교 실행 | 모두 동일 | tool별 text B |
| --- | --- | --- | --- |
| case-01 | 2 | 예 | gil_context 6230/6230 · gil_status 1476/1476 · gil_plan_read 3350/3350 |
| case-02 | 2 | 예 | gil_context 6403/6403 · gil_status 1478/1478 · gil_plan_read 3350/3350 |
| case-03 | 2 | 예 | gil_context 6574/6574 · gil_status 1478/1478 · gil_plan_read 3350/3350 |
| case-04 | 2 | 예 | gil_context 6687/6687 · gil_status 1478/1478 · gil_plan_read 3733/3733 |
| case-05 | 2 | 예 | gil_context 7142/7142 · gil_status 1478/1478 · gil_plan_read 4116/4116 |
| case-06 | 2 | 예 | gil_context 7155/7155 · gil_status 1531/1531 · gil_plan_read 3403/3403 |
| case-07 | 2 | 예 | gil_context 7702/7702 · gil_status 1531/1531 · gil_plan_read 3456/3456 |
| case-08 | 2 | 예 | gil_context 6188/6188 · gil_status 1476/1476 · gil_plan_read 3861/3861 |
| case-09 | 2 | 예 | gil_context 6991/6991 · gil_status 1477/1477 · gil_plan_read 3182/3182 |
| case-10 | 2 | 예 | gil_context 6784/6784 · gil_status 1476/1476 · gil_plan_read 3350/3350 |
| case-11 | 2 | 예 | gil_context 9026/9026 · gil_status 1476/1476 · gil_plan_read 3350/3350 |
| case-12 | 2 | 예 | gil_context 6232/6232 · gil_status 1476/1476 · gil_plan_read 3816/3816 |
| case-13 | 2 | 예 | gil_context 6240/6240 · gil_status 1476/1476 · gil_plan_read 4750/4750 |

## 중단·생성 실패·측정 누락

없음.

## 측정 한계

- 크기는 UTF-8 바이트·유니코드 코드 포인트("문자")·UTF-16 단위다(tools/context-metrics). 토큰 수가 아니며 토큰 계산기를 쓰지 않았다.
- text 응답과 structuredContent는 서로 다른 표현이며 더하지 않는다. Host가 어느 표현을 모델 입력에 넣는지는 이 도구가 관측하지 않는다.
- "읽기 호출 text 합"은 이 벤치가 고른 읽기 호출의 text 크기 합일 뿐 모델 입력량·전체 실행 비용이 아니다. 모델 호출이 없으므로 재조회·재시도·작업 정확성은 측정하지 않았다.
- 경로는 <project_root> 등 표식으로 가린 기록을 측정한다. 가리기 전 text 크기는 raw_text_utf8_bytes에 따로 적었다(임시 경로 길이에 따라 실행마다 다를 수 있다).
- 합성 기록이다. 문장 길이·항목 수가 실제 사용자 작업과 다르므로 절대값보다 축별 증가 모양을 본다. 한 번의 측정으로 임계값이나 CI 실패 기준을 만들지 않는다.
- 승인 계획 결정은 시험 harness가 App 경로로 보낸 합성 입력이다. 사람의 승인·화면 표시의 증거가 아니다.
