# Windows 릴리스 — 단일 실행 파일, 서명 없음 (2026-11-12)

배포에 필요한 것은 공개 GitHub 저장소와 그 Releases뿐이다. 서명·Store·NSIS·Companion은 쓰지 않는다
(결정서 §3; 재검토 조건은 [TRUST-STRATEGY](TRUST-STRATEGY-20261112.md) 머리말).

## 태그를 푸시하면 끝난다

```sh
git tag v0.3.0
git push origin v0.3.0
```

[`windows-release.yml`](../../.github/workflows/windows-release.yml)이 `v*` 태그에서 다음을 수행한다.

1. 태그에서 버전을 읽어 `GIL_BUILD_VERSION`으로 스탬프한다 (`gil --version` → `gil 0.3.0`).
2. `x86_64-pc-windows-msvc`, `RUSTFLAGS=-C target-feature=+crt-static`, 기능 프로필
   `approved-plan,visual-authoring,product-safety`로 release 빌드한다. VC++ 재배포 패키지가 없는 PC에서도 실행된다.
3. 빌드한 exe로 `--version`, `install --no-wait`, `doctor`(Host 없는 runner → `NOT_INSTALLED` 둘, 종료 0)를 실행해 확인한다.
4. 자산 `gil-x86_64-pc-windows-msvc.exe`, `checksums.txt`(SHA-256)를 Release에 올린다. 파일명에 버전을 넣지 않으므로
   `releases/latest/download/<자산>` 주소가 계속 유효하다.
5. 태그에 `-`가 있으면(`v0.3.0-rc.1`) prerelease로 표시한다. **prerelease는 `latest`가 되지 않는다.**

`gil` 저장소의 다른 검수 자산(`runtime-verify-*`, `windows-dev-visual-*`, `windows-owner-update-*`)은 prerelease로 두어
`latest`가 항상 이 워크플로의 정식 release를 가리키게 한다.

## 버전 체계 — 결정 (2026-11-12, Q4)

다른 프로젝트가 많이 쓰는 세 방식을 비교했다.

| 방식 | 예 | 장점 | 단점 |
|---|---|---|---|
| **A. 단일 SemVer 태그 `vX.Y.Z`** (rustup, uv, ripgrep, gh, cargo-dist 기본) | `v0.3.0`, `v0.3.1-rc.1` | 플랫폼이 달라도 버전 하나. `latest`가 명확. 도구·사람 모두 정렬 규칙을 안다. Monitor의 SemVer 비교와 그대로 맞는다 | Mac 채널이 같은 번호에 합류해야 한다 |
| B. 플랫폼별 채널 접미사 (현행) | `0.1.1-owner.15`, `0.2.1-preview.29` | 플랫폼마다 독립 cadence | 번호가 둘이라 사용자·문서가 혼동. `latest`가 어느 채널인지 불명. prerelease 접미사가 영구화됨 |
| C. 날짜 버전 (CalVer) | `2026.11.12` | 읽기 쉽고 충돌 없음 | prerelease 순서가 없다. Monitor의 strict SemVer 비교를 바꿔야 한다. "어떤 변화인지" 신호가 없다 |

**A를 채택한다.** Windows 첫 단일 실행 파일 릴리스는 `v0.3.0`이다(현행 Windows `0.1.1-owner.15`, Mac `0.2.1-preview.29`보다
큰 번호이므로 Monitor의 "새 버전 있음" 비교가 두 채널 모두에서 성립한다). macOS 채널은 다음 게시부터 같은 번호에 합류한다.
시험판은 `v0.3.0-rc.N` prerelease로 내며 `latest`에 영향을 주지 않는다.

## 게시 뒤 할 일

- `distribution/update-feed.json`의 `windows-x64`와 `distribution/windows/current-release.json`(현재 릴리스 핀: 버전·태그·자산 SHA-256)을
  새 버전으로 갱신한다(게시 장부와 같은 PR). `update-feed.test.mjs`가 둘의 일치를 검사한다.
  `install_doc`은 루트 `INSTALL.md`의 raw 주소다.
- [TESTER-CHECK](TESTER-CHECK.md)의 체크리스트와 [SIMPLE-INSTALL §10](SIMPLE-INSTALL-20261112.md)의 추가 항목을
  실기기에서 확인하고 결과를 장부로 남긴다. 특히 Smart App Control이 켜진 PC의 결과는 서명 재검토 조건이다.
