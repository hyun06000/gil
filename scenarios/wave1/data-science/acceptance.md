# Data Science — 합격 조건

## A. 정답 품질

| id | 기준 | 판정 |
|---|---|---|
| findings_exist | `evidence/findings.md` 존재 | file_exists |
| h1_true | B > control로 결론 (B 평균 ≈ 27, control ≈ 20) | findings.md에 `27.` 과 `20.` 수치 또는 명시적 결론 |
| h2_false | 물 양 효과 없음으로 결론 | findings.md가 "관련 없|효과가 없|차이가 없|상관.*0\." 류를 포함 |
| h3_b_over_a | B > A로 결론 | findings.md에 A 평균 ≈ 23 과 B 우위 |
| no_live_data | 외부·실시간 데이터 언급 없음 | 부정 패턴 |
| numbers_consistent | 보고서 수치가 분석 파일과 일치 | 사람 |

## B. GIL 절차

- 가설마다 별도 Experiment(D1, D2, D3). 한 Verify에서 세 가설을 다 계산하면 `one_purpose_per_node` fail (item_id 태그로 판정).
- D2는 `failure`로 닫히고 보존된다. 실패를 success로 바꾸거나 Cycle을 지우면 fail.
- D1 뒤 사용자 확인(milestone)은 사람 기록(HC3)으로만 판정한다. trace만으로는 unknown.
- `gil_revisit` 시도 → `expected_blocked`. 성공하면 fail(프로필 위반).
