# GIL

제품 source 정본의 새 저장소: [hyun06000/gil](https://github.com/hyun06000/gil).
현재는 **공개 opt-in unsigned preview 채널**이며 안정판이 아니다. 기존 구현의 코드·계약을 선별해
독립 Git 이력으로 시작한다. 아래 기존 기능 인수와 새 저장소의 회귀 검증을 구분한다.
이전 범위와 제외 기준은 [SOURCE-MIGRATION.md](SOURCE-MIGRATION.md)를 본다.

> **GIL은 인간과 AI의 문제 해결을 검증 가능한 실험 그래프로 만드는 local-first 실행
> 프로토콜이다.**

AI가 무엇을 했는지 나중에 추적하는 것만으로는 충분하지 않다.

GIL은 AI가 다음 행동으로 넘어가기 전에 무엇을 정의하고, 실행하고, 관측하고, 해석하고,
보고해야 하는지를 구조적으로 제한한다. 실패한 접근은 삭제하지 않으며, 과거의 Artifact 상태로
돌아가더라도 그 실패에서 얻은 지식은 잃지 않는 것을 목표로 한다.

```text
Artifact는 되돌릴 수 있다.
Journey는 되돌아가지 않는다.
```

v0의 지속적 Existence와 Journey는 프로젝트 로컬 `.gil`에 저장한다. 모델이나 세션이 바뀌어도
같은 프로젝트의 Current Existence를 읽어 동일한 존재로 작업을 이어간다. `gil start`는 최초
Interview Cycle을 연다. 설계상 사용자 Relation과 프로젝트 목표는 설정값으로 미리 지어내지
않고 그 Interview 안에서 형성하며, 사용자와 Existence는 소유자와 도구가 아니라 프로젝트를
함께 만드는 동등한 협력자다.

현재 구현은 dogfood 단계다. Bootstrap Interview, 여러 Cycle, 지속되는 Existence와 Will,
format 4 Artifact Snapshot, dirty gate, 중단 복구 가능한 `gil restore`, 그리고 필요한 규칙
하나만 조회하는 bundled Manual이 동작한다.

> **중요한 preview 제한:** 현재 Artifact 관측은 루트 `.gil/`을 제외한 모든 일반 파일을
> 관리한다. `.gitignore`를 읽지 않으며 `.git/`, `.env`, `node_modules/`, 빌드 산출물도 제외하지
> 않는다. `gil restore`는 목표 Snapshot에 없는 현재 파일을 삭제한다. 따라서 가치 있는 기존
> 저장소에서 안전한 추적 범위와 복구 가능한 격리가 구현되기 전에는 restore를 사용하지 않는다.

GIL의 주 사용자는 비개발자다. 인간은 명령과 저장 형식을 외우는 대신 지속형 **GIL Monitor**에서
현재 실험, 실패한 시도와 다음 방향을 본다. Monitor는 선택 부가기능이 아니다. 현재 기본 경로는
**MCP App fullscreen에서 가로 Graph와 채팅을 함께 보는 것**이다. Codex의 실제 화면·자동 갱신·
기본 재시작 복원을 확인했다. Claude Desktop Code 탭의 2026-11-03 실제 화면에서는 fullscreen 지원을
광고했지만 연결·렌더·재시도·클릭의 다섯 요청 모두 `inline`을 반환했다. 2026-11-09 Desktop 4.23118.0의 Host
로그에는 App 안의 새 클릭을 요구하며 프로그램 요청을 거부한 관측이 있다. 이 관측을 다른 오류의 원인으로
일반화하지 않는다. PR #107을 포함한 macOS preview.29는 명시적인 새 클릭 요구 응답을 `refused`, 타임아웃·연결 오류 등은
`error`로 구분한다. macOS preview.29와 Windows owner.15에는 이 개선과 `automatic_fullscreen` 필드가 포함된다.
이전 preview.27과 Windows owner.14에는 없다. 클릭 뒤 실제 fullscreen 성공 여부도 별도로 확인한다.
Native Companion은 같은 UI를 별도 창으로
제공하는 대안이자 Windows의 Host 연결·업데이트 관리 창이며, fullscreen 실패를 이유로 자동 실행하지
않는다. Companion 창이 열린 것을 MCP App 표시 성공으로 세지 않는다. inline 카드는 미리보기이지 설치 완료
상태가 아니다. PiP는 현재 지원을 주장하지 않는다.

소스는 [MIT 라이선스](LICENSE)로 공개한다. 현재 친구·테스터용 채널은 **서명되지 않은 preview**다.
**Windows x64 — 한 줄 설치 (2026-11-12 결정, 서명·설치 프로그램·Store 없음).** 사람은 PowerShell에 붙여 넣는다:

```powershell
irm https://raw.githubusercontent.com/hyun06000/gil/main/install.ps1 | iex
```

에이전트(Codex 또는 Claude Code 탭)에게는 한 문장만 말한다:
`https://raw.githubusercontent.com/hyun06000/gil/main/INSTALL.md 를 읽고 설치해줘`.
단일 실행 파일 `gil.exe`가 `%LOCALAPPDATA%\Programs\gil`에 놓이고 `gil install`이 Claude Desktop·Codex 설정에
스스로 등록하며 `gil doctor`가 결과를 보고한다. 첫 릴리스 `v0.3.0`은 [Windows 릴리스 절차](distribution/windows/RELEASE.md)로 게시한다.

[상세 통합 안내](distribution/INSTALL-GUIDE.md)에서 OS와 CPU에 맞는 검수본을 확인한 뒤 설치한다. macOS Apple
Silicon은 Codex Plugin `0.2.1-preview.29`, Windows x64의 구형 채널은 Companion installer
`0.1.1-owner.15`이다.
macOS preview.29와 Windows owner.15는 계획 근거의 입력 조건·줄별 일괄 오류와 PNG 제한값 안내를 보완한다.
참조 자동 이동 없이 기존 승인·기록·Snapshot을 보존한다. [Mac 게시 장부](distribution/codex/PREVIEW-29-PUBLICATION-20261110.md)와
[Windows 게시 장부](distribution/windows/PUBLICATION-OWNER-15-20261110.md)를 따른다.
**사용자에게 Node·Cargo·Homebrew 설치를 요구하지 않는다.** Mac Plugin은 별도 Companion 없이
[gil-marketplace](https://github.com/hyun06000/gil-marketplace)에서 설치하며, 범용 Plugins Directory
등재와는 다르다. Windows는 위 한 줄 설치를 사용한다.

이전 preview.6의 깨끗한 Apple Silicon Mac VM에서 Git/Xcode Command Line Tools가 없는 상태의 Git 없는 설치와
GIL 동작을 확인했다. 그 VM의 fullscreen 전환과 앱 완전 종료 뒤 복원은 별도 확인 사항이다.
Windows x64 실기기에서는 installer 설치, Companion의 Codex 연결·이전 연결 교체, 파일 생성과
사용자 버튼 업데이트를 확인했다. 두 배포 모두 OS publisher 신뢰를 제공하는 Apple
Developer ID/공증 또는 Authenticode 서명은 없다. Intel Mac, Windows ARM64와 서명된 일반 공개판은
남아 있다. OS가 실행을 막으면 보안 설정을 끄거나 ACL을 강제로 바꾸지 말고 중단한다.
[Mac 상세 안내](distribution/codex/INSTALL.md), [Windows 상세 안내](distribution/windows/INSTALL.md)와
[Windows 배포 상태](distribution/windows/README.md)를 따른다.
[시험판 조건](distribution/codex/RELEASE-macos.md)과 [보안 제보 안내](SECURITY.md)를 따른다.
Claude Desktop의 **Claude Code**는 2026-10-30 소유자 보고로 MCP App fullscreen을 정상 지원한다.
같은 날 폴더가 연결된 Claude Code 세션에서 설치된 Plugin 경로의 `show_gil_monitor`가 View를
반환했지만 사용자 화면에는 서지 않았고(10/31 이 브랜치 빌드의 marketplace 설치본도 같음), 같은 세션의
직접 등록 Monitor는 사용자가 **모니터 펼치기**로 fullscreen을 확인했다(10/31 진단, 서버 로그 일치).
따라서 Claude에서는 Desktop 설정 직접 등록이 현재의 표시 경로다. 그 경로에서 10/31까지의 빌드는 Monitor 준비
뒤에만 자동 요청을 보냈고 Host는 그것을 적용하지 않아 **첫 클릭에 fullscreen**으로 펼쳐졌다. 2026-11-02 빌드부터
App은 연결 직후에도 요청하고 **연결 정보**의 `automatic`에 모든 자동 시도와 Host 응답을 적는다. 2026-11-09
Desktop 4.23118.0 로그에서 Claude Code 탭은 그 자동 요청을 "App 안에서의 새 클릭 필요"로 거부했다.
카드 안 또는 모니터 펼치기로 재요청하며 펼쳐진 화면 자체는 사용자 확인 대상이다.
preview archive 하나에 Codex(`.agents/plugins`)와 Claude Code(`.claude-plugin`) 두 catalog를 함께 싣고,
Windows Companion은 **Codex 연결**과 **Claude Desktop 연결** 버튼으로 같은 고정 런처를 각 Host에 등록한다
(Claude 버튼은 owner.10부터, 실제 Windows Claude 화면은 사용자 확인 대기). 사용자 화면 인수와 fullscreen 중 도구 승인 창 병행은
[호출 경로별 진단](mcp-app/CLAUDE-DESKTOP-ROUTES.md)에 Host·버전별로 따로 기록하며, 한 경로의
성공을 다른 경로로 일반화하지 않는다. 자세한 범위는 아래 구현 상태와 Roadmap을 따른다.

전체 명세를 미리 읽지 않은 새 Agent 세션이 `gil context`와 주소 가능한 Help Topic만으로
실제 작업과 GIL Cycle을 완주하는 것을 Claude와 Codex 계열에서 각각 확인했다. 실패한 Cycle에서
유효한 Closed ancestor로 돌아가 그 아래에 새 시도를 여는 **Cycle-level revisit**도 CLI와
Artifact 복구까지 연결되어 있다.

---

## GIL만의 강점 — 시각 자료가 첨부된 노드 보고서

GIL의 노드 보고서는 글만 남기지 않는다. Verify가 **실제 결과를 눈으로 확인할 수 있는 PNG**(코드로 그린 플롯·
다이어그램, 헤드리스·일반 브라우저로 띄운 실제 화면의 스크린샷, 파일 변화 비교표)를 보고서에 첨부하고, 그 Cycle의
Analysis와 Cycle Report가 같은 자료를 인용한다. 그림은 원본 파일이 바뀌거나 revisit으로 되돌아가도 그 노드에
그대로 남고, Monitor는 선택한 노드에서 그림을 Report 본문보다 먼저 보여 준다. AI가 그린 상상의 이미지는 시각 자료로
치지 않는다.

이것을 모델의 선의에 맡기지 않는다. 시각 형식(5·7) 프로젝트에서 Core가 거절하는 것:

- 선언 없는 Verify close, 그리고 `없음`·`불필요`·`not needed` 같은 한마디 이유의 `not_needed`.
- 증거 파일에 PNG를 적어 놓고 그림이 필요 없다고 선언하는 것.
- Verify가 그림을 첨부했는데 그 그림을 인용하지 않은 Analysis·Cycle Report.

확인한 범위(2026-11-02): 위 세 거절과 첨부·인용 close를 Rust 단위·stdio 통합 시험으로 검사했고, 임시 폴더의
합성 데이터 분석 실례(세 그룹 평균 비교 → matplotlib 막대 그래프 첨부 → Analysis·Cycle Report 인용)를 개발 빌드의
MCP로 완주했다. 같은 Monitor 번들을 개발용 mock Host에서 실제 Core 기록으로 렌더링해 노드 배지·개수·시각 자료
패널을 확인했다. Codex·Claude Desktop의 실제 화면과 사용자가 요구하지 않을 때 모델이 스스로 그림을 만드는 비율은
아직 측정하지 않았다(합성 시나리오와 평가 케이스는 준비됨).
[시각 증거 정책](spec/GIL_Visual_Evidence_Policy_v0.1.md)을 따른다.

---

## 왜 만드는가

장시간 실제 프로젝트를 수행하는 AI에게는 다음 문제가 생긴다.

- 무엇을 해결하려 했는지보다 마지막 출력만 남는다.
- 실패한 접근과 방향 전환의 이유가 사라진다.
- 파일을 과거 상태로 되돌리면 이후에 얻은 지식까지 잃기 쉽다.
- 실행 trace는 많지만 다음 협업자가 무엇을 이어받아야 하는지 알기 어렵다.
- 인간은 Agent가 너무 멀리 진행한 뒤에야 잘못된 문제를 풀고 있었다는 사실을 발견한다.

GIL은 문제 해결을 **Step → Cycle → Chain**의 재귀적 Graph로 구성하여 이 문제를 다룬다.

```text
Project
└─ Chain Graph
   └─ Chain
      └─ Cycle Graph
         └─ Cycle
            └─ Step Graph
               └─ Step
```

- **Step** — 지금 수행할 하나의 원자적 사고 또는 행동
- **Cycle** — 고정된 문제와 성공 조건 아래 수행하는 하나의 실험
- **Chain** — 인간의 큰 질문에서 시작해 여러 실험을 묶는 탐색 범위

---

## 핵심 설계

### 선택한 프로젝트에만 적용하는 안내

GIL은 전역·프로젝트 로컬·Plugin·Skill 훅을 설치하지 않는다. 사용자가 GIL을 요청한 프로젝트에서
공통 프롬프트와 시작·재개·단계 전환의 MCP 응답으로 다음 행동을 안내한다. 일반 개발·설치·웹 조사나
다른 프로젝트에 GIL 단계·셸 래퍼·별도 권한을 요구하지 않는다. 파일 작업은 실제 열린 Verify에서
수행하도록 안내하지만 OS 쓰기 차단은 아니다. Core의 승인·전이·Report·증거·dirty 검사는 유지한다.
기존 설치의 GIL 훅 제거와 배포 버전별 적용은 [설치 안내](INSTALL.md)를 따른다.
기존 프로젝트의 새 세션도 산출물 작업 전에 context를 읽고 현재 계약에서 재개한다. 프로젝트 내부 안내의
초기 Snapshot·Verify 경계와 기존 안내 보존은 [재개 계약](distribution/codex/PROJECT-RESUME.md)을 따른다.

### Trace가 아니라 Protocol

Observability 도구는 Agent가 무엇을 했는지 기록한다.

GIL은 그보다 앞에서 **어떤 기록과 상태가 있어야 다음 행동이 허용되는지**를 정의한다.

```text
Define → Hypothesis → Verify → Analysis → Outcome
```

- Report 없이는 Step을 닫을 수 없다.
- 열린 Step을 둔 채 허용되지 않은 다음 Step으로 갈 수 없다.
- Artifact 파일 변경은 열린 Verify에서 수행하도록 안내하며, Core는 Verify에서만 Snapshot 확정을 허용한다.
- Cycle은 마지막 Outcome과 일치하는 Cycle Report가 있어야 닫힌다.

### 실패는 Graph를 성장시키는 사건

실패는 삭제하거나 성공으로 포장할 예외가 아니다.

```text
Cycle A (success)
├─ Cycle B (failure)
└─ Cycle C (new attempt)
```

실패한 Cycle은 자식을 만들지 않는다. 유효한 Closed ancestor로 revisit한 뒤 그 아래에 새로운
가지를 연다. 대상이 실패 Cycle의 직접 부모일 때 새 가지는 실패 Cycle의 형제가 된다. 실패
Report와 Knowledge는 이후 Journey에 남아 같은 시도를 반복하지 않게 한다.

### 두 개의 시간선

```text
Revisit
├─ Artifact timeline → 구조가 가리키는 snapshot으로 복원
└─ Journey timeline  → 현재까지의 경험과 지식을 유지
```

GIL에서 과거로 돌아간다는 것은 과거의 세계를 다시 선택하는 것이지, 과거의 Agent로 돌아가는
것이 아니다.

### 인간의 의도는 인간이 확정한다

Interview Cycle은 모호한 사용자 요청을 작은 명제로 나누고 AI의 해석을 인간에게 다시
확인한다.

- AI는 사용자의 응답보다 넓은 의도를 확정하지 않는다.
- Interview는 인간이 승인한 Synthesis를 근거로 성공한다.
- Chain의 최종 success 또는 failure는 Closing Interview에서 인간이 승인한다.

### Report는 인수인계 문서다

모든 Step, Cycle과 Chain Report는 다음 원칙을 따른다.

- 정의되지 않은 약어와 내부 코드명만으로 대상을 표현하지 않는다.
- 관측과 해석을 구분한다.
- 성공·실패를 기준과 증거로 추적할 수 있게 쓴다.
- 해당 Node에 참여하지 않은 협업자도 독립적으로 이해할 수 있게 쓴다.
- 세부 기록 전체를 복제하지 않고 다음 계층이 반드시 받아야 할 내용을 압축한다.

GIL은 모델의 비공개 chain-of-thought를 저장하려는 시스템이 아니다. 협업을 위해 제출 가능한
가설, 실행, 관측, 해석, 판정, 근거와 다음 방향을 기록한다.

---

## 현재 구현 상태

### 동작함

- Define → Hypothesis → Verify → Analysis → Outcome Step Grammar
- Step Open / Close와 Kind별 Report 검증
- Step-level revisit과 형제 Hypothesis
- 프로세스를 넘는 local persistence
- Bootstrap Interview와 승인된 Synthesis
- 여러 Cycle의 parent, lineage와 handoff
- 지속되는 Existence, Journey와 Active/Done Will
- Cycle Report와 기본 경로인 `gil close` (`gil cycle close`는 호환 명령)
- 마지막 Outcome만 Cycle 판정 근거로 허용
- 사람이 읽는 `gil status`와 `gil story`
- 새 Agent 세션이 이어받는 계층적 `gil context`
- Define·Outcome을 선택적으로 투영하는 협업자용 Cycle story
- 제목·개행·indentation으로 구분되는 plain text Cycle Report
- 이전 저장 형식을 조용히 무시하지 않는 복원 검사
- format 4 Snapshot registry와 내용 주소 객체 저장소
- `gil start`의 실제 프로젝트 세계 확정
- Verify close의 Snapshot 확정과 비-Verify dirty gate
- Cycle Entry/Exit Snapshot과 World Current 유도
- 프로젝트 단위 잠금과 중단 뒤 임시 객체 회수
- 현재 Snapshot과 clean/dirty를 보여 주는 `gil status`
- 중단 뒤 rollback 가능한 `gil restore`의 transaction 기반 동작. 현재 추적 범위가 지나치게
  넓으므로 일반 프로젝트의 안전한 복구 기능으로는 아직 승인하지 않음
- 바이너리에 함께 실리는 Manual과 `gil help <주제>` 조회
- 현재 상태에 맞는 Topic만 보여 주는 `gil help`
- 거절이 복구 Topic 하나를 가리키는 오류 Router
- Cycle-level `gil revisit`과 대상 Exit 세계 복원
- `parent = target`, `revisit_from = failure`, `entry = target.exit`인 새 Cycle 가지
- 실패 Report를 유지하는 pending·story·context와 `cycle/revisit` Help Topic
- 지금 상태와 그 근거를 한 화면으로 읽는 read-only `gil monitor`
- 저장해서 열 수 있는 standalone 문서 `gil monitor --html`
- `127.0.0.1`의 임의 port와 일회성 capability 주소로 여는 `gil monitor --serve`
- 파일과 `state.yaml` 변화를 debounce로 모아 전체 재조회로 수렴하는 자동 갱신
- 관측 실패를 stale로 표시하고 사건 없이도 재시도해 회복하는 갱신 상태기계
- 같은 Snapshot에서 그리는 결정적 inline SVG Cycle Graph (parent 실선 · revisit 점선)
- `지금 / 지금 할 일 / 여정 / 왜 여기 왔는가` 순서의 focus panel과 접어 둔 상세 기록
- 여러 Project를 전환하고 변화에 자동 수렴하는 macOS Native GIL Companion
- Agent가 설치된 Companion을 열거나 앞으로 가져오는 로컬 Plugin 경로
- Rust 단일 MCP 실행 파일의 17개 도구와 내장 MCP App (설치본 실행에 Node 불필요)
- Codex MCP fullscreen의 가로 DAG·접기·선택 카드·상세 Report·자동 갱신
- Codex 대화 왕복 및 Host 재시작 뒤 수동 재연결 없는 기본 Monitor 복원
- 다국어: Core 응답·안내·Monitor 화면이 en(기준·fallback)·ko·zh-Hans·zh-Hant·ja 테이블로 나뉜다.
  Monitor App은 Host의 `locale`을, Core는 `gil_start`의 선택 필드 `locale`(없으면 App이 보고한 Host locale,
  그것도 없으면 기본값 ko)을 따르고 프로젝트 단위로 기억한다. 기존 저장 형식과 Host별 분기는 바꾸지 않았다

### 확인됨

- 전체 명세 없이 Bootstrap만 받은 Agent가 오류가 가리킨 Topic 하나로 복구하고 완주
- 다른 모델 계열의 새 세션이 `gil context`만으로 열린 Verify와 Active Will을 인수인계

아직 확인되지 않은 것: 전체 토큰 절약, 모든 모델·작업에서의 일반성.

### Chain 개발 프로필 (M6-A)

`chain-core` 빌드는 실제 Chain 아래 Interview→Experiment를 저장하고 CLI/MCP/Monitor에
현재 Chain을 표시한다. [저장·호환 계약](spec/GIL_Chain_Storage_v0.1.md)을 따른다.
기본 빌드와 설치본에는 아직 활성화하지 않았다. 기존 format 4/5 기록은 자동 이전하지 않으며,
Chain 종료·인간 판정·다음 Chain은 M6-B/C에 남아 있다.

### 다음

- 사용자 UX: 프로젝트 시작과 fullscreen Monitor 연결, 노드 상세 시각 자료의 실제 Host 화면 확인과
  요구 없는 시각 자료 생성률 측정, 현재 노드·진행 상태 표시, GIL + Git/GitHub 병행·현재 상태 공유 연구
- 심화 Interview와 실제 Chain: 승인된 로드맵·기능명세·사전 문서, 추가 인터뷰를 통한
  수정, 승인 문서에 없는 지침을 실험에서 실행하지 않는 경계.
  열린 Cycle을 둔 채 다른 Cycle은 열지 않으며, 재인터뷰도 현재 Cycle을 규칙에 따라 닫은 뒤 개설
- Windows/Codex 사용자 단위 installer, 고정 실행 진입점, Companion의 사용자 버튼 업데이트와
  macOS 기준 핵심 UX 동등성. 자동 다운로드·자동 설치는 기본 동작으로 만들지 않는다.
- Windows 설치·경험 수집 뒤 홍보, 홍보 중 OpenAI와 로컬 MCP App 공식 디렉터리 노출 경로 협의
- 독립 Claude 트랙: Anthropic 설치·화면·작업 경험 강화와 공용 변경의 회귀검증

2026-10-27까지 기존 Mac, 깨끗한 Apple Silicon Mac VM과 Windows x64 실기기에서 위에 적은
preview 범위를 확인했다. 친구·테스터 검수를 일반 사용자 지원, 서명·공증 또는 공식 디렉터리
등재와 혼동하지 않는다. 기존 판독 실험과 시나리오 검증도 유지한다.

상세 진행 상황과 합격 조건은 [GIL Living Roadmap](spec/GIL_Roadmap.md)에서 추적한다.

GIL은 누적된 기록을 모든 독자에게 같은 해상도로 반복하지 않는다.

```text
gil story    인간이 현재 상황을 이해한다.
gil monitor  인간이 지금 자리와 그 근거를 한 화면으로 읽는다.
gil context  새 Agent 세션이 계층별로 압축된 지식을 이어받는다.
gil help     Agent가 지금 필요한 규칙 하나를 배운다.
```

현재 구현은 이전 Cycle을 Cycle Report, 현재 Cycle을 Step Report 해상도로 읽는다. 미래 Chain
계층에서는 이전 Chain을 Chain Report 해상도로 읽는 같은 원칙을 적용한다. 압축된 내부 Graph는
삭제되지 않으며, 전체 경로 감사 명령은 아직 구현하지 않았다.

---

## 개발자용 CLI 사용

아직 정식 릴리스가 아니다. 다음은 기여자·개발 검수용이며, 비개발자에게 요구하는 설치 절차가
아니다. Rust 개발 환경에서 저장소의 현재 작업 트리를 직접 설치한다. 일반 사용자는
[공개 preview 설치 안내](INSTALL.md)를 따른다. 일반 질문과 문제 제보는 [SUPPORT](SUPPORT.md), 기여·시험 방법은
[CONTRIBUTING](CONTRIBUTING.md)을 참고한다.

```bash
cargo install --path . --force
```

빈 작업 폴더에서 시작한다.

```bash
gil start
gil status
gil context
```

막혔을 때는 그 자리의 규칙 하나만 읽는다. 거절이 읽을 주소를 함께 알려 준다.

```bash
gil help                            # 지금 상태에 관련된 주제만
gil help artifact/dirty/non-verify  # 그 주제 하나
```

`gil start`는 먼저 Interview Cycle을 연다. 현재 자리에서 가능한 명령과 필요한 Report 필드는
`gil status`와 상태에 민감한 도움말이 안내한다. Report와 Action Context는 stdin으로 전달한다.

```bash
gil open --help
gil close --help
```

실패한 Experiment Cycle Report가 유효한 Closed ancestor를 `target_cycle_ref`로 확정하면,
대상을 명령 인수로 다시 고르지 않고 두 단계로 새 가지를 연다.

```bash
gil revisit
gil open experiment  # 또는 gil open interview
```

첫 명령은 대상 Cycle의 Exit 세계로 Artifact를 복원하되 실패 Report와 Journey를 남긴다. 두 번째
명령은 그 대상 아래에 새 Cycle을 연다.

Experiment Cycle의 Step Kind:

```text
define
hypothesis
verify
analysis
outcome
```

명령과 Report 형식은 아직 변경될 수 있다. 기존 `.gil/walk.yaml`과 format 3
`.gil/state.yaml`은 format 4로 자동 변환하지 않으며, 발견하면 기록을 보존한 채 이전
형식임을 알린다.

---

## 관련 영역과 차이

GIL은 다음 연구·제품 영역과 맞닿아 있다.

| 영역 | 대표적인 중심 | GIL이 집중하는 것 |
|---|---|---|
| Agent orchestration | durable execution, checkpoint, human-in-the-loop | 실험의 의미와 허용되는 다음 행동 |
| Agent observability | trace, 비용, 지연, 사후 디버깅 | 실행 전에 강제되는 Report와 판정 문법 |
| Agent evaluation | 데이터셋, scorer, 실행 비교 | 실제 작업 안의 가설·실패·전환과 handoff |
| Version control | 파일 변경과 복원 | Artifact 상태와 의미 있는 실험 Graph의 연결 |
| Reflective agents | 언어적 자기 피드백 | 인간과 다음 Agent가 검증하는 불변의 실패 기록 |

인접한 프로젝트와 연구:

- [LangGraph](https://langchain-ai.github.io/langgraph/index.html) — stateful Agent orchestration과 durable execution
- [LangSmith](https://www.langchain.com/langsmith-platform) — Agent observability와 evaluation
- [MLflow Tracing](https://mlflow.org/docs/latest/genai/tracing) — framework-agnostic Agent trace와 feedback
- [Weights & Biases Weave](https://docs.wandb.ai/weave/tutorial-eval) — 모델·Agent 버전과 evaluation
- [Reflexion](https://arxiv.org/abs/2303.11366) — 언어적 reflection을 이용한 Agent 학습

GIL은 이들과 같은 tracing dashboard나 범용 Agent framework가 되는 것을 우선 목표로 삼지
않는다. 어떤 Agent든 실제 프로젝트에서 검증 가능한 실험 문법을 따르게 하는 local execution
layer를 목표로 한다.

---

## 연구 질문

로드맵의 dogfood 시나리오는 다음 질문을 검증하기 위한 실험이기도 하다.

1. onboarding만으로 Agent가 GIL의 Step과 Cycle Grammar를 지키는가?
2. GIL을 사용하면 실패한 접근을 반복하는 빈도가 줄어드는가?
3. Cycle Report가 raw trace보다 협업자의 인수인계 이해도를 높이는가?
4. Artifact를 복원하면서 Knowledge를 유지하는 revisit이 다음 시도의 품질을 높이는가?
5. Interview가 모호한 사용자 의도를 더 정확한 실험 명제로 만드는가?
6. 인간은 Monitor만 보고 현재 실험, 실패 이유와 다음 방향을 이해할 수 있는가?
7. 구조적 제약이 만드는 비용은 어떤 종류의 작업에서 정당화되는가?

---

## 명세

- [GIL Specification v0.1](spec/GIL%20Specification%20v0.1.md)
- [GIL Node Model v0.1](spec/GIL_Node_Model_v0.1.md)
- [GIL Time Model v0.2](spec/GIL_Time_Model_v0.2.md)
- [GIL Cycle Model v0.1](spec/GIL_Cycle_Model_v0.1.md)
- [GIL Context Model v0.1](spec/GIL_Context_Model_v0.1.md)
- [GIL Agent UX Model v0.1](spec/GIL_Agent_UX_Model_v0.1.md)
- [GIL Existence Model v0.1](spec/GIL_Existence_Model_v0.1.md)
- [GIL Will Model v0.1](spec/GIL_Will_Model_v0.1.md)
- [GIL Manual Model v0.1](spec/GIL_Manual_Model_v0.1.md)
- [GIL Storage Model v0.1](spec/GIL_Storage_Model_v0.1.md)
- [GIL Monitor Model v0.1](spec/GIL_Monitor_Model_v0.1.md)
- [GIL Host UI Model v0.1](spec/GIL_Host_UI_Model_v0.1.md)
- [GIL Distribution Model v0.1](spec/GIL_Distribution_Model_v0.1.md)
- [GIL Open Source Readiness v0.1](spec/GIL_Open_Source_Readiness_v0.1.md)
- [Machine-readable grammar](spec/gil-spec.yaml)
- [Living Roadmap](spec/GIL_Roadmap.md)

명세가 구현보다 앞선다. 구현과 명세가 충돌하면 조용히 맞추지 않고 충돌을 드러내고 다시
결정한다.

---

## Project status

**현재 사용자 채널은 unsigned preview 둘이다.** macOS Apple Silicon은 Plugin/archive `0.2.1-preview.29`,
Windows x64는 Companion installer `0.1.1-owner.15`이며, 두 채널 모두 Codex와 Claude Desktop(Claude Code 탭)을
같은 Core·Monitor·작업 규칙으로 지원한다. 네 조합의 확인 상태(실기기 확인 / 사용자 확인 / 자동 테스트만 통과 /
사용자 확인 대기)는 [설치 안내](INSTALL.md)의 표를 따른다. 서명된 일반 공개판·Intel Mac·Windows ARM64는 없다.

앞선 Go 구현과 과거 개발 이력은 기존 개발 저장소에 남겨 두었으며, 이 `gil` 저장소에는
그 branch·tag·commit history를 가져오지 않았다. 현재 구현은 Rust로 작성하며, Git wrapper가
아니라 GIL의 개념과 불변식을 먼저 세우는 방향으로 진행한다.

MIT License.
