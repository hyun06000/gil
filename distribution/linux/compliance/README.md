# Linux x64 (GNU) third-party notices

제작자용 inventory다. 도구와 규칙은 [공통 고지 안내](../../compliance/README.md)와 같고, 이 폴더는
`x86_64-unknown-linux-gnu` target의 **자기 Cargo graph**를 검토한 결과다. macOS inventory를 대신 쓰지 않는다.

- `notice-policy.json`: 잠근 graph 101개(Cargo 96 + MCP App JS bundle 5)와 Rust 1.97.1 표준 라이브러리 고지.
- `texts/<sha256>.json`: 원문 UTF-8의 lossless JSON 표현. 파일명은 복원된 원문 bytes의 SHA-256이다.

## 2026-11-25 검토 (Cargo.lock·UI 입력은 macOS inventory와 같은 hash)

`collect(root, {target: 'x86_64-unknown-linux-gnu'})`의 결과를 macOS inventory와 package 단위로 비교했다.

| | package | 선언 license | 원문 |
|---|---|---|---|
| Linux에만 있음 | `inotify@0.11.5` | ISC | crate의 `LICENSE` 하나. 하위 고지·NOTICE 없음 |
| Linux에만 있음 | `inotify-sys@0.1.8` | ISC | crate의 `LICENSE` 하나. 하위 고지·NOTICE 없음 |
| Linux에만 있음 | `mio@1.2.2` | MIT | crate의 `LICENSE` 하나. 하위 고지·NOTICE 없음 |
| macOS에만 있음 | `fsevent-sys@4.1.0` | — | 이 inventory에 넣지 않는다 |

나머지 98개는 이름·버전·역할·integrity·선언 license·원문 hash가 macOS inventory와 같았다. 세 원문은 선언된
license와 일치하는 전문이며 그대로 실었다. 빌드 의존성을 보수적으로 포함하므로 모든 package가 최종 실행
파일에 링크됐다는 뜻이 아니다.

표준 라이브러리 고지는 macOS·Windows inventory가 고정한 것과 같은 bytes(공식 Rust 1.97.1 배포의
`COPYRIGHT-library.html`)다. Linux CI job이 같은 toolchain의 `x86_64-unknown-linux-gnu` rust-docs component를
그 runner에 설치해 그 안의 `COPYRIGHT-library.html`과 이 원문을 바이트 단위로 대조한다(`product-candidate.yml`).
다르면 job이 실패하며 원문·정책을 자동으로 고치지 않는다. 대조 결과(양쪽 SHA-256)는 그 실행의 요약에 남는다.

## 실패 조건

`distribution/product/build.mjs`는 Linux 포장 전에 이 policy를 실제 graph와 대조한다
(`assertCoverage`). package·버전·역할·integrity·license·원문 hash·Rust commit이 다르면 포장이 실패한다.
자동 갱신 옵션은 없다. 의존성이 바뀌면 원문을 검토하고 이 폴더를 명시적으로 고친다.
