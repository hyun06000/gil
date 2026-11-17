# Chain context bench — complete

개발용 측정 기록이다. 크기는 바이트·코드 포인트이며 **토큰 수가 아니다.** 모델을 호출하지 않았다.

## 실행 조건

| 항목 | 값 |
| --- | --- |
| schema / tool | gil-chain-context-bench/0.1 / 0.2.0 |
| source commit | dceaac8eab9c115ed57b0c3f31de66fbb07771e5 · dirty 아니오 (0 entries) |
| Core binary | <worktree>/target/ccb-product/debug/gil · sha256 452499255f5f451bdbd2fb27b3686f7dc5dda9ac4b169b77c99f71af12bf545f |
| binary 출처 | built by this run with cargo --features |
| features (선언) | approved-plan,product-safety,visual-authoring |
| 관측 | tools 33 · plan 도구 예 · revisit 도구 예 · 승인 복구 도구(gil_revisit_review·gil_revisit_apply) 예 |
| 실패 뒤 복구 경로 | Core가 내놓은 경로: App 승인 복구 (gil_revisit_review → gil_revisit_apply) · 실행한 복구 6회 · harness 합성 승인 6회 · 사람의 승인 0회 |
| locale | ko-KR |
| 읽기 호출 | gil_context, gil_status, gil_plan_read |
| 측정 방식 | generation in one Core process, reads in a second fresh Core process (new session); isolated HOME/LOCALAPPDATA/settings per run |
| base | chains=0 cycles=0 failures=0 steps=2 report_lines=1 plan_items=1 |
| 반복 / 사례 | 2 / 13 |
| 예상·상한 호출 | 1044 / 3000 |
| 환경 | darwin-arm64 · node v26.5.0 |

## 축: chains — 이전 Chain 수 (base 0)

| level | case | 상태 | 생성 호출 | .gil B | context text B | context 문자 | context structured B | said⊂text | gil_status text B | gil_plan_read text B | 읽기 text 합 B | 반복 동일 | 읽기 전후 기록 동일 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | case-01 | complete | 21 | 11362 | 6830 | 3866 | 5930 | 예 | 1476 | 3652 | 11958 | 예 | 예 |
| 1 | case-02 | complete | 63 | 30461 | 7003 | 3980 | 6107 | 예 | 1478 | 3652 | 12133 | 예 | 예 |
| 2 | case-03 | complete | 105 | 49623 | 7174 | 4092 | 6282 | 예 | 1478 | 3652 | 12304 | 예 | 예 |

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

gil_context 단락 중 이 축의 모든 level에 똑같이 있는 것: 15개 (4063 B). level별 고정/변동: L0=4063/2676, L1=4063/2847, L2=4063/3016 B.

반복 근거(gil_context 한 응답 안): L0: 단락 24, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B) · L1: 단락 25, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B) · L2: 단락 26, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 2개 (338 B).

읽기 호출 사이에 똑같이 반복된 단락(context-metrics 반복 후보, 첫 등장 제외): L0=329 B, L1=329 B, L2=329 B. 40자 이상 같은 줄의 반복분(첫 등장 제외, 그중 다른 호출 사이): L0=1172(1172) B, L1=1172(1172) B, L2=1172(1172) B.

## 축: cycles — 현재 Chain의 닫힌 Experiment Cycle 수 (base 0)

| level | case | 상태 | 생성 호출 | .gil B | context text B | context 문자 | context structured B | said⊂text | gil_status text B | gil_plan_read text B | 읽기 text 합 B | 반복 동일 | 읽기 전후 기록 동일 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | case-01 | complete | 21 | 11362 | 6830 | 3866 | 5930 | 예 | 1476 | 3652 | 11958 | 예 | 예 |
| 1 | case-04 | complete | 34 | 18812 | 7287 | 4177 | 6415 | 예 | 1478 | 4035 | 12800 | 예 | 예 |
| 2 | case-05 | complete | 47 | 25447 | 7742 | 4486 | 6898 | 예 | 1478 | 4418 | 13638 | 예 | 예 |

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

gil_context 단락 중 이 축의 모든 level에 똑같이 있는 것: 13개 (3296 B). level별 고정/변동: L0=3296/3443, L1=3296/3854, L2=3296/4209 B.

반복 근거(gil_context 한 응답 안): L0: 단락 24, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B) · L1: 단락 30, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B) · L2: 단락 36, 같은 응답 안 동일 단락 반복 +1 (54 B), 같은 틀 단락 12개 (712 B).

