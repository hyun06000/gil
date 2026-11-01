# 문맥·프로필 fixture 후속 검증 — 2026-11-02

소스: `309007c` (`review/claude-wave2-20261102`). 이전 [통합 검토](REVIEW.md)의 context 14개 실패를 해결했다. 실행 명령·수치·제한은 [context-validation.json](context-validation.json)에 있다.

## 무엇을 고쳤는가

- 기존 `tests/context.rs`의 branch/revisit 투영 시나리오는 승인 계획 도입 이전의 계약이다. 이제 format 4, 또는 Chain 빌드에서는 승인 ledger가 없는 format 6을 **합성 fixture로 명시적으로 작성하고 실제 loader로 읽는다**. 기존 15개 시험의 assertion을 유지하며, 조건부 성공·ignore로 숨기지 않는다.
- 공통 역사 기록 helper를 `tests/common/legacy.rs`로 추출했다. 빈 합성 프로젝트만 받으며 기존 state 파일이 있으면 재작성하지 않는다. 기존 guidance 시험도 같은 format-4 helper를 사용한다. 제품의 새 프로젝트 생성 방식이나 공통 bootstrap의 승인 검사는 바꾸지 않았다.
- `context_profiles`는 역사 기록을 읽고 이어 써도 format·Chain 포함 관계·승인 ledger 부재가 보존되는지 검사한다. 별도로 새 프로젝트의 실제 프로필을 확인하고, 승인 프로필에서는 맨 `approved: yes`가 거절되며 거절 전후 저장 bytes가 같은지 검사한다. 기본/Chain 프로필의 기존 승인 계약도 검증한다.
- 실제 stdio 승인 시험에 제안 대기·승인·Experiment·Verify·Cycle 종료·재인터뷰 요청 후 Context를 두 번 읽는 검증을 추가했다. 두 응답과 저장 bytes가 같아야 하며, 종료 후 서버를 바꿔도 동일한 문맥이 복원되어야 한다.

## 회귀로 발견한 제품 안내 오류

`gil_plan_read`의 닫힘 안내는 이전 통합에서 수정했지만, 뒤이어 읽는 `gil_context` 본문에는 여전히 과거 항목의 파일 저장 지시가 남아 있었다. 첫 다음 행동은 새 Experiment를 가리키는데 아래 문장은 “이번 Experiment: A / Verify 전 파일 저장”을 지시하는 모순을 실제 stdio 시험으로 재현했다.

현재는:

- 닫힌 Cycle의 항목은 **닫힌 Experiment의 실행 연결**로 표시한다.
- 계획 문서·증거 파일 저장 안내는 **실제로 열린 Verify에서만** 출력한다. Verify를 열기 전 파일 작업을 시키지 않는다.
- 재인터뷰 요청이 있는 Cycle이 이미 닫혔다면 새 Interview를 안내한다. 닫힌 Cycle의 남은 Step을 다시 마치라고 하지 않는다.

승인·저장·실행 권한은 그대로이고 변경된 제품 코드는 Context 투영뿐이다.

## 검증 결과

| 검사 | 결과 |
|---|---|
| 기본 Core 전체 | **1,518 통과 / 0 실패 / 2 ignored** |
| 승인 프로필 lib + context + context_profiles + guidance_wave1/2 + 실제 stdio 승인 시험 | **704 통과 / 0 실패 / 1 ignored** |
| Chain 프로필 context/context_profiles/guidance_wave1/2 | **25 통과 / 0 실패** |
| UI·Plugin JS | **147 통과 / 0 실패 / 4 skipped** |
| 배포·고지 | **96 통과 / 0 실패 / 1 skipped** |
| 기본·승인 프로필 all-target build | 통과 |
| diff whitespace 검사 | 통과 |

기존 ignored/skipped를 늘리지 않았다. 새 회귀는 macOS/Windows `chain-core.yml`의 승인 프로필 실행 목록에 넣었고, Chain 프로필에도 `context_profiles`를 추가했다. 기존 컴파일 unused/dead-code 경고는 남아 있다.

## 아직 주장하지 않는 것

명세는 모든 기존 integration target에 승인 프로필을 일괄 적용한 실행을 인수 기준으로 정하지 않는다. 다른 suite에 남아 있는 legacy bootstrap이나 product-safety 거절 가정까지 이번에 전부 변환한 것은 아니다. 이전 E 보고서의 509 실패를 현재 값으로 재사용하지 않았으며, 전체 승인 프로필 aggregate가 모두 녹색이라고 주장하지 않는다.

Windows 네이티브 CI/실기, 실제 Host 표시·사용자 Monitor 버튼·Luna 작업은 미실행이다. 원격 push/PR/병합/배포도 하지 않았다. 다음 인수는 원격 양 OS CI와 사용자 실사용 관찰이며, 이번 소스 변경은 설치된 앱에 아직 반영되지 않았다.
