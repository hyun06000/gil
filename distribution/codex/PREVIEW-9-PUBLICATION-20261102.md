# Monitor 승인 복구·증거 보존 시험판 게시 장부 — 2026-11-02

대상: 친구·테스터용 macOS Apple Silicon `0.2.1-preview.9`, Windows 11 x64
`0.1.1-owner.8`. 사용자 요청은 두 OS 동작 통일과 배포다. 설치와 실제 Luna 시험은 별도다.
정식 OS publisher 서명이나 무인 자동 업데이트로 승격하지 않는다.

## 같은 사용자 계약

닫힌 실패 Cycle → Monitor의 전체 변경 목록 → 실제 사용자의 **보존 후 되돌아가기** →
원본·승인 receipt 보존 → 선택한 과거 파일 상태 복원 → 실패 계보를 잇는 새 시도다.
실패 Report·Will·Snapshot·승인 이력을 지우거나 성공으로 바꾸지 않는다. 기존 기록 schema와
Snapshot identity를 바꾸지 않는다. 보존본은 실제 기록 저장소의
`.gil/revisit-preserved/<pending-state-sha256>/`에 남으며 자동 삭제하지 않는다.

App 전용 도구와 일회성 비공개 token이 현재 상태·변경 목록을 결속한다. 채팅 동의, 모델 호출,
자동 새로고침으로 승인하지 않는다. 승인 뒤 중단은 보존된 receipt에 한해 재개한다.
반복 재시도와 재인터뷰 이후 새 승인 판에 대한 바인딩도 기존 실패 이력을 유지한다.

일반 `restore`와 Step revisit는 계속 차단한다. 보호 경로 변경·오래된 승인·손상된 증거·링크·
경로 충돌은 전체 복구 거절이다. Windows는 ancestor를 고정하는 no-reparse 핸들과 별도
transaction journal을 쓰며, 작업 폴더와 기록 저장소가 같은 볼륨이어야 한다.
계약 상세: [GIL_Revisit_Recovery_v0.1.md](../../spec/GIL_Revisit_Recovery_v0.1.md).

## 정확한 소스와 검증