읽기 호출 사이에 똑같이 반복된 단락(context-metrics 반복 후보, 첫 등장 제외): L0=329 B, L1=329 B, L2=329 B. 40자 이상 같은 줄의 반복분(첫 등장 제외, 그중 다른 호출 사이): L0=1172(1172) B, L1=1172(1172) B, L2=1172(1172) B.

## 축: failures — 같은 계보의 누적 실패 Cycle 수 (base 0)

| level | case | 상태 | 생성 호출 | .gil B | context text B | context 문자 | context structured B | said⊂text | gil_status text B | gil_plan_read text B | 읽기 text 합 B | 반복 동일 | 읽기 전후 기록 동일 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | case-01 | complete | 21 | 11362 | 6830 | 3866 | 5930 | 예 | 1476 | 3652 | 11958 | 예 | 예 |
| 1 | case-06 | complete | 36 | 24876 | 7755 | 4416 | 6889 | 예 | 1531 | 3705 | 12991 | 예 | 예 |
| 2 | case-07 | complete | 51 | 37558 | 8302 | 4781 | 7465 | 예 | 1531 | 3758 | 13591 | 예 | 예 |

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

gil_context 단락 중 이 축의 모든 level에 똑같이 있는 것: 16개 (4094 B). level별 고정/변동: L0=4094/2645, L1=4094/3520, L2=4094/3954 B.

반복 근거(gil_context 한 응답 안): L0: 단락 24, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B) · L1: 단락 31, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B) · L2: 단락 37, 같은 응답 안 동일 단락 반복 +1 (65 B), 같은 틀 단락 12개 (892 B).

읽기 호출 사이에 똑같이 반복된 단락(context-metrics 반복 후보, 첫 등장 제외): L0=329 B, L1=329 B, L2=329 B. 40자 이상 같은 줄의 반복분(첫 등장 제외, 그중 다른 호출 사이): L0=1172(1172) B, L1=1172(1172) B, L2=1172(1172) B.

## 축: steps — 현재 Cycle의 닫힌 Step 수 (base 2)

| level | case | 상태 | 생성 호출 | .gil B | context text B | context 문자 | context structured B | said⊂text | gil_status text B | gil_plan_read text B | 읽기 text 합 B | 반복 동일 | 읽기 전후 기록 동일 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 0 | case-08 | complete | 17 | 9466 | 6726 | 3686 | 5816 | 예 | 1476 | 4674 | 12876 | 예 | 예 |
| 2 | case-01 | complete | 21 | 11362 | 6830 | 3866 | 5930 | 예 | 1476 | 3652 | 11958 | 예 | 예 |
| 4 | case-09 | complete | 25 | 14729 | 7573 | 4477 | 6698 | 예 | 1477 | 3316 | 12366 | 예 | 예 |

| 구간 | context text Δ B | Δ/단위 | structured Δ B | 읽기 text 합 Δ B | 늘어난 context 절 (B) |
| --- | --- | --- | --- | --- | --- |
| 0→2 | 104 | 52 | 114 | -918 | current_cycle +392, plan +170, preamble +62, rules -9, next_call -511 |
| 2→4 | 743 | 371.5 | 768 | 408 | rules +640, current_cycle +458, here +1, preamble -18, next_call -168, plan -170 |

| 해상도 점검 | level 0 | level 2 | level 4 |
| --- | --- | --- | --- |
| prev_cycle_projection | 맞음 | 맞음 | 맞음 |
| prev_cycle_inner | 맞음 | 맞음 | 맞음 |
| plan_current_item | 맞음 | 맞음 | 맞음 |
| plan_constraint | 맞음 | 맞음 | 맞음 |
| current_step | — | 맞음 | 맞음 |

gil_context 단락 중 이 축의 모든 level에 똑같이 있는 것: 14개 (3876 B). level별 고정/변동: L0=3876/2773, L2=3876/2863, L4=3876/3537 B.

반복 근거(gil_context 한 응답 안): L0: 단락 22, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B) · L2: 단락 24, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B) · L4: 단락 29, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B).

읽기 호출 사이에 똑같이 반복된 단락(context-metrics 반복 후보, 첫 등장 제외): L0=329 B, L2=329 B, L4=329 B. 40자 이상 같은 줄의 반복분(첫 등장 제외, 그중 다른 호출 사이): L0=1683(1683) B, L2=1172(1172) B, L4=1004(1004) B.

## 축: report_lines — 현재 Cycle Report 칸당 줄 수 (base 1)

