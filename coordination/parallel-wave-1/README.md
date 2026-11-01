# Claude 동시 개발 Wave 1

목표: 서로 독립적인 구현/자동시험을 병렬로 끝낸 뒤 하나의 통합본에서 실제 Mac·Windows·Codex·Luna 검증과 디버깅을 수행한다.
실제 개발 세션은 사용자가 Claude Code에서 직접 연다. 이 문서 생성은 작업 착수나 기능 완료를 의미하지 않는다.

## 공통 출발점

- 대상: `https://github.com/hyun06000/gil`.
- 현재 main 기준: `3b5341f`. 준비 시 조회한 열린 PR은 없었다.
- 필요한 M6-D 구현: `fcd6e5d90363ab880b27ccbd85e7c6936609ef93` (로컬, 미병합·미배포).
- 준비 branch `coordination/parallel-wave-1-base`는 위 코드와 이 작업 지침을 함께 포함한다.
  각 세션은 같은 준비 commit에서 갈라진 전용 branch/worktree를 사용한다.
- 작업 도중 main이나 다른 세션 branch를 각자 당겨 섞지 않는다. 통합 담당자가 일괄 검토한다.
- 이전 `coordination/CLAUDE-M3-SAFETY-PROMPT.md`는 A/B로 분할되었다.
  이전 통합 M3 세션이 이미 실행 중이라면 그것을 그대로 추가 실행하지 말고 담당을 A 또는 B로 재배정한다.

## 세션별 소유권

| 세션 | 작업 | 주 소유 파일 | 끝나면 얻는 것 |
|---|---|---|---|
| [A](A-TRACKING.md) | 추적/ignore 정책 | artifact의 새 정책 모듈, mod.rs의 명시적 새 관측 API, Cargo 의존성 | 합성 폴더에 적용되는 정책 엔진·정책 ID |
| [B](B-RESTORE-PREVIEW.md) | 복원 미리보기·경로 판정 | restore의 새 preview 모듈, mod.rs의 모듈 선언 | 비파괴 diff·보존/충돌/거절 결과 |
| [C](C-MONITOR-UX.md) | Monitor 승인·진행 UX | mcp-app UI/시험, 생성 monitor.html/json | 초보 사용자가 승인 대기와 현재 항목을 알아보는 화면 |
| [D](D-AGENT-GUIDANCE.md) | 가벼운 모델 안내 | guidance/workflow/context, Manual 설명, 생성 Skill | 현재 상태에 맞는 다음 행동·복구 안내 |
| [E](E-SCENARIOS.md) | 자동 시나리오·wire 시험 | 신규 scenarios/tools/scenario tests | 네 작업 유형·실제 stdio 회귀·실기 평가표 |
| [F](F-M6E-DESIGN.md) | M6-E 설계 | 신규 proposal·순수 전이표 fixture | 열린 단계 중단/실패/재시도 계약 추천안 |

A–E는 구현 또는 실행 가능한 자동시험이다. F는 명세 미결을 해소하는 설계이며 Core 구현 완료로 세지 않는다.
M2의 Knowledge/Memory/Relations 객체와 Chain merge는 별도 schema/의미론 결정이 필요하므로 이 묶음에 억지로 포함하지 않는다.

## 병렬로 가능한 근거와 한계

현재 capture는 정책을 거의 갖지 않고, restore/plan은 target에 없는 파일을 Delete로 계획한다.
A는 관측 정책, B는 읽기 전용 미리보기를 나누고 callback으로 연결하므로 서로 코드를 기다릴 필요가 없다.
다만 정책을 기존 Snapshot에 소급 적용하면 파일 보존 의미가 바뀐다. 저장/Session/apply 연결은
두 결과를 함께 검토한 뒤 통합 단계에서 처리하며 이번 분업으로 안전 restore 전체가 완료되는 것은 아니다.

C와 D는 M6-D의 이미 정한 wire를 읽는 서로 다른 표면이다. E는 외부 프로세스로 그 계약을 검사한다.
따라서 UI/안내/wire 시험을 나눌 수 있고, E가 찾아낸 제품 결함은 통합 담당자가 공유 Core에서 수정한다.
F의 새로운 상태 전이는 Project·Will·Journey·저장을 함께 바꾸므로 다른 세션과 동시에 확정하지 않는다.

