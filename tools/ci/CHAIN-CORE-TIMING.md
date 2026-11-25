# Chain CI 실행 시간과 CI 전용 빌드 설정 (2026-11-19 측정)

`.github/workflows/chain-core.yml`의 macOS·Windows job은 검사 단계, 기능 조합, 시험 필터,
`expect-tests` 최소 실행 수를 그대로 두고 job 환경에 두 값만 더한다.

| 값 | 의미 | 바꾸지 않는 것 |
|---|---|---|
| `CARGO_PROFILE_DEV_DEBUG=line-tables-only` | dev·test 프로필(test는 dev를 상속)의 디버그 정보를 줄 번호 표만 남김 | release 프로필, 최적화 수준, `debug_assertions`, `overflow_checks`, panic 방식, 시험 필터·실패 판정 |
| `CARGO_INCREMENTAL=0` | 증분 컴파일 상태를 쓰지 않음(CI는 이 상태를 재사용하지 않음) | 위와 같음. 산출물의 의미는 같고 생성 방식만 다름 |

`tools/ci/cache.test.mjs`는 이 두 값 외의 프로필·컴파일러 환경(`RUSTFLAGS`, release·opt-level·assertion·
overflow·panic·codegen·LTO·strip)과 `--release`/`--profile` 사용을 거절하고, 저장소에 `[profile]` 절이
생기면 실패한다. 개발자 로컬 빌드와 배포 빌드는 영향을 받지 않는다.

**진단 손실.** 줄 번호 표는 panic 위치와 `RUST_BACKTRACE` 역추적의 함수·파일·줄을 유지한다(panic 위치는
디버그 정보와 무관하게 컴파일된다). 디버거의 지역 변수·형 정보는 사라진다. CI는 디버거를 붙이지 않으므로
실패 로그 내용은 같다. 디버그 정보를 완전히 끄는(`0`) 안은 Windows에서 더 빨랐지만 역추적의 파일·줄을
잃으므로 채택하지 않았다(아래 표).

## 1. 기준 실행 분해

`node tools/ci/job-timing.mjs <job.log>`로 job 로그를 단계 경계에서 겹치지 않게 나눈다. Cargo가
`Finished`를 출력하지 못한 Cargo 단계(취소·컴파일 실패)는 "컴파일 중단"으로 따로 센다. 대기 시간은
로그에 없으므로 run `created_at`→job `started_at`이다. "Cargo/시험 보고 합계"는 도구가 출력한 시간의
합이며 경과 시간 범주와 더하지 않는다. 단위는 초.

| 실행 | PR #172 최종 Windows | PR #172 최종 macOS | 병합 후 main Windows | 직전 시간 초과 Windows |
|---|---|---|---|---|
| run / job | 44527111497 / 135570526610 | 44527111497 / 135570527165 | 44533203704 / 135591679453 | 44521604369 / 135551349427 |
| 결과 · job 시간 | 성공 35:08 | 성공 19:09 | 성공 26:15 | 30분 제한 취소 |
| 서버 배정 대기 | 7 | 319 | 3 | 4 |
| 준비(job 준비·checkout·rustup) | 23.9 | 13.4 | 25.7 | 22.8 |
| 캐시 복원 | 15.8 | 7.2 | 16.6 | 13.1 |
| Cargo 컴파일·연결(경과) | 1230.5 | 657.0 | 869.0 | 1070.9 |
| Cargo 시험 실행(경과) | 679.1 | 389.8 | 506.4 | 625.7 |
| Cargo 컴파일 중단(완료 문구 없음) | 0 | 0 | 0 | 67.6 |
| Node 검사 | 19.2 | 54.0 | 14.0 | 0 |
| 캐시 정리(`prune-workspace-cache`) | 13.1 | 13.8 | 7.0 | — |
| 캐시 묶음 생성·압축 | 119.4 | 4.4 | 129.7 | — |
| 캐시 전송 | 2.1 | 3.7 | 2.1 | — |
| 나머지 후처리 | 2.3 | 0.8 | 1.8 | 3.4 |
| *Cargo 보고 빌드 합계* | *1175.3* | *628.0* | *828.3* | *1020.3* |
| *libtest 보고 시험 합계* | *669.1* | *367.6* | *496.7* | *614.8* |

- 네 실행 모두 정확한 키가 아니라 같은 접두사의 이전 키(`…-deps-v2-7e275fe…`, macOS `…db19a4d…`)를 복원했다.
  의존성 출력은 있었고 `gil` 자신만 기능 조합마다 다시 컴파일됐다. 캐시 크기는 Windows 약 149MB,
  macOS 약 127MB.
- 병목은 Cargo 컴파일·연결이다(Windows 58%, macOS 57%). 같은 기능 조합의 `cargo test`·`cargo build
  --all-targets`는 이미 연속 배치돼 있어 `gil` 라이브러리와 시험 실행 파일을 다시 만들지 않는다.
  남은 컴파일은 기능 조합 9개(기능 없음 포함) × (라이브러리 + 시험 실행 파일)이며, Windows에서는 PDB 연결 비용이 크다.
- Windows 캐시 후처리 약 2분은 이전 키 복원 뒤 새 키를 저장할 때만 든다. 실제 전송은 약 2초이고 나머지는
  묶음 생성·압축이다. 정리 단계는 workspace 출력 42.3GiB(macOS 16.7GiB)를 지웠다. 정확한 키가 있는 실행
  (일반 소스 변경 PR)은 정리·재저장을 건너뛴다. 이 비용은 workflow·정리 스크립트·lockfile이 바뀌어 키가
  달라진 실행과 그 병합 후 main 실행에서 한 번씩 든다.
