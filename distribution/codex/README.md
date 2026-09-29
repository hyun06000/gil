# Codex native marketplace preview

이 디렉터리는 **제작자용 배포 도구**다. 받는 사람에게 Node·Cargo·clone·tar 명령을 요구하는
설치 설명서가 아니다. 최종 UX는 Host의 Plugin 설치 한 번이다.

2026-09-30: 공개 HTTPS 설치·preview.2↔preview.1 rollback·Project 보존과 main 보호/PVR를 확인했다.
사용자는 [설치 안내](INSTALL.md), 제작자는 [현재 배포 장부](PREVIEW-2-PUBLICATION-20260930.md)를 따른다.
새 Mac·Apple 서명·공증은 미검수/미제공이다. 다음 9/29 절은 당시 준비 기록이다.

2026-09-29 기록: 기존 Mac의 공식 원격 설치·제거/재설치에 이어, 독립 이력의 비공개
`hyun06000/gil-marketplace`로 출처 전환과 실제 fullscreen·노드 상세를 확인했다.
[현재 경로 장부](CLEAN-MARKETPLACE-CHECKPOINT-20260929.md)를 따른다. 옛 `gil-distribution`은
비공개 감사/복구 이력으로 보존한다. 공개 HTTPS 설치·게시·새 Mac 검수는 별개로 남아 있다.
Skill 보정은 새 `0.2.1-preview.2` 후보로 검수하며 설치된 preview.1을 덮지 않는다.

기본 산출물은 `development_unsigned`, `publishable: false`인 **개발 검수물**이다.
사용자 승인으로 별도 `preview_unsigned` 시험판 채널을 준비한다. 검수한 clean tree에서
`unsigned-preview.mjs`가 새 version·경고·receipt를 갖춘 후보를 만들며, 기본 개발물을 덮지 않는다.
두 제작 경로 모두 원격 게시 기능은 없고 `artifact.mjs publish`는 항상 거절한다. 공개 marketplace
게시는 별도 승인·검수 단계이며 기존 `personal` marketplace를 자동 교체하지 않는다.

## 구조와 정본

```text
새 output/
  marketplace/                          장차 built-dist 저장소 root가 될 구조
    .agents/plugins/marketplace.json     root 기준 상대 source.path
    plugins/gil-companion-prototype/
      .codex-plugin/plugin.json          기존 정본 byte 그대로
      core/darwin-arm64/gil              Core + 17 tools + 내장 MCP App
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
Host 등록을 생성하지 않는다. 현재의 승인된 비공개 Host 등록은 위 장부에 별도로 기록한다.
template를 산출물의 `.agents/plugins/marketplace.json`에 복사한다. 소스 저장소의
`.claude-plugin/marketplace.json`은 별도 개발용 목록으로 그대로 남는다.

UI는 Rust 실행 파일에 내장된다. `server.mjs`, `node_modules`, Cargo source, 독립 `assets`, 설치
hook은 싣지 않는다. Companion을 싣거나 자동 실행하지 않는다. **도구·Skill 성공은 fullscreen
성공이 아니며**, Claude 작업 Plugin의 UI 전달 문제를 이 패키징으로 해결했다고 주장하지 않는다.

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
동치 → system dylib만 연결됨 확인 → 새 challenge → 17 tools → embedded resource/HTML hash 순서다.
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

`.github/workflows/codex-preview.yml`은 **수동 전용**, `contents: read`, credential 비보존,
full commit으로 고정한 Actions를 쓴다. tar와 검증 증거만 7일짜리 Actions artifact로 보관한다.
push/tag에 자동 실행하지 않고 release·dist repo·공개 catalog에 게시하지 않는다. workflow 작성과
GitHub runner 실제 성공은 별개다.

이전 source의 원격 CI 인수는 그 저장소에 보존한다. 새 `gil`의 clean root commit `fbc4100`에서
수동 CI·artifact 다운로드 검수를 통과했다 ([새 원격 체크포인트](CI-CHECKPOINT-20260928.md)).
35개 패키징/고지 시험·84개 package 고지·source snapshot·실행 권한·17 tools·내장 UI를 확인했다.
첫 실행의 Node 20 기반 Actions 경고는 그 기록에 보존한다. 현재 workflow는 공식
[checkout v7.0.1](https://github.com/actions/checkout/releases/tag/v7.0.1)과
[upload-artifact v7.0.1](https://github.com/actions/upload-artifact/releases/tag/v7.0.1)의 Node 24 실행점을
full SHA로 고정했다. `archive: true`를 명시해 tar·checksum·증거를 기존처럼 하나의 artifact로 묶는다.
보정한 PR #1의 clean `19fe57b`에서 원격 CI·다운로드 검수를 통과했고 annotation 0건으로
Node 20 경고 해소를 확인했다 ([후속 기록](CI-CHECKPOINT-20260928.md#follow-up-node-24-actions-on-pr-1)).
이 기록보다 뒤의 commit은 최종 PR head의 CI 성공을 따로 확인한 뒤 병합한다.
[이전 경계](../../SOURCE-MIGRATION.md)와 새 기계·Host 설치 인수를 구분한다.

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
3. 새 Mac 검수는 사용자 결정으로 유예하고 **미통과**로 표시한다. 기존 Mac의 preview.1 원격
   설치·제거/재설치·새 출처 전환·화면은 확인됐다. 새 후보의 version 쌍과 이전 판 rollback,
   공개 비인증 설치는 별도 검수한다.
4. 승인 후 built-dist 원격 저장소/불변 version·게시 권한·rollback 정책 확정 및 게시.
5. remote marketplace 등록은 public Plugins Directory 심사·등재와 별개임을 표시.

Developer ID·공증·새 Mac 검수를 갖춘 정식 신뢰 채널은 후속 작업이다. 기존
`release-preflight.mjs`는 그 서명 채널 점검이며 unsigned 후보의 선행 조건이 아니다.

Intel·universal·Windows는 아직 지원 대상이 아니다. macOS 배포를 닫은 뒤 추가한다.

## 참고 (2026-09-28 확인)

- [OpenAI Plugin packaging](https://developers.openai.com/plugins/build/plugins): compatibility manifest,
  root 상대 source.path, Git-backed marketplace와 public directory 구분.
- [GitHub runner 표](https://docs.github.com/en/actions/reference/runners/github-hosted-runners): `macos-15`
  arm64. 실제 job에서도 `uname -m`을 확인한다.
- [upload-artifact](https://github.com/actions/upload-artifact): raw 파일 권한 손실을 피하도록 tar 업로드.

정적 회귀시험의 가짜 Mach-O header를 실제 실행 성공 증거로 쓰지 않는다. 회귀시험과 실제 native
smoke는 분리되어 있다. 실행 검수 결과는 별도 체크포인트에 남긴다.