- 소스 [PR #67](https://github.com/hyun06000/gil/pull/67) 병합: `404f39a692741b15600671ca14b02ebc4f1d1086`.
- 양쪽 후보 빌드 소스: `c72f452e1e76fd76bef193caaebdfbed69a06ae3`.
- 최종 검증 head: `fa7993c809109c9a2cfda873805f56ea38badafd`.
  빌드 소스와의 유일한 차이는 `src/restore/preview/tests.rs`다. Windows 미지원이라는 과거
  assertion을 읽기 전용 미리보기·hardlink 거절 시험으로 교체했다. 런타임 소스는 동일하다.
  merge commit에서 바이너리를 빌드했다고 주장하지 않는다.
- `approved-plan,visual-authoring,product-safety` 프로필. Mac 도구 32개, Windows 33개.
- Claude의 호스트 지원 PR #66은 포함하지 않는다. 합칠 때 공용 registration/bootstrap과
  workflow·생성 HTML을 재검증해야 한다.

| 검증 | 결과/근거 |
|---|---|
| 로컬 기본 Core 전체 순차 | 1,519 통과 / 2 기존 ignored |
| 로컬 기본·제품 all-target 빌드 | 통과 |
| Mac/Windows 승인 라이브러리 | 각 679 통과 / 1 기존 ignored |
| 공통 실제 MCP 프로세스 승인·복구 wire | Mac/Windows 각 7 통과; 실패 보존·반복 재시도·실제 프로세스 중단/재개 |
| Windows 네이티브 안전 경계 | 5 통과; replace/create/delete, junction·hardlink, 부모 고정, 정리, 긴 경로 |
| chain 프로필 MCP | Mac 34 / Windows 37 통과 |
| JS UI/Plugin | 150 통과 / 4 skipped |
| 최종 배포·설치·고지 검사 | 132 통과 / 0 실패; 정확한 Mac archive 포함 |
| Mac 배포 저장소 | 무결성·raw history 28 통과 |
| Mac/Windows Chain CI | [41344142146](https://github.com/hyun06000/gil/actions/runs/41344142146) |
| Mac 기본 배포 CI | [41344142356](https://github.com/hyun06000/gil/actions/runs/41344142356) |
| Companion Mac/x64/ARM64 | [41344142196](https://github.com/hyun06000/gil/actions/runs/41344142196) |
| Windows x64/ARM64 default/visual | [41344142275](https://github.com/hyun06000/gil/actions/runs/41344142275) |
| Windows 제품 계약 | [41344142166](https://github.com/hyun06000/gil/actions/runs/41344142166) |
| Mac 실제 후보 | [41343593161](https://github.com/hyun06000/gil/actions/runs/41343593161) |
| Windows 실제 후보 | [41343590753](https://github.com/hyun06000/gil/actions/runs/41343590753) |

초기 Windows 실행에서 긴 archive 경로, 외부 기록 저장소를 찾지 못하는 CLI crash fixture,
과거 미지원 assertion을 발견해 수정했다. 긴 경로는 junction을 따라 canonicalize하지 않고
extended-length 경로로 변환한다. crash 시험은 실제 MCP server 프로세스를 종료·재시작한다.
초기 Mac `/var` alias 문제는 보존 경로를 실제 canonical 절대경로로 결속해 수정했다.
초기 stale catalog·UI notice hash는 계약·입력 비교 후 갱신했다. 신규 ignore는 추가하지 않았다.

추가 탐색으로 구형 general MCP fixture를 approved-plan 프로필에서 실행하면 10개가
Synthesis 승인 누락으로 실패한다. 해당 fixture의 지정 chain 프로필은 34개 통과하며,
승인 프로필은 전용 wire·전체 library로 검증한다. 모든 feature 조합의 legacy aggregate가
통과한다고 주장하지 않는다. 문서 worktree의 최초 고지 검사 3개 실패는 빠진 esbuild 의존성
때문이며 lockfile의 의존성을 설치한 뒤 최종 132개 검사를 통과했다.

## Mac 배포물

- [배포 PR #13](https://github.com/hyun06000/gil-marketplace/pull/13) 병합·tag commit: `fb7abe10b206a8c0da687e05c8ab21c02ef12348`.
- [고정 prerelease](https://github.com/hyun06000/gil-marketplace/releases/tag/v0.2.1-preview.9).
- archive: `gil-codex-macos-arm64-0.2.1-preview.9.tar.gz`, 3,536,700 bytes.
- SHA-256: `6d0a7e3b9755b67daabad63642453d6abf2904a673bd3b8b6f60a8a5edb2b64c`.
- Core: `5adbc495dc1aa97a071dda88a29979244f4d8129149f1986cd6446536d56f858`.
- UI: `22d3abaa9ccc26edd68f82e8ff1220348ae2d0f3f1cee502ee6fc03837466aed`.
- receipt: `f6c5ac18e2cbde81508f94b1b872509995c82ce730380768ff252f3cc7db7818`.
- 원본 CI bytes·실행 권한·고지·receipt를 그대로 보존한다. receipt의 `publishable:false`는
  빌드 당시 권한 기록이며 게시 승인은 사용자 요청과 PR/release로 별도 기록한다.
- 정확한 downloaded Core 승인 경계 wire: 20 통과 / 1 예상 product-safety 차단 / 0 실패.
  새 7개 복구 시험과 별개의 기존 승인 시나리오다. Git 없는 준비 시험 7개 통과.
- 게시 후 인증 없는 공개 HTTPS로 받은 archive의 크기·해시가 원본과 일치했다.
- 이번 판의 공식 CLI 설치·새 Mac VM·실제 Host fullscreen·사람 클릭·Luna 시험은 수행하지 않았다.

## Windows 배포물과 채널

- [고정 prerelease](https://github.com/hyun06000/gil/releases/tag/windows-owner-update-0.1.1-owner.8).
- installer: `GIL.Companion.Unsigned.Test._0.1.1-owner.8_x64-setup.exe`, 6,000,027 bytes.
- SHA-256: `da33d83b2d2d948ab8499f8a1220172c9be763c8cdc06a9d70f2ac2d602771f2`.
- Core: `224c24cb6b6861b13f532ebdceb8b8fca1bb6b95bd4a378b83005913ac59be0c`.
- 후보 CI의 설치·동일 버전 재설치·기존 MCP 연결 유지·다음 연결의 새 Core 선택·보이는 Companion 창·
  Codex CLI 등록/해제·제거 후 runtime 보존 검사 13개 모두 통과했다. Companion UI 내용과 실제
  Codex Desktop 대화·SmartScreen 평판은 검사 범위 밖이다.
- 정확한 Core의 기존 승인 wire 20 통과 / 1 예상 차단 / 0 실패. 설치된 Core 해시와 wire 해시가 같다.
- 서명 trusted comment가 `version:0.1.1-owner.8`을 포함한다. GitHub 자산용 파일명만
  정규화했으며 installer와 signature bytes는 원본과 같다. 원본 이름은 CI evidence에 남는다.
- `.exe`, `.sig`, `SHA256SUMS`, `unsigned-updater-acceptance.json`, `wire-run.json`을 첨부했다.
- 게시 후 인증 없는 공개 HTTPS로 받은 installer의 크기·해시가 CI 원본과 일치했다.
  공개 파일 검증 뒤 `distribution/windows/owner-update.json`을 owner.8로 전환한다.
- Authenticode publisher 서명은 없다. 기존 updater 서명과 `requireSignedVersion`은 유지한다.
- 사용자 업데이트 버튼으로 설치한다. 기존 MCP 연결을 강제 종료하거나 이전 Core를 덮어쓰지 않는다.
  새 연결부터 새 Core를 선택한다. Mac Plugin은 Windows 업데이트로 교체되지 않는다.

## 사용자가 직접 확인할 항목

업데이트 후 새 GIL 연결·새 대화에서 먼저 별도 시험 폴더로 확인한다.

1. Monitor가 실제로 보이고, 실패 Cycle의 변경 목록·대상·보존 위치가 표시되는가.
2. 클릭 전 파일은 그대로이며, 클릭 후 원본·실패 기록·증거가 남는가.
3. 같은 계획 항목의 새 Cycle을 열어 파일 증거와 배움을 남기며 다시 시도하는가.
4. 두 번째 실패에서도 이전 두 번의 실패가 다음 시도 문맥에 남는가.
5. 기존 막힌 프로젝트도 파일·보호 경로 검사를 통과하면 같은 절차로 이어갈 수 있는가.

실제 사용자의 Desktop 프로젝트·설치된 cache·설정·기록은 이번 개발에서 수정하지 않았다.
프로토콜 성공과 합성 App 입력은 실제 사람의 승인이나 화면 표시의 증거가 아니다.
