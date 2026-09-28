# Native MCP / Monitor third-party notices

제작자용 오프라인 검증·조립 도구다. 사용자 설치 과정에서 Node/Cargo나 추가 다운로드를 요구하지 않는다.
GIL의 root MIT LICENSE를 변경하지 않는다. Companion과 legacy Node bridge는 이 artifact에 포함하지 않는다.

## 포함 범위

macOS arm64 native MCP: 잠근 Cargo graph 79개(일반 후보 57 + build/proc-macro 22),
실제 MCP App JS bundle 5개, Rust 1.97.1 표준 라이브러리 고지.
빌드 의존성을 보수적으로 포함하며 모든 package가 최종 실행 파일에 링크됐다고 주장하지 않는다.
표준 라이브러리 고지도 각 플랫폼에서 실제 사용된 코드만 추려낸 것이 아니라 공식 배포본의 전체 고지다.

- `notice-policy.json`: 검토한 입력 hash, package/version/역할/선언 license, 원문 경로·hash.
- `upstream.json`: crate에 원문이 없는 rmcp 두 package의 고정 commit 대조 근거.
- `texts/<sha256>.json`: 원문 UTF-8의 lossless JSON 표현. CRLF나 끝 개행 없음까지 복원한다.
  파일명은 **복원된 원문 bytes**의 SHA-256이며 JSON 파일 자체의 hash가 아니다.
- `notices.mjs`: network/설치/정책 자동 갱신 없이 검증하고 최종 파일을 메모리에서 조립한다.
- `notices.test.mjs`: 누락·변경 거절과 실제 lock/bundle 정합성 시험.

Cargo package 안은 하위 경로까지 LICENSE/COPYING/COPYRIGHT/NOTICE를 찾는다.
`regex-syntax/src/unicode_tables/LICENSE-UNICODE`, `tracing-core/src/spin/LICENSE`도 포함한다.
`rmcp`/`rmcp-macros` 3.4.0은 `.cargo_vcs_info.json`의 commit
`fd7811fdaa9fefa1c8034534b4d7a31c97204f89`를 upstream tree와 대조했다.
그 tree의 법적 고지 파일은 root LICENSE 하나였고 별도 NOTICE는 없었다.
원문은 MCP UI SDK의 LICENSE와 바이트 단위로 같았으므로 같은 hash로 보관한다.

MCP SDK metadata의 MIT/Apache 표기를 원문 대신 쓰지 않는다. MIT→Apache 전환 조건과 잔존
MIT 기여분을 설명하는 원문을 그대로 싣는다. OR 선택지를 임의로 하나로 삭제하거나 AND 조건을
OR로 바꾸지 않는다. 원문에 문서 관련 조건이 있다는 이유만으로 문서 자체가 bundle에 들어갔다고
판정하지 않는다. 제3자의 저작권 표기와 공개 연락처는 고지의 일부이므로 그대로 보존한다.

## 실행과 실패 조건

```sh
# 제작자가 잠근 의존성을 준비한다. 설치된 Plugin은 이 도구들을 쓰지 않는다.
npm ci --prefix mcp-app
cargo fetch --locked
node distribution/compliance/notices.mjs check
node --test distribution/compliance/notices.test.mjs distribution/codex/artifact.test.mjs
```

`check`는 Cargo metadata/tree를 `--locked --offline`으로 조회하고 esbuild `write:false`의
metafile로 실제 UI 포함 package를 다시 계산한다. 재생성 JS가 현재 embedded HTML 안에 있는지도
검사한다. package 목록·버전·역할·integrity·license metadata·하위 고지 hash·Rust compiler commit이
policy와 다르면 실패한다. 원문 추가도 검토 대상이므로 조용히 새로 받아 통과시키지 않는다.

Cargo.toml/lock, UI lock/build recipe/현재 HTML hash도 고정한다. UI 코드만 바뀐 경우에도 먼저
정상 UI build를 한 뒤 새로운 bundle hash와 의존성 동일 여부를 확인해 policy를 명시적으로
갱신해야 한다. 동작을 막는 것은 사용자 앱이 아니라 제작자 패키징/CI 단계다.

