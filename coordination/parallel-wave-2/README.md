# Claude 병렬 수정 Wave 2

목표: Wave 1 검토에서 재현된 결함을 고치고 자동시험으로 고정한다. 신규 기능 확장·M6-E 구현·제품
capture/restore 연결은 이번 묶음에 넣지 않는다. 실제 사용 검증은 수정 결과를 합친 한 설치본에서 한다.

코드 기준은 검토 결과 `197a402`를 포함한 `coordination/parallel-wave-2-base`다. 다섯 worktree는
이 안내와 수정된 빌드 큐까지 포함한 동일 commit에서 출발한다. Wave 1 원본 브랜치를 이어서 수정하지 않는다.
검토 근거는 `../parallel-wave-1/review/REVIEW.md`, `validation.json`, `regressions.rs`, `probes.mjs`다.

| 세션 | branch | 소유 작업 | 프롬프트 |
|---|---|---|---|
| A | claude/wave2-restore-safety | R1/R2 복원 미리보기 경로·정규화 | A-RESTORE-SAFETY.md |
| B | claude/wave2-monitor-state | R3 실제 Cycle 상태 조회·UI | B-MONITOR-STATE.md |
| C | claude/wave2-guidance | R4 다음 호출·D 승인 프로필 시험 | C-GUIDANCE.md |
| D | claude/wave2-scenario-validation | R5 채점기·E Windows 시험 | D-SCENARIO-VALIDATION.md |
| E | claude/wave2-profile-tests | 공통 승인 fixture·도구 catalog·CI | E-PROFILE-TESTS.md |

## 모든 세션의 공통 규칙

1. 사용자에게 지정된 전용 worktree에서 branch/status와 `197a402` 조상 여부를 확인한다. 다른
   폴더의 checkout/reset/stash/clean을 하지 않는다. 시작 상태가 다르면 보존하고 불일치를 보고한다.
2. AGENTS.md, CONTRIBUTING.md, 이 문서, INTERFACES.md, 검토 보고서 및 담당 명세·코드를 읽는다.
   검토 재현 자료는 시험 데이터다. 실제 사용자 기록·설치된 Plugin으로 bootstrap하지 않는다.
3. 소유 목록 밖 파일은 읽기만 한다. 연결 변경이 필요하면 정확한 서명·실패 재현·예상 diff를 결과에
   남긴다. 다른 세션의 수정을 가져오거나 구현 방향을 임의로 바꿔 공유 파일을 고치지 않는다.
4. 생산 코드의 승인·증거·product-safety·저장 형식·Will/Journey·자동 이전 정책을 약화하지 않는다.
   Core 거절을 성공으로 바꾸거나 테스트를 지워 통과시키지 않는다. 실제 소스 폴더에서 gil start 금지.
5. 로컬 커밋까지 완료한다. push/PR/병합/CI dispatch/릴리즈/설치/실제 모델 호출은 하지 않는다.
   기능 추가가 아닌 결함 수정 묶음이다. F 설계의 D1/D2/D3를 이번에 임의 확정하지 않는다.
6. 자신의 재현 시험과 관련 검사부터 실행한다. 원래 실패와 수정 후 결과, 타 세션/통합 소유 실패,
   환경 제한을 구분한다. 지원하지 않는 OS를 합성 시험으로 통과했다고 하지 않는다.
7. 결과: `coordination/parallel-wave-2/results/<LETTER>-RESULT.md`에 수정·시험·미검증·남은 연결을
   기록하고 로컬 commit SHA를 사용자에게 보고한다. 다른 결과 문서·로드맵·공통 지침은 수정하지 않는다.
   개인 절대경로·token·실제 .gil·대화·실행 파일·cache는 커밋하지 않는다.

## 빌드 — 같은 잠금, 서로 다른 실행 파일

```sh
python3 coordination/parallel-wave-2/cargo-queue.py -- cargo test --locked -p gil
python3 coordination/parallel-wave-2/cargo-queue.py -- cargo build --locked -p gil --all-targets
```

- 같은 저장소 전체의 빌드 순서만 잠금으로 직렬화한다. target은 **worktree 절대경로의 hash별로 분리**한다.
  Wave 1의 shared target/debug/gil 재사용 문제를 반복하지 않는다. 옛 큐 경로도 새 구현으로 위임한다.
- `CARGO_INCREMENTAL=0`, dev/test debug info=0, jobs=2로 디스크·메모리 사용을 줄인다. 기능 flag와
  debug assertions를 끄는 설정이 아니다. 준비 시 가용 공간 약 14 GiB이며 공간을 주기적으로 확인한다.
- native runner도 큐 안에서 현재 branch를 빌드하고 같은 슬롯에서 실행한다. `GIL_PARALLEL_TARGET_DIR`는
  현재 worktree의 target이다. 중첩 큐를 호출하지 않는다. 큐 밖에서 다른 세션의 바이너리를 실행하지 않는다.
- 순수 JS 시험은 독립 실행한다. 각 worktree에서 `npm ci --prefix mcp-app`과
  `npm ci --prefix plugins/gil-companion-prototype`를 해당 lockfile대로 준비한다.
- 공유/다른 세션 cache를 지우지 않는다. 공간이 부족하면 결과를 보존하고 통합 담당자에게 알린다.
  OS 로컬 소켓 제한으로 시험이 실패하면 제품 결함과 구분하고 환경의 정식 승인 경로로 재실행한다.

## 동시 작업과 통합 경계

- A만 필요 최소한의 Cargo 의존성을 추가할 수 있다. 라이선스/크기/lock 변경을 보고한다.
- B만 plan 조회의 read-only current_cycle 필드 및 Monitor UI/생성 번들을 수정한다.
- C는 context/guidance/workflow/자기 시험, D는 scenario runner/scorer/자기 wire 시험을 담당한다.
- E는 C/D의 시험 파일을 수정하지 않고 공통 fixture·catalog 시험·CI를 정리한다.
- 고지 inventory/notice-policy 해시는 A/B 결과가 모두 나온 뒤 통합 담당자가 갱신한다. E가 작업 도중
  B의 HTML hash를 대신 복사하지 않는다. 현 baseline의 고지 2개 실패는 숨기지 말고 통합 대기라고 기록한다.
- 각 branch에서 다른 담당의 기존 실패가 남을 수 있다. 전체 녹색이라고 주장하지 말고 내 수정이 고친
  항목과 통합 대기 항목을 구분한다. 마지막에는 통합 담당자가 전부 다시 돌린다.

완료 후: 다섯 보고/commit 검토 → 한 branch로 합류 → 고지·CI·전체 계약 검사 → 테스트 설치본
→ 실제 Mac/Windows/Codex/Luna. 병합과 배포는 검증된 구체 결과로 다음 단계에서 진행한다.
