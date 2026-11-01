# 평가표 양식

한 행이 한 실행이다. 사람 칸은 사람이 적는다. 채점기 출력(`score.json`)의 verdict를 그대로 옮기고 다듬지 않는다.

| run id | scenario | arm (control/GIL) | synthetic approval? | model / effort | answer verdict | procedure verdict | monitor_visible (사람) | approved_by_click (사람) | 비고(결함 seq 등) |
|---|---|---|---|---|---|---|---|---|---|
| | | | | | | | | | |

## human.json 양식

```json
{
  "monitor_visible": true,
  "approved_by_click": true,
  "stepwise_review": { "HC2": true },
  "answer": { "layout_ok_at_360": true },
  "notes": "자유 기록. 개인 경로·토큰·대화 원문은 넣지 않는다."
}
```

- 보지 못한 항목은 **키를 넣지 않는다**. `false`는 "관측했고 아니었다"는 뜻이다.
- `answer.<id>`는 rubric의 `kind: "human"` 검사에 대응한다.

## 집계

| scenario | arm | runs | answer pass | procedure pass | procedure fail 항목 빈도 | unknown 빈도 |
|---|---|---|---|---|---|---|
| | | | | | | |
