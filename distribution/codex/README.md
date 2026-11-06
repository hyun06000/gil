# Codex native marketplace preview

이 디렉터리는 **제작자용 배포 도구**다. 받는 사람에게 Node·Cargo·clone·tar 명령을 요구하는
설치 설명서가 아니다. Agent가 Plugin/MCP와 보호 훅의 설치를 함께 조율한다.

현재 공개 macOS 검증판은 `0.2.1-preview.24`이다. 사용자 절차는
[통합 설치 안내](../../INSTALL.md)와 [macOS 상세 안내](INSTALL.md), 정확한 소스·CI·해시·인수 범위는
[preview.24 게시 장부](PREVIEW-24-PUBLICATION-20261106.md)를 따른다.
Windows x64의 별도 installer는 [Windows 안내](../windows/INSTALL.md)에 있다.
날짜별 과거 게시 장부는 해당 버전의 증거이며 최신 설치 절차로 사용하지 않는다.

기본 산출물은 `development_unsigned`, `publishable: false`인 **개발 검수물**이다.
사용자 승인으로 별도 `preview_unsigned` 시험판 채널을 준비한다. 검수한 clean tree에서
`unsigned-preview.mjs`가 새 version·경고·receipt를 갖춘 후보를 만들며, 기본 개발물을 덮지 않는다.
두 제작 경로 모두 원격 게시 기능은 없고 `artifact.mjs publish`는 항상 거절한다. 공개 marketplace
게시는 별도 승인·검수 단계이며 기존 `personal` marketplace를 자동 교체하지 않는다.

## 구조와 정본

프로필마다 도구 목록·action surface·저장 형식이 다르다. 공개판의 정확한 조합은 게시 장부와
후보의 `release.json`을 따른다. receipt의 compatibility/migration 선언은 실제 Core의
도구 schema·challenge 응답과 일치해야 smoke가 통과한다. `migration: requires_review`는
자동 업데이트·롤백 승인이 아니며 별도 호환성 검토를 요구한다.

```text
새 output/
  marketplace/                          장차 built-dist 저장소 root가 될 구조
    .agents/plugins/marketplace.json     Codex catalog · root 기준 상대 source.path
    .claude-plugin/marketplace.json      Claude Code catalog · 같은 marketplace 이름, 같은 상대 source
    plugins/gil-companion-prototype/
      .codex-plugin/plugin.json          기존 정본 byte 그대로
      .claude-plugin/plugin.json         Claude Code adapter · 같은 이름·설명·Core, ${CLAUDE_PLUGIN_ROOT} 기준
      core/darwin-arm64/gil              프로필별 native Core + 내장 MCP App
      skills/gil-companion/SKILL.md       공용 Skill 그대로
      LICENSE
      THIRD-PARTY-NOTICES.txt             제3자 package 출처·원문 고지
      THIRD-PARTY-NOTICES.json            검토한 버전·원문 hash inventory
      RUST-STDLIB-NOTICES.html            공식 Rust 표준 라이브러리 고지
    PREVIEW.md                           배포 금지·지원 CPU·미검증 경계
    release.json                         상대경로·mode·bytes·SHA-256·source snapshot
  gil-codex-macos-arm64-preview.tar.gz   숨김 manifest와 실행 비트 보존
  SHA256SUMS                            archive SHA-256
  checks.json                           압축 왕복·첫 실행·stdio·UI hash 실측
```

기본 build는 identity/version을 재발명하지 않는다. 정본의 개발 cachebuster도 보존한다.
unsigned 후보 제작에서만 출력 version을 명시적 `X.Y.Z-preview.N`으로 정한다.
`catalog.json`은 plugin-creator의 `create_basic_plugin.py`로 생성한
template다. `personal`과 구별하는 이름은 `gil-preview-macos-arm64`다. template 자체는 원격 주소나
Host 등록을 생성하지 않는다. 공개 설치 출처와 고정 ref는 설치 안내와 게시 장부에 기록한다.
template를 산출물의 `.agents/plugins/marketplace.json`에 복사한다. 게시 뒤에는 저장소 `main`의
`distribution/update-feed.json`에 플랫폼별 버전·release URL을 적어야 설치된 Core의 Monitor가 **업데이트 안내**
버튼을 띄운다(metadata만 읽음; `distribution/codex/update-feed.test.mjs`가 INSTALL.md와의 일치를 검사한다). `claude-marketplace.json`은
같은 이름의 Claude Code catalog로 산출물의 `.claude-plugin/marketplace.json`이 된다. 소스 저장소
최상위의 `.claude-plugin/marketplace.json`은 source clone용 개발 목록이며 산출물에 싣지 않는다.
unsigned 후보 제작은 두 manifest의 version을 같은 `X.Y.Z-preview.N`으로 바꾼다.

