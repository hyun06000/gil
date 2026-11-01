# D — 승인 평가 정확성 및 Windows 시험 격리

Common instructions: `coordination/parallel-wave-2/README.md` and `coordination/parallel-wave-2/INTERFACES.md`.

예상 branch: `claude/wave2-scenario-validation`.
전용 worktree에서 README.md와 INTERFACES.md를 읽고 수정·자동시험·로컬 커밋까지 완료한다.
검토 R5, probes.mjs의 음성 로그, E의 Windows 경로 한계를 먼저 재현/확인한다.

소유 파일:
- tools/scenarios/**.
- scenarios/wave1의 평가 계약·음성 fixture·설명 (기존 runs/ 산출물은 수정 금지).
- tests/approved_plan_wire_wave1.rs, tests/support_wave1/**.
- coordination/parallel-wave-2/results/D-RESULT.md 및 가린 새 실행 근거.
tests/common/mod.rs는 E 소유이며 기존 Windows helper는 읽기만 한다. 제품 Core는 변경하지 않는다.

작업:
1. approval_before_execution을 과거 어느 승인 한 번으로 통과시키지 않는다. 해당 scope/Chain의
   유효한 승인 판과 실행 항목을 관측으로 연결한다. read가 없으면 추측 pass 대신 unknown을 남긴다.
   이전 Chain 승인, 수정 전 승인, request_changes, 중복/응답 유실/로그 중간 시작, 정상 다중 Chain을
   구분하는 음성·양성 fixture를 추가한다. no_reused_approval 등 인접 평가도 같은 경계를 유지한다.
2. 사람 클릭/화면 표시는 실제 human checkpoint만 인정한다. 합성 App 입력을 실제 사용자 동의로
   승격하지 않는다. Core 차단 시험 통과와 모델 작업 평가 통과를 별도로 기록한다.
3. Rust wire test와 JS runner의 project_root/.gil/state.yaml 하드코딩을 없앤다. 기존 제품의
   project_state 및 Windows fixture 경로·환경 설정을 읽고 동일 계약에 맞춘 전용 helper를 만든다.
   LOCALAPPDATA 등 시험 데이터 루트는 임시 폴더 아래로 격리한다. HOME/PATH/필수 Windows 환경
   누락, 저장 bytes 비교가 null이면 통과하는 문제, 종료/timeout 시 자식 정리도 점검한다.
4. 실제 Windows 실행이 불가능하면 그 한계를 남긴다. Mac에서 Win32 경로 문자열만 시험한 것을
   Windows native 성공이라고 부르지 않는다. Windows CI 연결은 E가 소유한다.
5. wire runner W01–W21을 현재 branch의 Core로 새로 실행하고 token/개인경로를 가린 근거를 남긴다.
   이전 runs를 덮어쓰지 말고 이번 결과 위치를 분리한다. 정당한 expected_blocked/unknown은 유지한다.

검증: tools/scenarios/*.test.mjs, 새 채점기 음성 fixture, approved-plan 프로필 실제 wire test 및
runner. 일반/visual 승인 프로필의 차이도 지원 주장에 맞게 검사한다. native는 Wave 2 큐 안에서
현재 branch build와 run을 이어 실행한다. 실제 모델 호출·사용자 설치본 시험은 후속 통합 단계다.
