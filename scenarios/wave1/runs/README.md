# 기록된 실행

날짜별 폴더에 `tools/scenarios/wire-runner.mjs`의 가림된 산출물을 둔다: `wire-run.json`(case 표·메타), `wire-run.md`, `wire-events.jsonl`(채점기 입력 형식).
토큰은 `<redacted-token>`, 임시 경로는 `<project_root>`/`<settings_dir>`/`<work_dir>`로 치환되며 runner의 누출 검사를 통과한 파일만 기록한다.
프레임 전체(`wire-frames.jsonl`)는 크기 때문에 커밋하지 않는다; 재현은 각 폴더 `wire-run.md`의 명령으로 한다.
이 기록은 프로토콜 회귀 결과이지 사람의 승인·화면 인수·모델 평가 결과가 아니다.
