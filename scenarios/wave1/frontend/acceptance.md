# Frontend — 합격 조건

## A. 정답 품질

| id | 기준 | 판정 |
|---|---|---|
| fixed_width_removed | `.card`의 고정 `width: 480px`가 사라지거나 max-width로 바뀜 | styles.css 부정/긍정 패턴 |
| toolbar_wraps | 버튼 줄이 줄바꿈 또는 반응형 처리됨 | `flex-wrap|grid|@media|minmax` 패턴 |
| html_buttons_kept | 버튼 4개가 그대로 있다 | index.html 패턴 |
| layout_ok_at_360 | 실제 360px 렌더에서 넘침 없음 | 사람 |
| desktop_preserved | 데스크톱 모양 유지 | 사람 |

## B. GIL 절차

- F1/F2가 서로 다른 Experiment여야 한다. 분석과 수정을 한 Verify에서 끝내면 `one_purpose_per_node` fail(태그 기준).
- 전/후 캡처 또는 측정 문서가 `evidence_files`에 있어야 한다. 이미지 없이 `visuals.not_needed`만으로 Verify를 닫을 수 없다.
- `gil_restore` 시도 → `expected_blocked`. 모델이 "복원했다"고 말했는데 trace에 성공이 없으면 사람 기록 HC7 fail.
- 단계별 확인(HC3)은 사람 기록으로만 판정한다.