## 빌드 자원과 작업 규칙

이 맥의 준비 시 디스크 여유는 약 23 GiB였다. Rust target을 여섯 벌 만들지 않는다.

```sh
python3 coordination/parallel-wave-1/cargo-queue.py -- cargo test --locked -p gil
python3 coordination/parallel-wave-1/cargo-queue.py -- cargo build --locked -p gil --all-targets
```

- 큐는 모든 worktree가 공유하는 저장소의 `target/claude-wave-1`을 사용한다.
- 빌드/시험 전체 동안 잠금을 유지한다. 다른 branch 빌드가 테스트 중인 바이너리를 바꾸지 못하게 한다.
- native Core를 사용하는 시나리오 runner도 같은 큐 안에서 실행한다. native를 띄우지 않는 JS 시험은 독립 실행한다.
- 공유 target의 실행 파일을 다른 branch가 마지막으로 빌드했을 수 있다. `cargo test`는 같은 잠금 안에서
  자체 빌드하므로 괜찮지만, Node runner는 **같은 큐 명령 안에서 현재 branch의 Core를 빌드하고 곧바로 실행**한다.
  이전 명령에서 빌드했다는 이유만으로 `target/debug/gil`을 재사용하지 않는다. 중첩 큐 호출은 하지 않는다.
- 각 명령은 별도 합성 TMPDIR과 최대 2개의 Cargo build job을 사용한다. 큐를 강제 종료해 우회하지 않는다.
- 소스 작업은 여섯 세션이 동시에 한다. Node 의존성은 lockfile대로 각 worktree에서 준비한다.
- `cargo clean`, 공용 cache 삭제, 다른 worktree 수정, main 직접 push, 자동 병합·배포를 하지 않는다.

## 각 세션의 완료 기준

전용 로컬 commit과 `results/<세션>-RESULT.md`를 제출한다. 구현/검증/남은 연결/발견 결함을 분리한다.
테스트 실패를 숨기지 않으며 기대 차단·실행 불가·사람 확인 미수행을 pass로 합치지 않는다.
각자의 자동시험을 모두 끝난 뒤로 미루지 않는다. 사용자 실기는 통합 후에 한 번의 버전으로 수행한다.

공통 생성 고지 파일은 통합 담당자만 수정한다. A의 dependency/Cargo hash 또는 C의 bundle hash 때문에
그 branch의 packaging 검사가 실패하면 정확한 차이를 보고한다. 잘못된 정책을 통과시키지 않는다.

## 모든 세션이 끝난 뒤의 순서

1. 여섯 결과 문서와 commit을 수집하고 소유 파일/공통 기준/미검증·결함을 확인한다.
2. 공통 base에서 통합 branch를 만든다. A/B를 함께 연결·검토하고 C/D/E를 차례로 통합한다.
   F의 문서는 검토하되 미결 전이 의미를 자동 적용하지 않는다.
3. 공용 export·정책 provenance·모듈 연결·UI 생성물·고지 inventory/hash·CI·로드맵을 단일 소유자로 정리한다.
   준비가 안 된 기능은 개발 프로필에 남기고 기존 저장 의미와 product-safety를 유지한다.
4. 기본/Chain/approved-plan Core·MCP·UI·패키징 및 E의 신규 시나리오 자동시험을 통합 commit에서 돌린다.
   새로운 실패는 세션별 결과와 비교해 통합 담당자가 디버깅한다.
5. PR/양 OS CI를 거친 검토 가능한 테스트 설치본 하나를 만든 뒤 실제 Mac/Windows와 Luna에서 검증한다.
   게시·설치·업데이트는 해당 단계의 사용자 지시에 따라 수행한다.
6. 실제 화면/버튼/재개/증거/재인터뷰/종료를 같은 commit의 설치본으로 확인하고 결함을 수정한다.
   서로 다른 여섯 설치본을 사용자에게 번갈아 시험하게 하지 않는다.

이 순서는 이번 분업으로 모든 로드맵이 완성된다는 약속이 아니다. 연결과 실기까지 같은 결과물로 검증하기 위한 합류 계획이다.