UI는 Rust 실행 파일에 내장된다. `server.mjs`, `node_modules`, Cargo source, 독립 `assets`, 설치
hook은 싣지 않는다. 사용자 설치 안내에서 Core의 `write-guard install --auto --host codex|claude|both`로
사용 Host의 보호 훅을 별도 등록·검증한다(신규·누락·업데이트 모두). Plugin 설치 자체가 훅을 등록하는 것은 아니다.
Companion을 싣거나 자동 실행하지 않는다. **도구·Skill 성공은 fullscreen
성공이 아니며**, Codex와 Claude Code의 실제 화면 인수는 Host별로 따로 기록한다.

## 제작자 실행

macOS arm64, Rust 1.97.1 + aarch64-apple-darwin target, Node 22 이상과 잠근 UI 의존성이 필요하다.
받는 사용자에게 필요한 도구가 아니다. 기존 설치 cache·전역 gil·Plugin 설정은 바꾸지 않는다.

```sh
npm ci --prefix mcp-app
cargo fetch --locked
node --test distribution/codex/*.test.mjs distribution/compliance/notices.test.mjs
mkdir -p target
node distribution/codex/build-preview.mjs target/codex-preview
```

output 부모는 있어야 하고 마지막 디렉터리는 **새 이름**이어야 한다. 기존 output은 덮어쓰거나
지우지 않는다. 실패하면 성공 `checks.json`을 만들지 않고 partial output은 진단용으로 남긴다.
다시 실행할 때 새 output 이름을 쓴다. 이전 실패를 재실행 성공으로 덮지 않는다.

UI build → 오프라인 제3자 고지 coverage → remap된 Rust build → catalog/allowlist → archive → 새 디렉터리 압축 해제 → 파일·권한
동치 → system dylib만 연결됨 확인 → 새 challenge/호환성 대조 → 프로필별 tools schema → embedded resource/HTML hash 순서다.
tar owner/group은 root/wheel로 고정하고 ACL·xattr·resource-fork는 싣지 않는다. 파일별 hash/권한
재현과 archive 자체의 bit-for-bit 재현은 다르다. archive timestamp 정규화는 아직 하지 않는다.
정본의 platform Core나
설치 cache는 교체하지 않는다.

[제3자 고지 자동화](../compliance/README.md)는 잠근 Core/UI 의존성·하위 고지·Rust compiler
commit을 policy와 대조한다. 버전/원문/포함 bundle이 바뀌거나 파일이 없으면 제작자 build/CI가
실패한다. 사용자 설치 때 인터넷으로 고지를 내려받지 않으며 GIL의 MIT LICENSE도 바꾸지 않는다.

이미 지어진 실행 파일의 묶음만 검사하려면:

```sh
node distribution/codex/artifact.mjs create target/plugin-core-build/aarch64-apple-darwin/release/gil target/codex-tree-only
node distribution/codex/artifact.mjs verify target/codex-tree-only
node distribution/codex/smoke.mjs target/codex-tree-only plugins/gil-companion-prototype/assets/monitor.html
```

`create` receipt는 공급받은 binary와 source의 빌드 대응을 증명하지 않는다. 각각의 hash를 기록할
뿐이다. `build-preview`는 먼저 빌드하고 빌드 중 source 불변까지 검사한다. dirty source는 preview에만
허용하고 그대로 표시한다. 해시는 전송 오류·변조 검출용이지 publisher 인증이나 서명의 대체물이
아니다. receipt까지 함께 바꾸는 위협은 서명 및 검증된 배포 채널에서 다룬다.