| level | case | 상태 | 생성 호출 | .gil B | context text B | context 문자 | context structured B | said⊂text | gil_status text B | gil_plan_read text B | 읽기 text 합 B | 반복 동일 | 읽기 전후 기록 동일 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | case-01 | complete | 21 | 11362 | 6830 | 3866 | 5930 | 예 | 1476 | 3652 | 11958 | 예 | 예 |
| 4 | case-10 | complete | 21 | 12018 | 7633 | 4261 | 6741 | 예 | 1476 | 3652 | 12761 | 예 | 예 |
| 16 | case-11 | complete | 21 | 14548 | 9869 | 5399 | 9001 | 예 | 1476 | 3652 | 14997 | 예 | 예 |

| 구간 | context text Δ B | Δ/단위 | structured Δ B | 읽기 text 합 Δ B | 늘어난 context 절 (B) |
| --- | --- | --- | --- | --- | --- |
| 1→4 | 803 | 267.7 | 811 | 803 | current_cycle +554, preamble +249 |
| 4→16 | 2236 | 186.3 | 2260 | 2236 | current_cycle +2242, preamble -6 |

| 해상도 점검 | level 1 | level 4 | level 16 |
| --- | --- | --- | --- |
| prev_cycle_projection | 맞음 | 맞음 | 맞음 |
| prev_cycle_inner | 맞음 | 맞음 | 맞음 |
| current_step | 맞음 | 맞음 | 맞음 |
| plan_current_item | 맞음 | 맞음 | 맞음 |
| plan_constraint | 맞음 | 맞음 | 맞음 |

gil_context 단락 중 이 축의 모든 level에 똑같이 있는 것: 20개 (5330 B). level별 고정/변동: L1=5330/1409, L4=5330/2180, L16=5330/4320 B.

반복 근거(gil_context 한 응답 안): L1: 단락 24, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B) · L4: 단락 24, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B) · L16: 단락 24, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B).

읽기 호출 사이에 똑같이 반복된 단락(context-metrics 반복 후보, 첫 등장 제외): L1=329 B, L4=329 B, L16=329 B. 40자 이상 같은 줄의 반복분(첫 등장 제외, 그중 다른 호출 사이): L1=1172(1172) B, L4=1172(1172) B, L16=1172(1172) B.

## 축: plan_items — 승인 계획의 미실행 항목 수(현재 항목 포함) (base 1)

| level | case | 상태 | 생성 호출 | .gil B | context text B | context 문자 | context structured B | said⊂text | gil_status text B | gil_plan_read text B | 읽기 text 합 B | 반복 동일 | 읽기 전후 기록 동일 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| 1 | case-01 | complete | 21 | 11362 | 6830 | 3866 | 5930 | 예 | 1476 | 3652 | 11958 | 예 | 예 |
| 2 | case-12 | complete | 21 | 11596 | 6832 | 3868 | 5932 | 예 | 1476 | 4118 | 12426 | 예 | 예 |
| 4 | case-13 | complete | 21 | 11992 | 6840 | 3876 | 5940 | 예 | 1476 | 5052 | 13368 | 예 | 예 |

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

gil_context 단락 중 이 축의 모든 level에 똑같이 있는 것: 22개 (5785 B). level별 고정/변동: L1=5785/954, L2=5785/957, L4=5785/965 B.

반복 근거(gil_context 한 응답 안): L1: 단락 24, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B) · L2: 단락 24, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B) · L4: 단락 24, 같은 응답 안 동일 단락 반복 +0 (0 B), 같은 틀 단락 0개 (0 B).

읽기 호출 사이에 똑같이 반복된 단락(context-metrics 반복 후보, 첫 등장 제외): L1=329 B, L2=329 B, L4=329 B. 40자 이상 같은 줄의 반복분(첫 등장 제외, 그중 다른 호출 사이): L1=1172(1172) B, L2=1172(1172) B, L4=1172(1172) B.

## 모든 사례에 공통인 gil_context 단락

13개 사례 모두에 똑같이 있는 단락 10개, 2883 B. 상태와 무관하게 매 context에 실리는 고정 안내 후보다(필요성 판단은 하지 않는다).

## gil_context 절별 크기 변화 (전체 사례)

