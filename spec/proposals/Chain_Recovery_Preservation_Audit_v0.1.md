# Chain Recovery Preservation Audit v0.1 — 검증 보고

상태: **검증 보고.** 2026-11-16, 테스트 전용 작업이다. 제품 코드·저장 형식·기존 시험·workflow·
Skill·로드맵을 바꾸지 않는다. 규범은 [M4-R 승인 복구 계약](../GIL_Revisit_Recovery_v0.1.md),
[Artifact Model](../GIL_Artifact_Model_v0.1.md) §8·§9, [Roadmap](../GIL_Roadmap.md) §7 M3·§9 M4-R이다.
Artifact Model §3.6의 `.gilignore` 설계([PR #128](https://github.com/hyun06000/gil/pull/128))는
미승인·미구현이므로 여기서 시험하지 않는다. 일반 `restore`와 Step revisit 차단은 그대로다.

## 1. 질문

Chain 도중 성공한 Experiment 위에서 다음 Experiment가 실패하고 `revisit → 성공 Cycle`을 선언했을 때,
현재 **승인된 복구 경로**(`gil_revisit_review` / `gil_revisit_apply` / 재개용 `gil_revisit`)가
다음 세 종류의 파일을 기존 명세대로 다루는가.

| 종류 | 명세가 요구하는 처리 |
|---|---|
| 부분 결과 (실패 Cycle이 덮어쓴 기존 파일) | 대상 세계 바이트로 교체, 원본은 `.gil/revisit-preserved/<key>/`에 보존 |
| 현재에만 있는 파일 (실패 Cycle이 만든 파일, 중첩 폴더 포함) | 작업 폴더에서 제거(`preserve`), 보존본에 남김, 비게 된 폴더 제거 |
| 제외·보호 경로 (`.gitignore` 제외, `.env` 등 기본 보호) | 변경 없으면 목록에 없고 건드리지 않음; 어느 한쪽 정책이 제외하는 경로가 다르면 **전체 무변경 거절** |

기존 `tests/support_wave1/revisit_cases.rs`는 대상이 항상 뿌리 Interview `C1`인 경우만 다룬다.
성공 Cycle `C2`의 세계로 돌아가는 Chain 모양, 삭제된 파일의 재생성, 변경 없는 제외·보호 경로,
target 쪽 `.gitignore`가 넓어진 경우, 보존본 전체 대응, 완료 후 재시작 경계는 시험이 없었다.

## 2. 추가한 시험

`tests/chain_recovery_preservation.rs` — 실제 제품 프로필 `gil mcp --serve` stdio, 합성 임시 폴더만 사용.
cfg: `approved-plan` + `product-safety` + `visual-authoring` + macOS/Windows (기존 복구 시험과 동일).

공통 fixture: 기준 파일 `README.md`·`data/source.csv`·`.gitignore`(`scratch/`)·`scratch/cache.bin`·`.env`
→ `C1` Interview(계획 A/B/C 승인) → `C2` Experiment A 성공(`results/a.txt` 등 확정)
→ `C3` Experiment B 실패: `results/a.txt`·`README.md` 덮어쓰기, `results/b/partial.csv`·`evidence/C3-B.md` 생성,
`data/source.csv` 삭제, `revisit → cycle:C2`.

| 시험 | 고정하는 것 |
|---|---|
| `recovery_in_a_chain_returns_to_the_successful_cycle_and_preserves_every_failed_file` | review는 읽기 전용(기록·파일·보존본 무변경), 변경 목록이 정확히 replace 2·preserve 2·create 1이며 변경 없는 제외·보호 파일은 목록에 없음. apply 뒤 작업 폴더 == `C2` 종료 세계(바이트 전수), 빈 폴더 제거, 변경 없는 파일의 mtime 불변. 보존본은 before 세계 **모든** 파일을 `file-N`·경로·SHA-256으로 대응하고 `approval.json`에 토큰이 없음. 객체 저장소·Will/Existence·승인 판·닫힌 Cycle 전부 불변, `pending_cycle_revisit = 3`. 같은 승인 재호출은 멱등(보존본 1개). 다음 `gil_open`은 `C4(parent C2, revisit_from C3)`이며 항목 B가 다시 pending. |
| `an_excluded_path_differing_in_either_world_refuses_the_whole_recovery_unchanged` | (a) 실패 Cycle이 `.gitignore` 제외 폴더에 파일을 만든 경우, (b) 실패 Cycle이 `.gitignore`를 넓혀 target이 추적하는 `results/`를 제외한 경우 — 두 경우 모두 review가 보호 경로를 이름 붙여 거절하고 토큰을 발급하지 않음. 기록·작업 파일·객체·보존본·restore 영역 전부 무변경, 논리 이동 없음, 부분 결과는 사용자가 둔 그대로. |
| `after_completion_a_restart_is_a_no_op_and_a_later_file_is_refused_not_swept` | 완료 뒤 새 프로세스의 `gil_revisit`은 `completed` 수렴·파일/기록/보존본 무변경, review도 같은 digest의 `completed`. 완료 뒤 사용자가 만든 파일은 `gil_revisit`이 "기록되지 않은 파일 변경"으로 거절하고 삭제·보존본 추가 없음. 그 상태에서도 `gil_open`은 `C4`를 열며 작업 파일을 건드리지 않는다. |

변이 확인: `src/session/revisit_recovery.rs`의 보호 경로 거절(`!includes(...)`)을 임시로 꺼 보면
두 번째 시험이 "expected a refusal but the call succeeded"로 실패한다(확인 후 원복, 커밋하지 않음).

## 3. 결과

| 검사 | 결과 |
|---|---|
| 새 시험 3개 (`approved-plan,visual-authoring,product-safety`) | §4의 실행 기록을 따른다 |
| 기존 복구 시험 `approved_plan_wire_wave1` (revisit_cases 6개 포함) | §4 |
| 기존 restore 단위 시험 `--lib restore::` | §4 |
| 기본 프로필·chain-core 프로필에서 새 파일 컴파일 (cfg로 0개 실행) | §4 |

**현재 계약 안의 결함은 찾지 못했다.** 부분 결과·현재에만 있는 파일·제외 경로·보존본·멱등·재시작 경계가
명세대로 동작했다.

## 4. 실행 기록

PR 설명과 CI 결과를 정본으로 한다. 로컬은 macOS arm64, Rust 1.97.1, 독립 worktree, 공유되지 않은 `target/`.

## 5. 관찰 (결함 아님, 후속 결정 대상)

1. **보존본이 보호·제외 파일까지 복제한다.** 계약대로 before 세계의 모든 파일을 `file-N`으로 저장하므로
   `.env`·`scratch/cache.bin`도 `.gil/revisit-preserved/<key>/`에 0600으로 복제된다(변경이 없어도).
   현재 legacy whole-tree 계약에는 맞지만, §3.6 `.gilignore` 설계가 "제외 파일을 격리 영역으로 이동하지
   않는다"고 적으므로 새 정책 경로를 구현할 때 보존 범위를 **추적 대상에 한정할지** 결정해야 한다.
   이 세션은 그 결정을 내리지 않으며, 새 시험은 현재 계약(전체 대응)을 고정한다. PR #128 범위에서 다룬다.
2. **CI 편입이 필요하다.** `chain-core.yml`과 `codex-preview.yml`의 approved-plan 프로필은 시험 바이너리를
   `--test` 이름으로 열거하므로 새 파일은 CI에서 **컴파일만** 되고 실행되지 않는다(기본 프로필 전체 실행은
   cfg로 0개). 후속: `chain-core.yml`의 "Approval profile … (Wave 1 E wire)" 단계에
   `--test chain_recovery_preservation`을 추가한다. workflow 수정은 이 세션 범위 밖이라 하지 않았다.
3. **완료 뒤 추가 파일의 거절 문구.** 복구가 이미 끝난 상태에서 사용자가 파일을 만든 뒤 `gil_revisit`을
   다시 부르면 "기록되지 않은 파일 변경이 있어 복원을 거절했다"가 나온다. 파일은 보존되고 `gil_open`은
   정상 진행되므로 안전하지만, 이미 완료된 복구임을 함께 말하면 Agent가 재시도를 반복하지 않는다.
   문구 조정은 i18n을 다루는 M6-E 세션과 겹치므로 그쪽 후속으로 넘긴다.

## 6. 하지 않은 것

- 사용자 실제 프로젝트·설치본·Monitor 클릭·Windows 실기기 인수. 합성 App 입력은 사람의 승인 증거가 아니다.
- `.gilignore` 정책 경로, 자동 승인 모드(명세만 있고 구현 대기), Chain-level revisit, 일반 restore 해제.
- 복원 중 프로세스 종료 재개는 기존 `approved_recovery_survives_state_save_then_process_death_during_restore`가
  이미 다루므로 중복하지 않았다.