- PR #172 최종 실행(Windows 35분)과 병합 후 main 실행(Windows 26분)은 같은 단계·같은 이전 키 복원인데
  Cargo 보고 시간이 1175초와 828초로 달랐다. 서버 간 차이가 크므로 한 번의 원격 비교로 개선율을
  단정하지 않는다.
- 시간 초과 실행은 46번째 단계(`cargo build --all-targets`, end history + 제품 프로필)를 컴파일하던 중
  30분 제한으로 취소됐다. 완료 문구가 없는 이 67.6초는 완료된 컴파일과 따로 센다.

## 2. 후보 측정

측정 그룹은 `will-end-history,visual-authoring,product-safety` 단계 42–46의 Cargo 명령 **하나**다.
아래 수치는 이 한 기능 조합의 빌드 비교이며 전체 Chain job의 개선율이 아니다. 전체 job 비교는 PR 본문의
원격 실행(같은 PR의 캐시 재사용 실행)으로 따로 한다.
각 디버그 정보 값마다 의존성을 먼저 빌드한 뒤 `cargo clean -p gil -p gil-companion`으로 CI 캐시 상태를
재현하고, 증분 켬/끔을 같은 target에서 차례로 쟀다. 원격 실험은 병합하지 않는 별도 브랜치
`claude/ci-build-settings-experiment`의 임시 workflow다(캐시 저장 없음).

**Windows 원격** (windows-2025, pwsh, run 44547012584 / job 135640177344; 라이브러리 시험은 `--no-run`,
통합 시험 80건 실행. 증분 순서를 디버그 값마다 바꿔 실행 순서 효과를 줄임)

| 디버그 정보 | 증분 | Cargo 보고 빌드 | 경과 | 그룹 뒤 target | 의존성만 | 의존성 tar+zstd | 의존성 최초 빌드 |
|---|---|---|---|---|---|---|---|
| 2(기본) | 켬(기본) | 116.0 | 140.3 | 7.82GiB | 553MB | 80.5MB | 137 |
| 2 | 끔 | 96.2 | 115.8 | 5.67GiB | | | |
| line-tables-only | 끔 | **61.2** | **79.0** | 1.99GiB | 457MB | 67.4MB | 86 |
| line-tables-only | 켬 | 67.7 | 85.7 | 3.06GiB | | | |
| 0 | 켬 | 55.1 | 73.8 | 2.23GiB | 434MB | 63.3MB | 70 |
| 0 | 끔 | 47.5 | 65.9 | 1.34GiB | | | |

**macOS 원격** (macos-15 arm64 3코어, run 44544148269 / job 135630086354; 증분 끔이 항상 두 번째)

| 디버그 정보 | 증분 켬 빌드/경과 | 증분 끔 빌드/경과 | 의존성 tar+zstd |
|---|---|---|---|
| 2 | 76.6 / 98.3 | 57.7 / 74.7 | 113.6MB |
| line-tables-only | 82.2 / 99.1 | 60.0 / 75.7 | 96.2MB |
| 0 | 82.3 / 98.5 | 61.2 / 76.7 | 79.5MB |

**macOS 로컬** (Apple Silicon 8코어, 같은 그룹, 라이브러리 시험 753건 포함 실행, 2회 역순; 각 설정마다
새 target)

| 설정 | Cargo 보고 빌드 (1회/2회) | 경과 (1회/2회) | 그룹 뒤 target |
|---|---|---|---|
| 기본 | 46.2 / 44.4 | 119.8 / 116.2 | 2.75GB |
| line-tables-only | 45.5 / 38.5 | 117.3 / 108.9 | 2.30GB |
| 0 | 39.5 / 34.3 | 110.7 / 103.1 | 1.92GB |
| 증분 끔 | 41.3 / 34.3 | 114.7 / 106.4 | 1.76GB |
| line-tables-only + 증분 끔 | 28.0 / 29.2 | 97.1 / 98.3 | 1.48GB |
| 0 + 증분 끔 | 25.4 / — | 93.0 / — | 1.06GB |

판정:
- 증분 끔은 세 환경 모두에서 빌드 시간을 줄였다. CI는 증분 상태를 캐시하지 않고 같은 기능 조합을 다시
  빌드하지 않으므로 잃는 것이 없다. **채택.**
- 줄 번호 표는 Windows에서 빌드 시간을 크게 줄였고(증분 끔 기준 96.2→61.2초) 의존성 캐시를 16% 줄였다.
  macOS 원격에서는 시간 차이가 측정 오차 안이었고 캐시·디스크만 줄었다(113.6→96.2MB). 한 workflow의
  캐시 키 식을 OS별로 나누지 않기 위해 두 OS에 같이 적용한다. **채택.**
- 디버그 정보 0은 Windows에서 13.7초 더 빨랐지만 역추적의 파일·줄을 잃는다. **채택하지 않음.**
- 시험 실행 시간은 설정과 무관하게 측정 오차 안이었다.
- 첫 Windows 실험(run 44544148269)은 Git Bash에서 돌려 `tests/mcp.rs`의 ACL 거부 확인이
  `permission_denied`를 보지 못했다(chain-core는 pwsh이며 기준 실행에서 같은 시험이 통과). 셸만 pwsh로
  바꿔 다시 측정했고, 그 실패는 실험 환경의 차이로 기록한다.