| 절 | 나타난 사례 | 최소 B | 최대 B | 모든 사례에 같은 크기 |
| --- | --- | --- | --- | --- |
| text_part_1 | 13 | 329 | 329 | 예 |
| text_part_2 | 13 | 857 | 857 | 예 |
| preamble | 13 | 1000 | 1311 | 아니오 |
| next_call | 13 | 161 | 840 | 아니오 |
| file_scope | 13 | 720 | 720 | 예 |
| plan | 13 | 457 | 733 | 아니오 |
| chain_current | 13 | 376 | 376 | 예 |
| previous_cycles | 13 | 701 | 1505 | 아니오 |
| current_cycle | 13 | 226 | 3414 | 아니오 |
| here | 13 | 156 | 157 | 아니오 |
| will | 13 | 235 | 237 | 아니오 |
| next_moves | 13 | 147 | 147 | 예 |
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
| steps | 2→4 | 371.5 | 743 |
| report_lines | 1→4 | 267.7 | 803 |
| report_lines | 4→16 | 186.3 | 2236 |
| chains | 0→1 | 173 | 173 |

## 실패 뒤 복구와 보존 점검

새 Core 프로세스의 읽기 응답에서 본 것이다. 복구 승인은 harness의 합성 입력이며 사람이 누른 승인이 아니다. 관측/기대 순으로 적는다.

| case | 실패 | 복구 경로 | 합성 승인 | 실패 Cycle 참조 | 실패 표식 | 실패 절 | 현재 항목 | 실행 연결(재시도) | 연결이 context에 | 보존 폴더 | 보존 원본 | 모든 실행에서 유지 | 읽기 전후 동일 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| case-01 | 0 | 해당 없음 | 0 | 0/0 맞음 | 0/0 맞음 | 맞음 | 맞음 | 1/1 맞음 | 1/1 맞음 | — | — | 예 | 예 |
| case-02 | 0 | 해당 없음 | 0 | 0/0 맞음 | 0/0 맞음 | 맞음 | 맞음 | 1/1 맞음 | 1/1 맞음 | — | — | 예 | 예 |
| case-03 | 0 | 해당 없음 | 0 | 0/0 맞음 | 0/0 맞음 | 맞음 | 맞음 | 1/1 맞음 | 1/1 맞음 | — | — | 예 | 예 |
| case-04 | 0 | 해당 없음 | 0 | 0/0 맞음 | 0/0 맞음 | 맞음 | 맞음 | 1/1 맞음 | 1/1 맞음 | — | — | 예 | 예 |
| case-05 | 0 | 해당 없음 | 0 | 0/0 맞음 | 0/0 맞음 | 맞음 | 맞음 | 1/1 맞음 | 1/1 맞음 | — | — | 예 | 예 |
| case-06 | 1 | App 승인 | 1 | 1/1 맞음 | 4/4 맞음 | 맞음 | 맞음 | 2/2 맞음 | 2/2 맞음 | 1/1 | 4/4 | 예 | 예 |
| case-07 | 2 | App 승인 | 2 | 2/2 맞음 | 8/8 맞음 | 맞음 | 맞음 | 3/3 맞음 | 3/3 맞음 | 2/2 | 8/8 | 예 | 예 |
| case-08 | 0 | 해당 없음 | 0 | 0/0 맞음 | 0/0 맞음 | 맞음 | 맞음 | 1/1 맞음 | 1/1 맞음 | — | — | 예 | 예 |
| case-09 | 0 | 해당 없음 | 0 | 0/0 맞음 | 0/0 맞음 | 맞음 | 맞음 | 1/1 맞음 | 1/1 맞음 | — | — | 예 | 예 |
| case-10 | 0 | 해당 없음 | 0 | 0/0 맞음 | 0/0 맞음 | 맞음 | 맞음 | 1/1 맞음 | 1/1 맞음 | — | — | 예 | 예 |
| case-11 | 0 | 해당 없음 | 0 | 0/0 맞음 | 0/0 맞음 | 맞음 | 맞음 | 1/1 맞음 | 1/1 맞음 | — | — | 예 | 예 |
| case-12 | 0 | 해당 없음 | 0 | 0/0 맞음 | 0/0 맞음 | 맞음 | 맞음 | 1/1 맞음 | 1/1 맞음 | — | — | 예 | 예 |
| case-13 | 0 | 해당 없음 | 0 | 0/0 맞음 | 0/0 맞음 | 맞음 | 맞음 | 1/1 맞음 | 1/1 맞음 | — | — | 예 | 예 |

승인 복구의 보존본: 모든 실행에서 승인 기록·원본 목록·출처 기록이 있고 원본 bytes가 목록의 SHA-256과 같은 사례 2/2. 저장된 파일에서 승인 정보가 발견된 사례 0.

