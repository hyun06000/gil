# tools/docs-check — 문서 파일 연결 검사

설치 안내·로드맵·명세 같은 Markdown 문서에서 **저장소 안의 존재하지 않는 파일을 가리키는 연결**을
찾는 읽기 전용 도구다. Node 표준 라이브러리와 `git` 만 쓴다(의존성 설치 없음). 문서를 고치지 않고,
문서 안의 명령을 실행하지 않고, 네트워크를 쓰지 않는다.

내장 도움말 주제 주소(`gil help <주소>`, `더 알아보기`)의 검사는 이 도구의 일이 아니다. 그것은
Rust 쪽 `src/manual/router.rs`(`every_bundled_address_resolves`)·`tests/manual.rs`·`tests/help_ref.rs`가
이미 한다. 이 도구는 **일반 Markdown 파일 연결**만 본다.

## 실행

```sh
# Git 이 추적하는 Markdown 전체 (저장소 안 어느 폴더에서 실행해도 저장소 뿌리를 찾는다)
node tools/docs-check/check-links.mjs

# 특정 문서·폴더만 (폴더는 그 아래 추적 Markdown 전체)
node tools/docs-check/check-links.mjs README.md INSTALL.md spec/

# JSON · 문제 아닌 연결(외부 주소 등)까지 나열 · 다른 저장소
node tools/docs-check/check-links.mjs --json
node tools/docs-check/check-links.mjs --verbose
node tools/docs-check/check-links.mjs --root <저장소 안 폴더>

# 자체 시험 (합성 문서를 임시 Git 저장소에 만들어 검사한다)
node --test tools/docs-check/*.test.mjs
```

종료 코드:

| 코드 | 뜻 |
|---|---|
| 0 | 문제 없음 |
| 1 | 문제 상태(`missing` `untracked` `outside_repo` `unsupported`)의 연결이 하나 이상 있다 |
| 2 | 도구 실행 오류 — 잘못된 인자, Git 저장소가 아님, 입력 문서가 없음·Markdown 이 아님·저장소 밖 |

## 결과

사람이 읽는 결과(기본)는 문제 연결만 한 줄씩 적고 분류 개수와 검사하지 않은 것을 덧붙인다.

```text
README.md:3:26  [missing]  REMOVED.md  — 파일이 없다
^문서(저장소 상대) ^줄:칸  ^상태      ^적힌 대상    ^이유
```

`--json` 은 같은 판정을 그대로 담는다(`schema: 1`). 텍스트와 JSON 은 같은 findings 에서 만들어지며
시험이 둘의 문제 목록·개수 일치를 검사한다.

| 필드 | 뜻 |
|---|---|
| `scope.mode` | `tracked`(추적 Markdown 전체) 또는 `requested`(지정 문서) |
| `scope.documents` | 검사한 문서의 저장소 상대 경로 |
| `scope.untracked_inputs` | 지정했지만 Git 이 추적하지 않는 입력 문서 |
| `findings[]` | 연결마다 `file` `line` `column` `kind` `text` `target` `status` `reason`, 있으면 `resolved`(저장소 상대 해석 결과) `anchor` `label` |
| `problems[]` | `findings` 중 문제 상태만 |
| `counts` | 상태별 개수 |
| `not_checked[]` | 이번 실행이 검사하지 않은 것 |
| `exit_code` | 위 표의 종료 코드 |

### 상태

문제(종료 코드 1):

| 상태 | 뜻 |
|---|---|
| `missing` | 해석한 경로가 Git index 에도 디스크에도 없다. 대소문자만 다른 파일이 있으면 이유에 적는다 |
| `untracked` | 디스크에는 있지만 Git 이 추적하지 않는다 — 다른 checkout 과 GitHub 에서는 깨진다 |
| `outside_repo` | 저장소 뿌리 밖으로 나간다. 따라가서 존재를 보지 않는다 |
| `unsupported` | 연결처럼 적혔지만 검사할 수 없다(아래 "미지원 문법"). 조용히 성공으로 치지 않는다 |

문제 아님(분류만):

| 상태 | 뜻 |
|---|---|
| `ok` | 파일 또는 폴더가 Git index 에 있다 |
| `ok_anchor_unchecked` | `파일.md#절` — 파일은 있다. **#절 주소 자체는 검사하지 않았다** |
| `same_doc_anchor` | `#절` 만 있는 같은 문서 안 연결 — 검사하지 않는다 |
| `external` | `http:` `https:` `//` 외부 주소 — 네트워크로 확인하지 않는다 |
| `email` | `mailto:` |
| `other_scheme` | 그 밖의 `scheme:` 주소(`vscode:` 등) — 검사하지 않는다 |

## 지원 범위

