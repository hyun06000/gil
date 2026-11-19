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
- **job 분할·서버 종류 변경·플랫폼 삭제**: 대상 아님. 분할은 경과 시간을 줄여도 서버 대기·총 사용 시간·캐시
  저장을 늘린다(특히 macOS arm64 배정 대기 5분 이상, 병합 후 main에서 미배정 취소).
- **다른 연결기(`rust-lld`)**, **`registry/src` 캐시 제외**, **workflow 문구와 캐시 호환성 분리**: 측정하지
  않았다.