Core의 출처 기록: monitor_button 6. Core는 App 전용 도구로 들어온 승인을 monitor_button으로 적는다. 이 벤치에서는 그 입력이 harness에서 왔으므로 사람 클릭의 증거가 아니다(임시 프로젝트는 삭제됨).

보존 점검 위반: 0건.

## 재현성

| case | 비교 실행 | 모두 동일 | tool별 text B |
| --- | --- | --- | --- |
| case-01 | 2 | 예 | gil_context 6830/6830 · gil_status 1476/1476 · gil_plan_read 3652/3652 |
| case-02 | 2 | 예 | gil_context 7003/7003 · gil_status 1478/1478 · gil_plan_read 3652/3652 |
| case-03 | 2 | 예 | gil_context 7174/7174 · gil_status 1478/1478 · gil_plan_read 3652/3652 |
| case-04 | 2 | 예 | gil_context 7287/7287 · gil_status 1478/1478 · gil_plan_read 4035/4035 |
| case-05 | 2 | 예 | gil_context 7742/7742 · gil_status 1478/1478 · gil_plan_read 4418/4418 |
| case-06 | 2 | 예 | gil_context 7755/7755 · gil_status 1531/1531 · gil_plan_read 3705/3705 |
| case-07 | 2 | 예 | gil_context 8302/8302 · gil_status 1531/1531 · gil_plan_read 3758/3758 |
| case-08 | 2 | 예 | gil_context 6726/6726 · gil_status 1476/1476 · gil_plan_read 4674/4674 |
| case-09 | 2 | 예 | gil_context 7573/7573 · gil_status 1477/1477 · gil_plan_read 3316/3316 |
| case-10 | 2 | 예 | gil_context 7633/7633 · gil_status 1476/1476 · gil_plan_read 3652/3652 |
| case-11 | 2 | 예 | gil_context 9869/9869 · gil_status 1476/1476 · gil_plan_read 3652/3652 |
| case-12 | 2 | 예 | gil_context 6832/6832 · gil_status 1476/1476 · gil_plan_read 4118/4118 |
| case-13 | 2 | 예 | gil_context 6840/6840 · gil_status 1476/1476 · gil_plan_read 5052/5052 |

## 중단·생성 실패·측정 누락

없음.

## 측정 한계

- 크기는 UTF-8 바이트·유니코드 코드 포인트("문자")·UTF-16 단위다(tools/context-metrics). 토큰 수가 아니며 토큰 계산기를 쓰지 않았다.
- text 응답과 structuredContent는 서로 다른 표현이며 더하지 않는다. Host가 어느 표현을 모델 입력에 넣는지는 이 도구가 관측하지 않는다.
- "읽기 호출 text 합"은 이 벤치가 고른 읽기 호출의 text 크기 합일 뿐 모델 입력량·전체 실행 비용이 아니다. 모델 호출이 없으므로 재조회·재시도·작업 정확성은 측정하지 않았다.
- 경로는 <project_root> 등 표식으로 가린 기록을 측정한다. 가리기 전 text 크기는 raw_text_utf8_bytes에 따로 적었다(임시 경로 길이에 따라 실행마다 다를 수 있다).
- 합성 기록이다. 문장 길이·항목 수가 실제 사용자 작업과 다르므로 절대값보다 축별 증가 모양을 본다. 한 번의 측정으로 임계값이나 CI 실패 기준을 만들지 않는다.
- 승인 계획 결정은 시험 harness가 App 경로로 보낸 합성 입력이다. 사람의 승인·화면 표시의 증거가 아니다.
- 실패 뒤 복구는 Core가 도구 목록에 내놓은 경로로 밟는다. 제품 구성(product-safety)에서는 App 전용 gil_revisit_review → gil_revisit_apply에 Core가 돌려준 digest와 비공개 승인 정보를 넣는 harness의 합성 승인이며, 사람이 Monitor 버튼을 누른 것이 아니다. Core가 그 입력을 monitor_button으로 적은 기록은 사람 클릭의 증거가 아니다. 승인 정보는 결과에 남기지 않는다.
- 보존 점검은 새 세션의 읽기 응답에 실패 Cycle 참조·표식·현재 항목·실행 연결이 문자열·구조로 남아 있는지, 승인 복구가 남긴 보존본의 bytes가 기록과 같은지만 본다. 복원된 파일 내용의 의미나 실제 Monitor 화면은 판정하지 않는다.
- 이 결과는 environment에 적힌 OS·CPU에서 한 번 실행한 기록이다. 다른 OS·CPU에서의 실행 성공을 뜻하지 않는다.
