# 기록된 실행

날짜별 폴더에 `tools/scenarios/wire-runner.mjs`의 가림된 산출물을 둔다: `wire-run.json`(case 표·메타), `wire-run.md`, `wire-events.jsonl`(채점기 입력 형식).
토큰은 `<redacted-token>`, 임시 경로는 `<project_root>`/`<settings_dir>`/`<work_dir>`로 치환되며 runner의 누출 검사를 통과한 파일만 기록한다.
프레임 전체(`wire-frames.jsonl`)는 크기 때문에 커밋하지 않는다; 재현은 각 폴더 `wire-run.md`의 명령으로 한다.
이 기록은 프로토콜 회귀 결과이지 사람의 승인·화면 인수·모델 평가 결과가 아니다.
폴더 이름이 같은 날짜에 여러 실행이 있으면 `<날짜>-<lane>` 접미사로 나눈다. 과거 폴더는 덮어쓰지 않는다.
`2026-11-02-wave2-D`(approved-plan,visual-authoring,product-safety)와 `2026-11-02-wave2-D-plain`(approved-plan,product-safety)은
Wave 2 D가 고친 runner(격리 환경·제품 배치 기록 경로·연결 채점)로 현재 branch Core를 다시 돌린 결과다. Mac에서만 실행했다.