`build-preview.mjs`는 UI build 뒤 이 전체 검사를 실행한다. `artifact.mjs create`도 입력 hash와
원문을 검사하고 아래 세 파일을 반드시 동봉한다. tree-only create는 Cargo graph를 재조회하거나
공급 binary의 빌드 출처를 증명하지 않으므로 정식 후보는 반드시 전체 pipeline을 거친다.

```text
plugins/gil-companion-prototype/
  LICENSE                       GIL 자체 MIT
  THIRD-PARTY-NOTICES.txt        package별 출처·원문 연결 + 중복 제거한 원문 전문
  THIRD-PARTY-NOTICES.json       조립에 사용한 고정 inventory/policy
  RUST-STDLIB-NOTICES.html       Rust 공식 COPYRIGHT-library.html 원문
```

고지 JSON은 소비자가 실행할 코드가 아니다. preview의 allowlist, 파일별 hash/mode receipt와
압축 왕복 검사가 세 파일에도 적용된다. 고지 파일이 빠진 결과를 성공 receipt로 만들지 않는다.
기존 `development_unsigned` / `publishable:false` 경계는 유지된다.

## 의존성 갱신 시 유지관리자 절차

1. 잠금 파일과 UI를 갱신한 뒤 `collect()`가 돌려주는 후보를 기존 policy와 비교한다.
   기본 CLI에는 refresh/approve/download 옵션이 없다.
2. 변경된 package의 배포 원문과 하위 고지, source/integrity를 검토한다. 원문이 없으면 고정된
   upstream revision과 NOTICE 유무를 확인하며, 이름이 같은 일반 license template로 대체하지 않는다.
3. 원문을 그대로 `texts` JSON에 넣고 원문 bytes hash·policy를 명시적으로 갱신한다.
   package 제거 시 더는 참조하지 않는 고지도 확인 후 제거한다. 자동 삭제하지 않는다.
4. Rust toolchain 변경 시 해당 공식 배포본의 `COPYRIGHT-library.html`도 검토한다.
   현재 원문은 설치된 공식 rust-docs 1.97.1 component에서 얻었고 compiler commit을 함께 고정했다.
   CI의 minimal toolchain에 rust-docs가 없어도 이미 검토한 원문으로 검증·조립할 수 있다.
5. 위 시험과 새 artifact의 압축 왕복을 통과시킨다. 원격 CI 실행/업로드는 별도 승인 경계를 따른다.

CI는 **수동 preview workflow**에 연결돼 있다. 이전 source의 실행 증거는 원본 저장소에 보존하며
새 저장소의 CI 성공으로 옮겨 적지 않는다. 새 `gil` root commit `fbc4100`의
[독립 CI 및 다운로드 인수](../codex/CI-CHECKPOINT-20260928.md)에서 84개 package coverage와
원문 고지 bytes를 재검증했다. 새 source의 범위는 [SOURCE-MIGRATION.md](../../SOURCE-MIGRATION.md)를 따른다.
이 고지 자동화는 법률적 적합성의 자동 인증이 아니다. Companion/다른 target, 별도 배포 표면과
과거 이력의 공개 의도 등 [오픈소스 게이트](../../spec/GIL_Open_Source_Readiness_v0.1.md)의 나머지는 열려 있다.

## 근거

- [Cargo tree의 근사 범위](https://doc.rust-lang.org/cargo/commands/cargo-tree.html)
- [MCP rust-sdk 고정 commit LICENSE](https://github.com/modelcontextprotocol/rust-sdk/blob/fd7811fdaa9fefa1c8034534b4d7a31c97204f89/LICENSE)
- [Apache-2.0 원문 §4](https://www.apache.org/licenses/LICENSE-2.0)
- 고정 inventory와 원문은 이 디렉터리의 `notice-policy.json`·`upstream.json`·`texts/`가 정본이다.
