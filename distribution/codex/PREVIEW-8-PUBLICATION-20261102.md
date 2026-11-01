# Monitor 승인·Chain 시험판 게시 장부 — 2026-11-02

대상은 친구·테스터용 Mac Apple Silicon Plugin `0.2.1-preview.8`과 Windows 11 x64
Companion `0.1.1-owner.7`이다. 사용자 요청은 PR 생성, 양 OS CI 확인, 통과 후 병합과 시험판 게시다.
Luna 실사용은 사용자가 진행한다. 정식 공개판·OS publisher 서명·자동 업데이트로 승격하지 않는다.

## 소스와 배포 계약

- [소스 PR #64](https://github.com/hyun06000/gil/pull/64) 병합: `090284f2cf3ee58fc233a36ae6dd7a1dbeb58c6b`.
- 최종 PR CI는 Mac/Windows의 모든 실행 항목을 통과했다. dispatch 전용 게시 작업의 skip은 별도다.
- 두 후보의 실제 빌드 소스: `ca7c60959914f7ea8dc4ff7a125628b50ffea698`.
- 최종 PR head `c3a313af53c9bbc1bf873b6647aa1f4bdae7c3f7`는 위 소스의 자손이며, 차이는
  `tools/scenarios/` 평가 도구 다섯 파일뿐이다. 설치되는 Core·UI·Skill·의존성·패키징 코드는 동일하다.
  빌드가 merge commit에서 수행됐다고 주장하지 않는다.
- 명시적 `approved-plan,visual-authoring,product-safety` 프로필. Windows Companion의 내장
  `gil` 라이브러리도 같은 프로필이다. 기존 기본 빌드 프로필은 자동 전환하지 않았다.
- 새 프로젝트는 형식 7의 Chain·승인 계획을 사용한다. 기존 기록의 형식은 자동 변환하지 않는다.
  이전 Plugin/app으로 복귀해도 기록 형식은 내려가지 않는다.
- 제품의 destructive restore/revisit는 차단한다. M6-E 설계·추적 정책 엔진·읽기 전용 preview는
  실제 복구 적용이나 migration 완성을 의미하지 않는다.

## CI와 발견한 문제

첫 Windows 승인 프로필 전체 라이브러리 실행에서 12개 실패를 발견했다. Unix 절대경로 fixture,
state 파일 없는 inline 디렉터리 fixture, Windows SystemTime 범위를 벗어난 테스트 시각을 수정했다.
경로·identity·숫자 wire 안전성 assertion은 유지하며 ignore를 추가하지 않았다.
후속 실행에서는 Core 승인이 통과한 뒤 평가 도구의 Unix 셸 fixture, file URL 처리, JSON으로
escape된 Windows 경로 검사가 실패했다. 직접 인자 배열 실행·fileURLToPath·직렬화 경로 검사를
수정하고 양 OS CI를 다시 실행했다.

| 검사 | 실행/결과 |
|---|---|
| 최종 Mac/Windows Chain·M6-D·평가 도구 | [41334962898](https://github.com/hyun06000/gil/actions/runs/41334962898) |
| 최종 macOS 기본 배포 계약 | [41334962984](https://github.com/hyun06000/gil/actions/runs/41334962984) |
| 최종 Companion Mac/x64/ARM64 | [41334962985](https://github.com/hyun06000/gil/actions/runs/41334962985) |
| 최종 Windows x64/ARM64 default/visual | [41334963046](https://github.com/hyun06000/gil/actions/runs/41334963046) |
| 최종 Windows portable bundle | [41334962809](https://github.com/hyun06000/gil/actions/runs/41334962809) |
| Mac 승인 프로필 후보 | [41334225234](https://github.com/hyun06000/gil/actions/runs/41334225234) |
| Windows 승인 프로필 설치 후보 | [41334227859](https://github.com/hyun06000/gil/actions/runs/41334227859) |
| 로컬 기본 Core | 1,518 통과 / 0 실패 / 2 기존 ignored |
| 로컬 승인 라이브러리 | 678 통과 / 0 실패 / 1 기존 ignored |
| 최종 평가 도구와 Skill 계약 | 62 통과 / 0 실패 |
| 배포·Windows 설치 계약·고지 | 116 통과 / 0 실패, 실제 Mac archive 포함 |

전 승인 프로필 legacy integration aggregate 통과나 모든 미래 플랫폼 지원을 주장하지 않는다.

## Mac 배포물

- [배포 PR #12](https://github.com/hyun06000/gil-marketplace/pull/12) 병합 및 고정 tag commit:
  `116bc49f4a464f75c81400fb2ff3d18091b50706`.
- [고정 prerelease](https://github.com/hyun06000/gil-marketplace/releases/tag/v0.2.1-preview.8).
- archive: `gil-codex-macos-arm64-0.2.1-preview.8.tar.gz`, 3,435,833 bytes.
- 게시 뒤 인증 없는 공개 HTTPS 다운로드의 크기·해시가 CI 원본과 일치했다.
- SHA-256: `5fcfbb2a328ae65c71d6b5aa4432a556d89429d0fe4c98cacb855e9c2a88d266`.
- Core: `c986982fda1e9cb1fca07d8c9be152d429c56ad9782914c460e64cd3ae749f42`.
- UI: `1b29af14e7cb55045098cefab940cdefff0f5e8fabf8ef7550cf636517a8cccd`.
- receipt: `f2755f879e8f803c7f70b68f08b92a440083d5cba82b981c3934796c25122675`.
- 원본 CI archive/receipt/고지/실행 권한을 보존했다. Codex와 Claude catalog가 같은 Core·Skill을 가리킨다.
  receipt의 build-time `publishable:false`·신뢰 표시는 게시 승인에 맞춰 고쳐 쓰지 않았다.
- 정확한 Core wire 시나리오: 20 통과 / 1 예상 product-safety 차단 / 0 실패.
- 격리 설정의 앱 번들 Codex CLI `0.166.0-alpha.5.1`로 로컬 marketplace 등록·Plugin 설치·버전 조회와
  설치된 Core 해시 일치를 확인했다. 실제 사용자 설정·cache·기록을 변경하지 않았다.
- 게시 후 별도의 격리 설정에서 공개 Git tag `v0.2.1-preview.8`로 marketplace 등록·Plugin 설치를
  다시 확인했다. 조회 버전과 설치된 Core 해시는 동일했다. 실제 Host fullscreen 검수는 아니다.
- Git 없는 준비 스크립트는 정확한 archive로 7개 검사 통과. 무결성/변조 방어 19개와 raw Git history
  개인정보 검사 9개를 통과했다. 정적 검사는 OS publisher 인증이 아니다.

## Windows 배포물과 업데이트

- [고정 prerelease](https://github.com/hyun06000/gil/releases/tag/windows-owner-update-0.1.1-owner.7).
- installer: `GIL.Companion.Unsigned.Test._0.1.1-owner.7_x64-setup.exe`.
- 크기: 5,919,030 bytes. SHA-256: `0b690218e67370495544174a45b2658c2a99e3baa2a8040e70c61268be0cee96`.
- 설치된 Core: `71f081ed680e771fa4d072b96490a891baa4ea3b4cf7b13435f2dd8981358c0e`.
- CI는 설치·같은 버전 재설치·보이는 Companion 창·공식 Codex CLI 연결/해제·기존 MCP 연결 유지와
  다음 연결의 새 Core 선택·제거 후 runtime 보존을 통과했다. Core wire 20 통과 / 1 예상 차단 / 0 실패.
- 게시 뒤 인증 없는 공개 다운로드 크기·해시가 원본과 일치했다. GitHub용 파일명만 정규화했으며
  installer와 signature bytes는 바꾸지 않았다. 원본 이름은 첨부 CI evidence에 남아 있다.
- 공개 release에 `.exe`, `.sig`, `SHA256SUMS`, `unsigned-updater-acceptance.json`, `wire-run.json`을 첨부했다.
  서명의 trusted comment가 `version:0.1.1-owner.7`을 포함하는지 확인했다.
- Authenticode는 없다. 버전이 결속된 updater 서명과 `requireSignedVersion`은 유지한다.
- 공개 설치 파일을 검증한 뒤 `distribution/windows/owner-update.json`을 새 버전으로 바꾸는 PR을 제출한다.
  사용자가 업데이트 확인·업데이트 버튼을 누를 때만 적용한다. 기존 MCP 연결은 이전 Core를 유지하고
  새 연결부터 새 Core를 선택한다.

## 사용자가 직접 확인할 항목

새 대화·새 빈 폴더에서 Luna로 수행한다. 기존 프로젝트를 실험용으로 이관하거나 지우지 않는다.

1. 시작 직후 Codex의 실제 Monitor가 보이는가. Companion 표시와 구분한다.
2. Interview의 마지막 제안을 Monitor에서 읽고 승인 버튼을 누른 뒤 Interview 전체가 닫히는가.
3. 독립된 두 질문 이상이 각각의 계획 항목과 Experiment로 진행되는가.
4. Verify를 먼저 열고 계획 문서와 별도 실제 파일을 남기는가. 채팅만 출력하고 끝내지 않는가.
5. Analysis·Outcome의 해석과 배움이 실제 증거에 연결되고 Cycle이 닫히는가.
6. 새 세션에서 상태와 다음 행동을 정확히 복원하는가.

프로토콜 성공·합성 App 입력은 화면 표시나 실제 사람의 동의가 아니다. 이번 버전의 실제 Host
fullscreen·승인 버튼·Luna 결과·깨끗한 VM 인수는 사용자가 확인하기 전까지 미검수다.