**검사 대상**: Git 이 추적하는 `*.md` `*.markdown`(기본) 또는 지정한 문서. 존재 판정의 기준은
`git ls-files`(index)다. 파일 시스템을 돌아다니지 않으므로 `.gil/` 기록, 설치된 Plugin 캐시, 비추적
개인 파일은 보지 않는다. 디스크 확인은 `untracked` 판정을 위해 **그 경로 하나**만 `lstat` 하고, 대소문자를
가리지 않는 파일 시스템(macOS 기본)에서 적힌 이름 그대로인지 부모 폴더 이름 목록으로 확인한다.

**경로 해석**: 상대 경로는 그 문서가 있는 폴더 기준이다. `../` 로 올라가 뿌리를 넘으면 `outside_repo`.
퍼센트 인코딩(`%20`, `%EB%AA%A8…`)은 풀어서 비교하고, 한글은 NFC 로 정규화해 비교한다. 폴더(`spec/`, `./`)도
존재만 본다. Markdown 이 아닌 파일(`.png` `.json`)도 존재만 본다.

**인식하는 문법**:

| 문법 | 예 | 처리 |
|---|---|---|
| 인라인 연결 | `[t](경로)` `[t](경로 "제목")` `[t](<공백 든 경로>)` `[t](a(b).md)` | 검사 |
| 이미지 | `![a](경로)` , 연결 안 이미지 `[![a](i.png)](x.md)` | 둘 다 검사 |
| 참조 정의 | `[label]: 경로` `[label]: <공백 든 경로> "제목"` | 정의의 경로를 검사 |
| 참조 연결 | `[t][label]` `[label][]` `[label]`(정의가 있을 때만) | 정의를 따라 검사. 정의 없는 `[t][label]` 은 `unsupported` |
| 울타리 코드 | ```` ``` ```` `~~~`, 목록 안 들여쓴 울타리 | 안의 연결을 보지 않는다(닫히지 않으면 문서 끝까지) |
| 인라인 코드 | `` `[가짜](x.md)` `` `` ``…`` `` | 보지 않는다(문단 안에서만 닫힌다) |
| HTML 주석 | `<!-- … -->` | 보지 않는다 |
| 이스케이프 | `\[t](x.md)` | 연결이 아니다 |

**미지원 문법** — 발견하면 `unsupported` 로 **보고**하고 검사하지 않는다:

- HTML 속성 연결 `<a href="…">` `<img src="…">`
- 공백이 든 맨 목적지 `[t](a b.md)` — 렌더러도 연결로 보지 않는다. `<a b.md>` 또는 `a%20b.md` 로 적는다
- 공백이 든 맨 참조 정의 `[l]: a b.md` — 같은 이유. `<a b.md>` 로 감싼다
- 닫히지 않은 괄호 `[t](x.md` , 빈 대상 `[t]()`, 해석할 수 없는 퍼센트 인코딩 `bad%ZZ.md`
- 뿌리 절대 경로 `/INSTALL.md`, Windows 절대 경로 `C:\…` — 렌더러마다 뿌리가 다르다
- 질의 문자열이 붙은 파일 경로 `INSTALL.md?x=1`

**인식하지 않는 것**(검사 대상에서 조용히 빠지거나 글로 본다):

- 4칸 들여쓰기 코드 블록 — 목록 안 문단과 구분하지 않아 **코드로 가리지 않는다**. 그 안의 `[t](x.md)` 는
  보통 연결처럼 검사된다. 가짜 연결 예시는 울타리 코드나 인라인 코드에 둔다
- `<…>` 자동 연결 중 scheme 없는 것(`<x.md>`) — CommonMark 에서도 연결이 아니다
- `#절` 주소의 실제 heading 존재, 외부 주소의 유효성, 연결 글자(text)의 내용
- 링크 안에 든 또 다른 링크, 중첩된 `[[…]]` 위키 문법

## 저장소 검사 (2026-11-16)

`node tools/docs-check/check-links.mjs` — 추적 Markdown 169개, 연결 681개: `ok` 402 · `ok_anchor_unchecked` 41 ·
`external` 238 · 문제 0, 종료 코드 0. 저장소에는 이미지·참조형·HTML 연결이 없어 그 경로는 합성 문서 시험으로만
확인했다. `#절` 41건의 절 존재와 외부 주소 238건의 유효성은 검사하지 않았다.

## 하지 않는 것

문서를 수정하지 않는다(시험이 입력 문서의 해시와 Git 상태가 바뀌지 않음을 확인한다). 문서 안의 명령을
실행하지 않는다. 네트워크를 쓰지 않는다. 예외 목록·무시 파일이 없다 — 발견한 문제는 모두 보고한다.
공통 로드맵 갱신과 CI 편입은 이 폴더의 범위가 아니다.