smoke child는 PATH `/usr/bin:/bin`과 격리된 Monitor 설정만 받는다. 사용자 Project나 Companion은
열지 않는다. 첫 challenge 실행과 MCP initialize는 각각 30초 제한이며 자동 재시도하지 않는다.
이전 ZIP 첫 실행 timeout 미결을 보존한다. **새 기계·quarantine·실제 marketplace 설치**와는 별개다.

## 파이프라인과 공개 배포 조건

`.github/workflows/codex-preview.yml`은 **main 대상 PR 및 수동 실행**, `contents: read`, credential 비보존,
full commit으로 고정한 Actions를 쓴다. tar와 검증 증거만 7일짜리 Actions artifact로 보관한다.
PR은 정확한 head의 개발 검수물만 만들며 unsigned version 후보는 명시적 수동 입력에서만 만든다.
push/tag로 release·dist repo·공개 catalog에 자동 게시하지 않는다. workflow 작성과
GitHub runner 실제 성공은 별개다.

과거 CI·Node 실행점 전환의 증거는 [초기 체크포인트](CI-CHECKPOINT-20260928.md)에 보존한다.
현재 Actions·트리거·matrix는 `.github/workflows/codex-preview.yml`이 정본이다.
각 PR은 최신 head의 전체 해당 CI를 확인한 뒤 병합한다. 과거 통과를 새 commit에 승계하지 않는다.

source와 active built-dist 저장소는 공개됐다. CI 로그와 다운로드 가능한
Actions artifact도 공개 범위에 포함된다. 개발용이라는 표시는 접근 제한이 아니며, 원격 실행·업로드 전에도
민감정보와 포함 파일을 검수한다. 이 pipeline은 정식 release나 marketplace 게시를 하지 않는다.

공개 배포 전 필요한 작업:

[native macOS release gate](RELEASE-macos.md)에 준비 → 공개 → 배포 순서와 읽기 전용
`release-preflight.mjs`를 둔다. 이 점검은 서명·공증이나 게시를 수행하지 않고 항상
`publishable: false`를 유지한다. 현재 유지관리자 환경은 Developer Program/배포용 identity 미준비다.

1. [오픈소스 공개 준비 게이트](../../spec/GIL_Open_Source_Readiness_v0.1.md) 완료와 source 검수·commit,
   재현 build와 독립 CI 성공. dirty preview를 정식 release로 재명명하지 않음.
2. 첫 unsigned 시험판에는 Apple Developer ID·공증이 없음을 고지하고 version/hash·출처를 고정.
   별도 후보 제작·검증·공식 설치 안내는 [release gate §1](RELEASE-macos.md)을 따른다.
3. 새 후보의 실제 설치·화면·이전 판과의 업데이트/rollback 범위를 따로 기록한다.
   이전 판의 새 Mac VM 검수와 이번 판의 사용자 화면 검수를 구분하며 미실행은 그대로 표시한다.
4. 승인 후 built-dist 원격 저장소/불변 version·게시 권한·rollback 정책 확정 및 게시.
5. remote marketplace 등록은 public Plugins Directory 심사·등재와 별개임을 표시.

Developer ID·공증·새 Mac 검수를 갖춘 정식 신뢰 채널은 후속 작업이다. 기존
`release-preflight.mjs`는 그 서명 채널 점검이며 unsigned 후보의 선행 조건이 아니다.

이 제작 경로는 macOS arm64 전용이다. Windows는 별도 배포 경로이며 Intel Mac·universal을 지원한다고 주장하지 않는다.

## 참고 (2026-09-28 확인)

- [OpenAI Plugin packaging](https://developers.openai.com/plugins/build/plugins): compatibility manifest,
  root 상대 source.path, Git-backed marketplace와 public directory 구분.
- [GitHub runner 표](https://docs.github.com/en/actions/reference/runners/github-hosted-runners): `macos-15`
  arm64. 실제 job에서도 `uname -m`을 확인한다.
- [upload-artifact](https://github.com/actions/upload-artifact): raw 파일 권한 손실을 피하도록 tar 업로드.

정적 회귀시험의 가짜 Mach-O header를 실제 실행 성공 증거로 쓰지 않는다. 회귀시험과 실제 native
smoke는 분리되어 있다. 실행 검수 결과는 별도 체크포인트에 남긴다.