같은 소스에서 CI가 실행하지 않는 `cargo test --locked -p gil --features chain-core --no-fail-fast`를
기본 설정과 새 설정으로 각각 로컬 실행했다. 두 실행 모두 1586건 통과, 기존 86건 실패이며 실패 시험
목록이 같았다(형식 4 고정 suite 등 기존 실패, 이 변경과 무관).

## 3. 캐시

- 키 접두사에 `dev-debug-${{ env.CARGO_PROFILE_DEV_DEBUG }}`를 넣었다. 디버그 정보 값이 다르면 의존성
  산출물 전체가 달라지므로 다른 값으로 만든 캐시는 접두사로도 복원하지 않는다. 이전 키를 복원하면 Cargo는
  새 산출물을 따로 만들고 정리 단계는 의존성을 남기므로 두 벌이 캐시에 쌓이게 된다. 증분 상태는 캐시에
  들어가지 않으므로 키에 넣지 않는다.
- Rust 버전·OS·CPU·lockfile은 기존처럼 키에 있고 소스 커밋은 넣지 않는다. 정확한 키가 있으면 검사는 모두
  다시 수행하고 정리·재저장은 건너뛴다. 정리 스크립트, 저장 경로, 의존성 보존 검사는 바꾸지 않았다.
  `cache.test.mjs`의 정리 시험은 기본 빌드 설정(다른 workflow)과 chain-core job의 빌드 설정을 각각
  검사하며, 두 경우 모두 workspace의 증분 상태가 제거되는지도 확인한다.
- 새 접두사의 첫 실행은 복원할 캐시가 없어 의존성을 처음부터 빌드한다(실험에서 Windows 의존성 빌드
  86초). PR 범위의 캐시는 main에서 쓸 수 없으므로 병합 직후 main push 실행도 한 번 처음부터 빌드해 기준
  캐시를 만든다. 이후 일반 PR은 main의 정확한 키를 복원한다. 비교는 세 경우를 섞지 않는다: (가) 같은 PR
  안에서 정확한 키를 복원한 실행, (나) 새 접두사의 최초 실행(PR), (다) 병합 후 main의 최초 생성 실행.
  (다)는 병합 뒤에만 잴 수 있어 이 PR에서는 측정하지 않는다.
- workflow 문구 변경도 키를 바꾸는 기존 규칙(`hashFiles(chain-core.yml, prune-workspace-cache.mjs)`)은
  유지했다. 단계 추가만으로 Windows 약 2분의 저장 비용이 PR과 병합 후 main에서 한 번씩 들지만, 키 규칙은
  네 workflow 공통 계약이라 이번 변경에서 바꾸지 않았다.
- registry 묶음 측정(Windows): `registry/src` 28,442개 파일 6.5초, `registry/cache` 0.2초, 의존성 출력
  1.1–1.7초. 실제 후처리의 묶음 생성 119–130초는 이 합보다 훨씬 길다. 원인은 확인하지 못했다
  (같은 job에서 지운 workspace 출력 42GiB 뒤의 디스크·검사 부하로 추정). 새 설정의 첫 실행에서 다시 잰다.

## 4. 검증 범위 대응표

