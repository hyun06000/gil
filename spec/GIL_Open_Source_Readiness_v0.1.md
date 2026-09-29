# GIL Open Source Readiness v0.1

상태: 독립 marketplace 전환·기존 Host 인수 완료, 새 Skill 후보 및 공개/게시 게이트 준비 (2026-09-29)

## 1. 결정과 현재 사실

새 정본 저장소의 이름은 **gil**이다. [hyun06000/gil](https://github.com/hyun06000/gil)은
비공개로 만들었고, 선별한 현재 소스의 독립 root commit을 push한 뒤 첫 CI를 검수했다.
기존 개발 저장소를 fork하거나 그 branch·tag·과거 commit을 옮기지 않는다.
[MIT LICENSE](../LICENSE)와 기존 저작권·제3자 고지는 그대로 보존한다.

이는 기존 공개 이력의 삭제·정리·회수를 뜻하지 않는다. 기존 저장소와 그 미커밋 문서는
보존하며, 과거 개인 개발 경로·작업 기록의 정리 대상과 영향은 별도 검토한다.
새 저장소를 만든 것만으로 기존 공개 사본의 문제가 해결됐다고 말하지 않는다.

2026-09-28 추가 승인으로 기존 개발 저장소도 비공개로 전환했다. 공개 fork·기존 clone의 회수는
아니다. 두 저장소의 main은 PR 전용 운영으로 바꾸며, 비공개 저장소 보호 규칙은 요금제 제한으로
서버 적용이 막혀 있다 ([설정 장부](../distribution/compliance/REPOSITORY-GOVERNANCE-20260928.md)).

이후 유지관리자는 **배포 준비 완료 → 새 gil 공개 → 정식 배포** 진행을 승인했다. 지금 공개하거나
미완료 검수를 면제한 것은 아니다. 현재 비공개·PR·CI·승인 후 병합 원칙으로 준비한다.
기존 저장소는 비공개로 유지한다. 계정 가입·결제·credential 등록은 별도 사용자 조치다.

이후 첫 배포를 **Codex/macOS arm64의 Apple 개발자 서명·공증 없는 opt-in 시험판**으로 준비하기로
승인했다. 새 Mac 검수는 명시적으로 유예하며 통과로 세지 않는다. 공개 표면 감사와 PR·CI·게시
승인 경계는 유지한다. 정식 신뢰 채널과 Companion 배포 기준은 별개다.

9/29 추가 결정으로 개인정보가 남은 `gil-distribution`은 비공개로 보존하고, 독립 이력의
`hyun06000/gil-marketplace`를 새 배포 출처로 채택했다. PR #1 병합과 공식 Host의 출처 전환,
preview.1 동일 bytes·fullscreen·노드 상세를 확인했다
([새 경로 장부](../distribution/codex/CLEAN-MARKETPLACE-CHECKPOINT-20260929.md)).
현재 source `gil`과 active marketplace 모두 비공개이며 공개를 실행한 것은 아니다.

| 단계 | 현재 범위 | 아직 의미하지 않는 것 |
|---|---|---|
| 새 저장소 | 비공개 `hyun06000/gil`, 준비 후 공개·배포 순서 승인 | 지금 즉시 공개·정식 출시 가능 |
| 소스 선별 | 현재 코드·시험·명세·고지, 개인 기록과 옛 감사 보고서 제외 | 전체 공개 표면 감사 완료 |
| 라이선스 | 기존 MIT 및 제3자 원문 고지 보존 | 모든 배포 target의 권리 검수 완료 |
| 제품 검수 | 이전 구현 인수·새 checkout 검증·새 CI를 구분 | 새 기계 설치·정식 출시 |

이전 저장소의 CI 실행 번호나 감사 결과를 새 저장소의 성공으로 승계하지 않는다.
선별 기준과 이번 실행 결과는 [SOURCE-MIGRATION.md](../SOURCE-MIGRATION.md)에 기록한다.

## 2. 공개 전 게이트

- [x] 저장소 이름 `gil`, 비공개 준비, 기존 이력·설정·설치본 보존 경계 확정.
- [x] MIT LICENSE와 필요한 제3자 원문 고지 보존.
- [x] README·CONTRIBUTING·개발 지침에서 개인 bootstrap 및 옛 실행 환경 의존 제거.
- [~] 선별된 첫 source tree의 비밀·개인정보·fixture·이미지·문서·링크 감사.
  패턴 검색 결과를 전체 보안 감사로 확대하지 않으며, 시험용 유출 방지 문자열과 실제 값을 구분한다.
  [초기 9/29 감사](../distribution/compliance/PUBLIC-SURFACE-AUDIT-20260929.md)의 배포 commit 개인
  이메일 문제는 옛 저장소 비공개 보존·독립 marketplace 경로로 해결했다. 후속 source/marketplace
  이력·CI·PR·artifact 감사 범위와 source의 vendor co-author 예외는
  [새 장부](../distribution/codex/CLEAN-MARKETPLACE-CHECKPOINT-20260929.md)에 구분한다.
  새로운 source/후보/ref도 검수하며 이전 시점의 검사를 전체 보안 보증으로 확대하지 않는다.
- [x] 독립 marketplace의 no-reply Git 이력·PR/CI·동일 payload·공식 설치 출처 전환 검수.
  옛 배포 저장소는 비공개 복구 이력으로 보존하고 force push·삭제하지 않았다.
- [~] macOS arm64 native Core 79개 + 실제 UI bundle 5개 package 및 Rust 표준 라이브러리
  고지 조립·원문/coverage/lock 변경 거절 구현 보존.
  [고지 검증](../distribution/compliance/README.md)은 Companion·다른 target까지 보증하지 않는다.
- [x] 새 저장소의 독립 macOS arm64 preview CI와 source↔artifact·고지·실행 권한 대조.
  clean `fbc4100`, 35개 시험과 17 tools 검수 ([실행·다운로드 증거](../distribution/codex/CI-CHECKPOINT-20260928.md)).
  전체 Core/UI suite나 Host 설치 시험을 원격에서 수행했다는 뜻은 아니다.
- [~] 유지관리자 기여 정책·Code of Conduct·issue/PR 안내 및 연락 경로 검토.
  [기여 안내](../CONTRIBUTING.md)·[SUPPORT](../SUPPORT.md)·issue/PR 양식을 준비했다.
  [행동 강령](../CODE_OF_CONDUCT.md)은 초안이며 담당자 검토와 민감한 신고 경로는 미결이다.
- [ ] 실제 비공개 보안 보고 경로 결정 후 SECURITY.md 게시.
  [준비 계획](GIL_Security_Reporting_Plan_v0.1.md)에서 공개 시 GitHub PVR 사용을 결정했다. 공개 저장소용이므로
  현재 비공개 저장소에서 개통됐다고 표시하지 않는다. 보안 설정은 자동 변경하지 않고,
  없는 이메일·응답 기한을 약속하지 않는다.
- [~] main의 PR 전용 변경 정책 — 개발 지침 반영. 비공개 기간에는 운영 규칙을 지키고,
  새 gil 공개 시 지원되는 서버 보호를 적용·재조회한다. Pro 가입을 임의로 진행하지 않는다.
- [ ] 미결과 공개 표면 검토를 완료한 뒤 조건부 승인에 따라 공개 전환하고 보호·PVR를 검증한다.
- [ ] unsigned 시험판의 공개 source 감사·CI 출처·고지·불변 version·설치/복구 안내를 검수한 뒤,
  승인한 동일 bytes를 marketplace 경로에 게시한다. 새 Mac 검수는 유예 상태로 공개 고지한다.
  서명·공증·새 기계 설치를 갖춘 정식 신뢰 채널은 별도 후속 게이트다.
  기존 Mac의 개발판→preview 교체·제거/재설치·Project 보존·화면은
  [Host 장부](../distribution/codex/HOST-INSTALL-CHECKPOINT-20260929.md)와 위 출처 전환 장부로 확인했다.
  Companion 선택 사항·Codex-only 범위를 바로잡은 Skill은 새 preview.2로 검수한다. 기존 설치본은
  preview.1 그대로이며 새 version 인수로 세지 않는다. 공개 게시 주소·설치와
  실제 이전 판 rollback 인수는 미완료이며,
  [native release gate](../distribution/codex/RELEASE-macos.md)를 따른다.
  직접 등록한 MCP 화면 성공은 Plugin 설치 경로 성공이 아니다.

준비된 [게시 순서와 사용자 안내](../distribution/codex/PUBLICATION-PLAN-20260929.md)는 초안이다.
저장소 공개·보호/PVR 설정·게시를 실행하지 않았다. Skill source 보정은 기존 설치본/후보의
덮어쓰기를 승인하지 않으며 새 후보·배포 PR·설치 인수를 각각 거친다.

## 3. 검증과 승인 경계

로컬 검증은 Core/MCP, JS/UI, packaging, 고지, 선택적 Companion을 구분한다.
프로토콜 응답은 실제 fullscreen 화면의 증거가 아니고, Core 시험은 Tauri 검수의 대체가 아니다.
Windows와 Claude 작업 Plugin의 미결도 macOS/Codex의 이전 성공으로 닫지 않는다.

원격 push, CI 실행, 공개 전환, Plugin 설치/재설치, 서명 credential, release/marketplace 게시는
각각 승인된 범위에서 진행한다. 공개 전환 전에 소스뿐 아니라 CI 로그·artifact도 검수한다.
비밀 값은 저장소·보고서·공개 issue에 복사하지 않는다.

이미 공개된 비밀이 발견되면 소유자에게 폐기·교체 필요를 알리고 승인된 범위에서 정리한다.
파일 삭제나 저장소 비공개 전환만으로 fork·clone까지 회수됐다고 주장하지 않는다.
이력 재작성·force push·기존 저장소 삭제는 이번 이전의 범위가 아니다.

## 4. 관련 문서

- [이전 범위와 새 검증 장부](../SOURCE-MIGRATION.md)
- [Roadmap M5-G](GIL_Roadmap.md)
- [Distribution Model §6.3](GIL_Distribution_Model_v0.1.md)
- [개발 preview 패키징](../distribution/codex/README.md)