단계 번호·이름·명령·기능 조합·필터·`expect-tests` 최소 수는 바뀌지 않았다. 새 구성의 대응은 같은 단계
번호다. 아래는 기준 실행(PR #172 최종)의 실행 수이며 새 실행과의 비교는 PR 본문에 적는다. 표기:
통과/무시/필터 제외 (시험 실행 파일 수). `cargo build --all-targets`는 선택한 시험 밖의 모든 대상(bin,
나머지 통합 시험, lib 시험)을 그 기능 조합으로 컴파일하는 단계이며 그대로 둔다.

| 단계 | 기능 조합 | 대상·필터 | Windows | macOS |
|---|---|---|---|---|
| 5 | 없음(`--no-default-features`) | run.mjs → `--test chain_resume_benchmark`(출력 포착) | 포착 | 포착 |
| 6 | `chain-core` | `--test chains chain_closing chain_handoff cycles context context_profiles will walk resume_guidance` | 158/0/0 (9) | 158/0/0 (9) |
| 7 | `chain-core` | expect≥3 `--test will_end_unsupported` | 3/0/0 (1) | 3/0/0 (1) |
| 8 | `chain-core` | `--test revisit cumulative_context_survives_… --exact` | 1/0/52 (1) | 1/0/52 (1) |
| 9 | `chain-core,visual-authoring,product-safety` | `--lib visual_start` | 27/1/723 (1) | 9/1/744 (1) |
| 10 | 〃 | `--test visual_start chain_resume_benchmark` | 2/0/0 (2) | 2/0/0 (2) |
| 11 | 〃 | `--test mcp` | 40/0/0 (1) | 37/0/0 (1) |
| 12 | 〃 | `build --all-targets` | 컴파일 | 컴파일 |
| 13 | `approved-plan,visual-authoring,product-safety` | `--lib restore::windows -- --test-threads=1` (Windows) | 5/0/767 (1) | — |
| 14 | 〃 | `--test approved_plan_wire_wave1 chain_recovery_preservation chain_monitor_fixtures` | 15/0/0 (3) | 17/0/0 (3) |
| 15 | 〃 | expect≥4 `--test closing_handoff` | 4/0/0 (1) | 4/0/0 (1) |
| 16 | 〃 | `--lib -- --test-threads=1` | 771/1/0 (1) | 774/1/0 (1) |
| 17 | 〃 | `--test context context_profiles` | 17/0/0 (2) | 17/0/0 (2) |
| 18 | 〃 | `--test guidance_wave1 guidance_wave2 guidance_interview_chain resume_guidance` | 13/0/0 (4) | 13/0/0 (4) |
| 19 | 〃 | `--test manual` (macOS) | — | 35/0/0 (1) |
| 20 | 〃 | run.mjs → `--test chain_resume_benchmark`(출력 포착) | 포착 | 포착 |
| 21 | 〃 | `--test automatic_guard write_guard -- --test-threads=1` | 15/0/0 (2) | 21/0/0 (2) |
| 22 | 〃 | `build --all-targets` | 컴파일 | 컴파일 |
| 23 | — | manual-example-check `run --gil target/debug/gil` (macOS) | — | Node |
| 24 | — | `node --test tools/chain-context-bench/*.test.mjs` (macOS) | — | Node 25/2/0 |
| 25 | `exploration-flow,visual-authoring,product-safety` | `--lib -- --test-threads=1` | 790/1/0 (1) | 793/1/0 (1) |
| 26 | 〃 | expect≥4 `--test exploration_flow` | 4/0/0 (1) | 4/0/0 (1) |
| 27 | 〃 | expect≥4 `--test exploration_tools` | 4/0/0 (1) | 4/0/0 (1) |
| 28 | 〃 | expect≥6 `--test exploration_evidence` | 6/0/0 (1) | 6/0/0 (1) |
| 29 | 〃 | expect≥1 `--test exploration_plan_panel` | 1/0/0 (1) | 1/0/0 (1) |
| 30 | 〃 | expect≥5 `--test closing_handoff` | 5/0/0 (1) | 5/0/0 (1) |
| 31 | 〃 | `--test approved_plan_wire_wave1 chain_recovery_preservation context context_profiles guidance_wave1 guidance_wave2 guidance_interview_chain resume_guidance` | 43/0/0 (8) | 45/0/0 (8) |
| 32 | 〃 | `--test manual` (macOS) | — | 35/0/0 (1) |
| 33 | 〃 | `build --all-targets` | 컴파일 | 컴파일 |
| 34 | `approved-plan,visual-authoring,recovery-auto-approval` | `--lib recovery_mode` | 4/0/769 (1) | 4/0/772 (1) |
| 35 | 〃 | `--test recovery_approval_mode` | 5/0/0 (1) | 5/0/0 (1) |
| 36 | 〃 | `build --all-targets` | 컴파일 | 컴파일 |
| 37 | `will-end-history` | expect≥17 `--test will_end` | 17/0/0 (1) | 17/0/0 (1) |
| 38 | 〃 | expect≥7 `--test will_end_session` | 7/0/0 (1) | 7/0/0 (1) |
| 39 | 〃 | `--test chains chain_closing chain_handoff cycles context context_profiles will walk resume_guidance mcp` | 187/0/0 (10) | 188/0/0 (10) |
| 40 | 〃 | `--lib -- --test-threads=1` | 708/0/0 (1) | 737/0/0 (1) |
| 41 | 〃 | `build --all-targets` | 컴파일 | 컴파일 |
| 42 | `will-end-history,visual-authoring,product-safety` | `--lib -- --test-threads=1` | 750/1/0 (1) | 753/1/0 (1) |
| 43 | 〃 | expect≥1 `--test will_end_visual` | 1/0/0 (1) | 1/0/0 (1) |
| 44 | 〃 | expect≥7 `--test will_end_session` | 7/0/0 (1) | 7/0/0 (1) |
| 45 | 〃 | `--test visual_start chain_resume_benchmark mcp chain_handoff visual_attachment` | 72/0/0 (5) | 69/0/0 (5) |
| 46 | 〃 | `build --all-targets` | 컴파일 | 컴파일 |
| 47 | `will-end-history,approved-plan,visual-authoring,product-safety` | expect≥2 `--test chain_monitor_fixtures` | 2/0/0 (1) | 2/0/0 (1) |
| 48 | — | Skill·workflow·scenario Node 시험 | Node 72/0/0 | Node 72/0/0 |
| 49 | — | `node --test tools/ci/*.test.mjs` | Node 7/0/0 | Node 7/0/0 |
| 50 | — | manual-example-check Node 시험 | Node 19/2/0 | Node 19/2/0 |
| 51–55 | — | interview·closing 평가기, fixture 무결성, resume 측정 계약 | Node 59, 129, 3 + 검사 2 | 같음 |
| 56 | — | chain-context-bench (macOS) | — | Node 25/2/0 |
| 57 | — | 캐시 정리(정확한 키가 없을 때) | 실행 | 실행 |

Node 표기는 통과/건너뜀/실패. 단계 49는 이 변경이 더한 시험만큼 7건에서 14건으로 늘어난다(빌드 설정
계약 1건, 정리 계약을 기본/Chain 설정 두 건으로 나눔 +1, `job-timing` 5건).

## 5. 채택하지 않았거나 측정하지 않은 후보

- **명령 합치기**: 같은 기능 조합의 Cargo 명령은 이미 연속이고 다시 컴파일하지 않는다(뒤 명령의
  `Finished … in 0.27s` 등). 합쳐도 줄어드는 컴파일이 없고 단계별 실패 진단만 흐려진다.
- **`restore::windows` 필터 제거(단계 13)**: 같은 lib 시험 실행 파일을 단계 16이 재사용하므로 중복 비용은
  시험 실행 약 0.5초다. Windows 복원 경계를 먼저 실패시키는 진단 가치가 더 커 유지한다.
- **스레드 수·대기 시간**: 전체 lib 시험의 `--test-threads=1`은 fork 기반 실행기의 잠금 상속 때문이다.
  바꾸지 않았다.
- **job 분할·서버 종류 변경·플랫폼 삭제**: 2026-11-19에는 대상이 아니었다(job 분할은 2026-11-25에 §6으로 채택). 당시 판단: 분할은 경과 시간을 줄여도 서버 대기·총 사용 시간·캐시
  저장을 늘린다(특히 macOS arm64 배정 대기 5분 이상, 병합 후 main에서 미배정 취소).
- **다른 연결기(`rust-lld`)**, **`registry/src` 캐시 제외**, **workflow 문구와 캐시 호환성 분리**: 측정하지
  않았다.

## 6. 두 job으로 분할 (2026-11-25)

Windows 실행이 중단된 뒤 PR 한 건의 대기 시간은 Chain job 하나가 정했다. 최근 성공 실행 기준으로 Chain은
15~18분(run 45513817750: 1034초, 45510618707: 1065초, 45506764969: 905초), 같은 PR의 다른 workflow는
6~9분(제품 후보 macOS 461~559초·Linux 412초, Preview 396~400초, 첫 시작 342~420초)이었다.

분할은 기능 조합 경계에서만 한다. 기능 조합마다 `gil`을 따로 컴파일하므로 한 조합을 두 job에 걸치면 그
컴파일이 두 번 든다. run 45513817750의 단계 시간(초)으로 나눈 결과:

| job | 기능 조합(순서 그대로) | 기준 실행의 단계 합 |
|---|---|---|
| `chain-product` | 기능 없음(resume 측정), `chain-core`, `chain-core,visual-authoring,product-safety`, `approved-plan,visual-authoring,product-safety`(+ 그 빌드를 쓰는 Node 검사 두 단계), `approved-plan,visual-authoring,recovery-auto-approval`, Core 없는 Node 검사 전부, 캐시 정리 | 164 + 233 + 89 + 23 = 509 |
| `chain-opt-in` | `exploration-flow,visual-authoring,product-safety`, `will-end-history`, `will-end-history,visual-authoring,product-safety`, `will-end-history,approved-plan,visual-authoring,product-safety` | 192 + 150 + 137 + 18 = 497 |

준비(checkout·rustup·캐시 복원, 기준 실행 22초)는 두 job이 각각 한다. 위 단계 합으로 계산한 기대값은 job당
약 9분이었다.

**분할 뒤 실측** (PR #326, run 45518641566, 두 job 모두 성공, 배정 대기 4초, 동시에 시작): `chain-product`
600초, `chain-opt-in` 591초. 분할 전 1034초에서 약 10분으로 줄었다. 이 실행은 workflow 파일이 바뀌어 정확한
캐시 키가 없던 경우(접두사 복원 + 정리·저장)이고, 같은 단계가 기준 실행보다 느렸다(승인 프로필 lib 98→108초,
end history lib 82→101초). 한 번의 실행이므로 서버 간 차이와 구분하지 못한다.

바뀌지 않은 것: 단계 이름·명령·기능 조합·필터·`expect-tests` 최소 수(§4의 단계 5~56이 그대로 있고 순서만
job별로 나뉨), 빌드 설정 두 값, 캐시 키 식과 저장 경로. 바뀐 것:

- 빌드 설정과 Rust 버전은 workflow 수준 `env:`로 옮겨 두 job이 같은 값을 쓴다.
- 의존성 캐시는 하나다. 두 job이 같은 키·접두사로 복원하고, `chain-product`만 정리 후 저장한다.
  `chain-opt-in`은 같은 고정 커밋의 `actions/cache/restore`를 써서 저장을 시도하지 않는다(같은 키를 두 job이
  저장하면 뒤쪽은 예약 실패 경고만 남긴다). 기능 조합은 외부 의존성을 바꾸지 않는다(선택 의존성 없음).
- M4-R 조합(단계 34~36)은 탐색 조합 앞, 승인 프로필 뒤로 옮겼다. 제품 프로필 안에 끼지 않는 규칙은 같다.
- job 제한 시간은 45분에서 30분으로 줄였다(45분은 Windows 실행 기준이었다).
- job 이름이 `chain (macos-15, bash)`에서 `chain product profiles (macos-15)`·`chain opt-in profiles (macos-15)`로
  바뀐다. main의 필수 검사는 `macos-arm64-preview` 하나라 보호 규칙은 영향을 받지 않는다.

비용: macOS job이 PR당 4개에서 5개가 된다. 여러 PR이 겹치면 macOS 배정 대기가 길어질 수 있다(분할 전에도
겹친 실행에서 305~621초 대기가 관측됨: run 45500007797, 45500007726, 45494238496). 총 사용 시간은 준비 한 번
(약 20~40초)만큼 늘어난다. 세 job 분할은 job당 약 6분이 되지만 다른 workflow(8~9분)보다 짧아져 PR 대기
시간은 더 줄지 않고 macOS job만 6개가 되므로 하지 않았다.

`tools/ci/cache.test.mjs`가 job 두 개의 이름·서버, job별 기능 조합 순서, 두 job의 캐시 키·경로·준비 단계
일치, 두 번째 job이 저장·정리를 하지 않음을 고정한다.

## 7. Linux 기능 매트릭스 시험 운영 (2026-11-25, 추가만 함)

macOS 동시 job 한도(계정당 5개로 관측) 때문에 macOS에서는 더 나눠도 대기가 줄지 않는다. 다음 단계는 OS와
무관한 기능 검사를 Linux로 옮기는 것이며, 이 절은 그 첫 단계다: **macOS job은 그대로 두고** 같은 기능 조합을
Linux(`ubuntu-24.04`)에서 조합당 한 행으로 나란히 돌려 통과 여부와 시간을 본다. macOS 축소는 이 결과를 본 뒤
따로 결정한다.

| 행 | 기능 조합 | lib | 통합 시험 |
|---|---|---|---|
| `chain-core` | `chain-core` | 없음(macOS와 같음) | Chain·Will 도메인 10개 + `revisit` 1건(정확한 이름) |
| `development-product` | `chain-core,visual-authoring,product-safety` | `visual_start` 필터 | `visual_start`, `chain_resume_benchmark`, `mcp` |
| `approval` | `approved-plan,visual-authoring,product-safety` | 전체 | 승인 wire·보존·Monitor fixture·Closing·문맥·안내 |
| `exploration` | `exploration-flow,visual-authoring,product-safety` | 전체 | 탐색 4종·Closing·item-queue 유지 suite |
| `end-history` | `will-end-history` | 전체 | end history·기존 Chain suite·`mcp`·형식 전환 |
| `end-history-product` | `will-end-history,visual-authoring,product-safety` | 전체 | 형식 9·전환·이미지 보고 + 승인 프로필 Monitor fixture 1건 |

- 각 행의 통합 시험은 macOS job이 같은 기능 조합으로 돌리는 suite만 쓴다(`cache.test.mjs`가 대조). 행마다
  `expect-tests` 최소 수를 한 번 적용한다(macOS 단계별 최소 수의 합 이상). 단계별 이름은 Linux에서는 합쳐진다.
- Linux에서 돌리지 않는 것: `manual`, `automatic_guard`, `write_guard`(macOS 유지 대상), macOS 제품 빌드를
  읽는 Node 검사 두 단계, resume 측정(`run.mjs`), Core 없는 Node 검사(이미 macOS job에서 한 번 실행).
- Linux 의존성 캐시는 macOS와 같은 키 식(`runner.os`로 구분)이고 한 행(`chain-core`)만 정리 후 저장한다.
- 시험 분류의 근거(조건부 컴파일 문장과 시험 이름 기준, 시험 본문 전수 확인 아님): macOS 필수 84건,
  두 OS 모두 약 497건, Linux만으로 충분 약 483건(통합 시험 1,073건 중).
- **첫 Linux 실행** (run 45520042159, 커밋 6c570a4, 배정 대기 3~5초): 6개 행 성공 — `chain-core` 93초,
  `development-product` 103초, `end-history` 108초, `approval` 125초, `end-history-product` 135초, `exploration`
  147초. 의존성 캐시가 없던 첫 실행의 값이다. `recovery-approval` 행은 실패했다: `tests/recovery_approval_mode.rs`가
  파일 전체에 `cfg(any(macOS, Windows))`가 걸려 Linux에서 0건이 되었고 `expect-tests`가 이를 통과로 세지 않았다.
  이 조합(M4-R)은 Linux 행에서 빼고 macOS에 둔다.

## 8. macOS 축소 (2026-11-25)

§7의 Linux 행이 통과한 뒤(커밋 601c991의 실행에서 6개 행 모두 성공) macOS job 두 개를 `chain-macos` 하나로
줄였다. 모든 기능 조합의 lib 전체와 통합 시험은 Linux 행이 돌리고, macOS에는 macOS가 필요한 것만 남긴다.

근거는 시험 본문 전수조사다(저장소 밖 `gil-evidence/ci-os-sensitivity-audit-20261125/`, 시험 2,654건의 분류와
근거 한 줄). 통합 시험 1,080건 중 macOS에서만 컴파일되는 것 19건, OS·파일시스템 동작에 결과가 걸린 것 131건,
두 OS의 제품 코드가 다른 것 88건이며, 마지막 묶음의 64건은 이미지 보고 빌드에서 프로젝트를 시작하는 경로를
지나가기만 한다(그 경로 자체는 `visual_start` 시험이 검증).

| 기능 조합 | macOS에 남긴 검사 | macOS에서 뺀 검사(Linux 행이 실행) |
|---|---|---|
| 기능 없음 | resume 측정 | — |
| `chain-core` | — | 도메인 suite 10개, `revisit` 1건 (조합 전체) |
| `chain-core,visual-authoring,product-safety` | lib `visual_start`, `visual_start`, `chain_resume_benchmark`, `mcp`, 전체 컴파일 | — |
| `approved-plan,visual-authoring,product-safety` | `approved_plan_wire_wave1`, `chain_recovery_preservation`, `closing_handoff`, `resume_guidance`, `manual`, `automatic_guard`, `write_guard`, OS에 닿는 lib 모듈(필터), resume 측정, 전체 컴파일, 제품 빌드를 읽는 Node 검사 2개 | lib 전체, `chain_monitor_fixtures`, `context`, `context_profiles`, 안내 suite 3개 |
| `approved-plan,visual-authoring,recovery-auto-approval` | 세 단계 전부(통합 suite가 macOS·Windows에서만 컴파일) | — (Linux 행 없음) |
| `exploration-flow,visual-authoring,product-safety` | — | 조합 전체(lib, suite 13개, `manual`, 전체 컴파일) |
| `will-end-history` | `will_end_session`, `mcp`, `format_transition` | lib 전체, `will_end`, 도메인 suite 9개, `tracking_record_boundary`, 전체 컴파일 |
| `will-end-history,visual-authoring,product-safety` | `will_end_visual`, `will_end_session`, `format_transition`, `visual_start`, `chain_resume_benchmark`, `mcp`, `chain_handoff`, `visual_attachment` | lib 전체, 전체 컴파일 |
| `will-end-history,approved-plan,visual-authoring,product-safety` | — | `chain_monitor_fixtures` |
| — | Core 없는 Node 검사 전부 | — |

macOS에서 줄어든 검증(이 workflow 기준):

- 조합별 lib 전체 실행 네 번이 없어졌다. lib에는 OS에 닿는 시험이 약 250건 있다. Preview job(기본 조합)과
  제품 후보 job(`tracking-policy,exploration-flow,visual-authoring,product-safety`)은 macOS에서 lib 전체를 계속
  돌린다. 승인 프로필에서는 OS에 닿는 모듈만 이름 필터로 한 번 돌린다(`restore::`, `lock::`, `install::`,
  `launcher::`, `runtime_`, `visual_`, `internal_fs::`, `write_guard::`, `monitor::serve::`, `mcp::bindings::`,
  `session::format_transition::`, `project_resume::`, `companion`). end history 단독 조합에서만 컴파일되는
  OS 민감 lib 시험이 몇 건인지는 세지 않았다.
- 탐색 조합은 이 workflow에서 macOS로 빌드하지 않는다. 같은 suite를 제품 후보 job이 macOS에서 제품 조합으로
  돌린다(정확히 같은 조합은 아니다). 탐색 빌드의 `manual` 1단계는 macOS에서 빠졌다(`manual`은 승인 프로필로 남음).
- "로직만 확인"으로 분류된 시험은 Linux에서만 돈다. 분류가 틀린 시험이 있으면 macOS 결함을 놓친다.
  전수조사 뒤 본 세션이 본문을 다시 읽어 대조한 것은 3건이다.

기존 단계 시간으로 계산한 기대 시간은 약 8분(job 한 개)이었다.

**축소 뒤 실측** (PR #326, run 45521948764, 커밋 d66894e, 모든 job 성공):

| job | 실행 시간 | 배정 대기 |
|---|---|---|
| `chain-macos` | 515초 | 132초 |
| `chain-linux` `chain-core` | 63초 | 4초 |
| `chain-linux` `development-product` | 81초 | 4초 |
| `chain-linux` `end-history` | 104초 | 4초 |
| `chain-linux` `exploration` | 125초 | 4초 |
| `chain-linux` `approval` | 138초 | 4초 |
| `chain-linux` `end-history-product` | 152초 | 3초 |

- Chain workflow의 대기 시간은 분할 전 1034초 → 두 job 600초 → 515초(배정 대기 제외)가 되었고, macOS 사용 시간은
  1034초 → 1191초(두 job 합) → 515초다. 한 번의 실행이며, 이 실행도 workflow 파일이 바뀌어 정확한 캐시 키가
  없었다.
- 이 실행의 macOS 배정 대기(Chain 132초, Preview 861초)는 같은 PR에 연달아 푸시해 취소된 실행들과 겹친 결과다.
  일반 PR의 대기를 대표하지 않는다.
- 두 번째 실행(run 45524783209, main 병합 뒤 커밋 0ccfe70, 모든 job 성공, 배정 대기 27초 이하)에서 `chain-macos`는
  632초였다. 단계 구성은 같고 거의 모든 단계가 조금씩 더 걸렸다(승인 프로필 wire 35→53초, M4-R 세 단계
  75→111초, lib 필터 91→102초). Linux 행은 56~118초였다. 따라서 `chain-macos`의 관측 범위는 515~632초이며,
  두 job 구성(약 600초)보다 대기 시간이 짧다고 단정하지 않는다. 확실한 차이는 macOS job 수(5→4)와 macOS
  사용 시간(두 job 합 1191초 → 515~632초)이다.
- OS에 닿는 lib 모듈 필터 단계는 359건 통과·1건 무시, 91초였다. 최소 실행 수는 처음에 50으로 두었다가 이 실측을
  보고 300으로 올렸다.

## 9. macOS job의 경로 조건 (2026-11-25)

`chain-macos`를 별도 workflow(`.github/workflows/chain-macos.yml`)로 옮기고, PR에서는 macOS에서 다르게 동작할 수
있는 파일이 바뀔 때만 돌게 했다. OS와 무관한 로직만 바꾼 PR의 Chain 검사는 Linux 행(약 1~2분)으로 끝난다.

PR에서 job을 시작하는 경로:

- **OS 조건이 있는 소스 76개**: `cfg`에 `unix`, `windows`, `target_os`, `target_family`가 들어간 `src/**/*.rs`.
  `tools/ci/cache.test.mjs`가 소스에서 이 목록을 다시 계산해 workflow의 목록과 다르면 실패한다. 새 파일에
  `#[cfg(unix)]`를 넣고 목록에 더하지 않으면 그 시험(항상 도는 `chain-node` job)이 막는다.
- **이 job이 돌리는 통합 시험 16개 파일**과 공용 지원 모듈(`tests/common`, `tests/support_wave1`,
  `tests/guidance_support`, `tests/fixtures`).
- `Cargo.toml`, `Cargo.lock`, macOS 빌드를 읽는 도구(`tools/manual-example-check`, `tools/chain-context-bench`,
  `tools/chain-resume-benchmark`), `tools/ci`, workflow 파일 자신.

경로 조건이 놓치는 경우와 그 대비:

- OS 조건이 없는 파일의 변경이 macOS에서만 다른 결과를 내는 경우(예: 경로 문자열 처리, 임시 폴더 경로에
  기대는 로직). PR에서는 잡지 못한다. **main에 병합된 뒤에는 소스·시험 변경마다 이 job이 돈다**(`push` 조건).
  실패는 병합 뒤에 보이므로 PR을 막지 않는다.
- Preview job(필수 검사)은 PR마다 macOS에서 기본 조합의 전체 시험을 그대로 돌린다.
- `automatic_guard`, `write_guard`, M4-R 조합은 Linux 행이 없어 이 job에서만 돈다. 이 suite가 쓰는 OS 조건
  없는 로직이 바뀌면 PR에서는 돌지 않고 main에서 돈다.

함께 옮긴 것:

- Core 프로세스를 띄우지 않는 Node 검사 9단계는 `chain-core.yml`의 `chain-node` job(`ubuntu-24.04`)으로 옮겼다.
  조건부가 된 macOS job에 두면 대부분의 PR에서 돌지 않기 때문이다. Node는 제품 후보 Linux job과 같은 고정
  배포본(v22.23.5, SHA-256 확인)을 쓴다. `chain-context-bench` 계약은 macOS 전용 조건을 떼고 Linux에서 돈다.
- `manual` suite를 승인·탐색 조합의 Linux 행에 더했다(전수조사: 35건 모두 Linux에서 컴파일·실행). macOS job의
  `manual` 단계는 그대로다.

이 절을 쓰는 시점에 `chain-node` job과 Linux의 `manual`은 원격에서 실행된 적이 없다.

## 10. 시험 스레드: 순차가 필요한 묶음만 순차로 (2026-11-25)

§9 이후 일반 PR의 대기는 필수 검사인 Preview job이 정한다. 그 job의 마지막 실행(PR #326의 마지막 Preview 실행, 380초)은 컴파일
약 156초, 시험 실행 약 174초, Node 검사 약 30초였고, 시험은 전부 `--test-threads=1`이었다(lib 1,286건 55초,
`monitor` 약 21초, `cli` 약 21초).

`--test-threads=1`의 이유로 적혀 있던 것은 fork 기반 실행기가 다른 fixture의 잠금을 물려받는 문제다. 프로세스를
직접 복제하는 코드는 `src/runtime_launch.rs` 한 곳이고, 기본 조합에서는 lib 시험 실행 파일 안에서만 실행된다.

로컬 반복 실행(macOS arm64 8코어, CI와 같은 빌드 설정, 기본 조합):

| 실행 | 결과 |
|---|---|
| lib 전체 순차(기존) | 1,286건 통과, 62초 |
| lib 병렬, `runtime_`만 제외 | 5회 중 1회 실패: `install::file::tests::store_backs_up_replaces_and_refuses_a_concurrent_change` |
| lib 병렬, `runtime_`·`install::` 제외 | 20회 중 1회 실패: `monitor::serve::tests::an_artifact_change_reaches_the_screen` |
| lib 병렬, `runtime_`·`install::`·`monitor::serve::` 제외 | 25회 모두 통과, 1,113건, 약 7.5초 |
| lib 순차, 위 세 묶음만 | 173건 통과, 38초 |
| 통합 시험 전체, 기본 스레드 | 4회 모두 통과, 55~56초 (lib을 함께 돌린 3회에서도 통합 시험 실패 없음) |
| 통합 시험 전체, 순차(기존) | 약 107초 (lib 포함 169초에서 lib 62초를 뺀 값) |

- `install::file`의 실패는 이전에 가끔 보이고 재현하지 못했던 설치 잠금 실패(PR #184)와 같은 시험이다. 병렬에서
  재현되며 원인은 여기서 고치지 않았다. `monitor::serve`에는 실제 파일 감시기의 알림을 시간 안에 기다리는
  시험과 프로세스 전체의 신호 처리기를 바꾸는 시험이 있다.
- 승인 프로필의 OS 민감 lib 묶음(§8)도 같은 방식으로 나눴다: 병렬 부분 185건 12회 모두 통과(약 3초), 순차 부분
  174건(약 38초). 합은 기존 한 단계와 같은 359건이다.

적용:

- **Preview, 기본 조합**: `cargo test -p gil -- --test-threads=1` 한 줄을 lib 병렬(위 세 묶음 `--skip`), lib 순차(같은
  세 이름 필터), `--bins`, `--test '*'`(기본 스레드), `--doc`로 나눴다. `--skip`과 이름 필터가 같은 문자열을 쓰므로
  두 lib 실행은 서로의 여집합이다. lib 두 실행에는 `expect-tests` 최소 수(1000, 150)를 걸었다. 수동 실행 전용인
  다른 프로필의 명령은 바꾸지 않았다.
- **`chain-macos`**: OS 민감 lib 단계를 병렬 단계와 순차 단계로 나눴다(각 최소 150).
- Linux 행의 lib 전체는 순차 그대로다(대기를 정하지 않고, Linux의 파일 감시 방식이 달라 따로 확인해야 한다).

한계: 반복 횟수는 lib 25회, 통합 4회다. 이보다 드문 실패는 배제하지 못한다. CI 서버는 3코어라 단축폭은 로컬보다
작다. 원격 실측은 이 변경의 PR에서 얻는다.

