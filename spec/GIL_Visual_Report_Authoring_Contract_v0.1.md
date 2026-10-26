# T1.2 G5 — 시각 보고서 작성 경로 계약안

2026-10-13. **G5-A/B 개발 후보 구현·합성 stdio 검수. 공개 설치본 미적용.**
사용자가 이 계약에 따른 첨부 입력·close 연결과 합성 시험을 승인했다. 아래 최초 제안과
최신 구현 상태를 구분하며, 구체적인 개발 wire 계약은 §7을 따른다.
기준: [제품 원칙](GIL_Visual_Report_Proposal_v0.1.md),
[저장 계약](GIL_Visual_Report_Storage_Contract_v0.1.md),
[표시 계약](GIL_Visual_Report_Display_Contract_v0.1.md).

## 1. 표시 성공에서 실제 작성으로

G4에서 합성 Project의 Verify 이미지·설명·출처와 Analysis 재사용을 실제 Codex에서 확인했다.
선택한 노드의 보고서만 표시하며 Step/Cycle 자료 전환 버튼은 없다.
이는 일반 Agent가 첨부를 작성할 수 있다는 뜻은 아니다. 현재 `gil_close` 입력은
`project_root`와 텍스트 `report`이며, 이미지 확정은 실험용 Session API에만 연결돼 있다.

다음 목표는 격리된 개발 Project에서 **Agent 작성 → 원자 close → Monitor 표시 → 후속
Report 재사용**을 도구 경로로 완주하는 것이다. 사용자 데이터 이관·기본 format 변경·설치본
교체·게시를 이 목표에 포함하지 않는다. G4의 채팅 동시 사용 지속성, 재시작 화면 검수와
Windows/Claude 실측은 별도 미완 항목이다.

## 2. 사용자 경험

사용자에게 파일 ID나 JSON 작성을 요구하지 않는다. Agent는 승인받은 작업 범위 안에서
필요한 시각 자료를 계획하고 Verify에서 PNG 파일을 준비한다. 첫 저장 전에는 다음을 알린다.

> 검증한 화면과 설명을 이번 보고서에 함께 남기겠습니다. 저장한 자료는 나중에 원본 파일이
> 바뀌어도 보존되고, 모니터에서 볼 때 사용 중인 앱으로 전달됩니다. 개인정보가 있으면
> 저장 전에 가리겠습니다.

이 안내는 개인정보 전송·화면 캡처 권한을 새로 부여하지 않는다. 민감한 화면, 외부 자료,
작업 범위 밖의 캡처는 해당 권한을 먼저 확인한다. 이미지를 모델 대화나 외부 서버에 자동
첨부하지 않는다. 가림·변환은 Verify의 명시적인 파일 변경이며 원본을 몰래 가공하지 않는다.

Verify close 전 Agent는 다음 중 하나를 명시한다.

- 첨부: 실제 파일·설명·대체 텍스트·용도·출처를 확인한다. 생성 코드만으로 완료하지 않는다.
- 불필요: 왜 이미지가 필요 없는 검증인지 짧게 설명한다.
- 누락: 필요한 자료를 못 만든 이유와 후속 계획을 기록한다. 일부 첨부와 공존할 수 있다.

첨부가 누락됐다고 성공을 자동 실패로 바꾸지 않는다. 성공 조건은 기존 규칙으로 판단한다.
추가 시각화가 필요하면 현재 Cycle을 규칙에 맞게 닫고 다음 Cycle의 Verify에서 만든다.
열린 Cycle을 둔 채 다른 Cycle을 열거나 닫힌 Report를 보충하는 경로는 없다.

## 3. 쓰기 경계와 도구 설계 제안

별도의 사후 업로드 도구 대신 기존 close 동작에 **typed 시각 입력을 함께 전달하는 확장**을
제안한다. 정확한 CLI 문법·MCP schema·action surface 호환 범위는 구현 전 별도 확정한다.
이 제안 당시에는 실행 코드 또는 설치 Skill을 바꾸지 않았다. 이후 승인된 구현은 §7에 기록한다.

입력은 논리적으로 다음을 구분한다.

| 입력 | 의미 | 허용 자리 |
| --- | --- | --- |
| 새 자료 | Project 상대경로, 출처 종류·설명, 사용 설명 | 열린 Verify만 |
| 기존 자료 인용 | 같은 Project의 확정 asset ID, 보고서별 설명·alt·용도 | 열린 Report |
| Verify 선언 | attached / not_needed / missing 및 필요한 이유·후속 계획 | 새 형식의 Verify 필수 |

새 자료 입력은 요청 안에서만 유효한 식별자로 연결하고 서버가 영구 asset ID를 할당한다.
클라이언트가 미래 ID나 digest를 추측하지 않는다. ID는 wire에서 문자열로 다룬다.
기존 자료 ID는 서버가 실제 확정 receipt 또는 해당 보고서의 참조 목록으로 제공한 것만 쓴다.
모델이 사용할 참조 메타데이터 조회 경로도 명시적으로 설계한다. 현재 App-only 조회가 모델에도
자동 노출된다고 가정하거나 UI 내부를 읽어 ID를 수집하도록 Skill에 지시하지 않는다.

서버는 현재 열린 Report를 잠금 안에서 확인하고 예상 대상 참조와 일치할 때만 닫는다.
지연된 호출이 다음 Report를 실수로 닫지 않도록 예상 owner 검사를 계약에 포함한다.
비-Verify의 새 파일 등록, 다른 Project의 참조, 아직 확정되지 않은 참조는 거절한다.
텍스트 report 안의 Markdown/JSON/경로를 첨부 권한으로 해석하지 않는다.

기존 format 4의 텍스트 close는 그대로 유지한다. 시각 입력을 받았는데 지원하지 않으면
명시적으로 거절하며, 조용히 이미지만 버리고 Report를 닫지 않는다. format 5의 Verify는
선언 생략을 거절한다. 과거 미기재 Report에 이유를 소급 작성하지 않는다.

## 4. 안전한 확정과 응답

기존 Session의 경로·파일·PNG 디코딩·Snapshot 일치 검사를 재사용한다. 첫 범위는 정적 PNG,
파일 16 MiB, 최대 8개·합계 32 MiB이며 기존 차원·픽셀 한도도 유지한다. 다른 종류의 파일을
Artifact 추적에서 임의 제외한다는 뜻은 아니다. 외부 URL·절대경로·링크 우회는 허용하지 않는다.

Report, 시각 선언, 참조, Snapshot, Journey를 한 state commit으로 확정한다.
새 객체 저장 뒤 state 실패로 미참조 객체가 남을 수 있으나 부분 Report 성공은 없다.
receipt는 확정된 owner, 자료 ID 및 사용 설명의 확인 정보를 돌려주고 원본 절대경로나 bytes를
로그에 남기지 않는다. 저장 성공·Monitor 데이터 준비·사용자 화면 표시는 구분해 보고한다.

응답 유실이나 내구성 오류 뒤 close를 자동 반복하지 않는다. 잠금을 다시 얻고 예상 Report와
시각 참조를 조회해 확정됨 / 미확정 / 판정 불가를 구분한다. 확정됐으면 기존 receipt 정보를
읽고, 미확정이면 입력 파일을 다시 검증한 뒤 재시도한다. 판정 불가면 보존하고 멈춘다.
이 확인 경로까지 시험하기 전 쓰기 도구를 제품에 노출하지 않는다.

## 5. 재사용과 추가 작업

Verify에서 확정한 A를 Analysis·Outcome·Cycle Report·다음 Define에서 인용할 수 있다.
각 보고서의 설명은 독립적이고 원래 Verify 출처와 bytes는 유지한다. 재사용을 새 관측으로
보고하지 않는다. 재캡처·재가공한 B는 새 Verify에서 새 자료로 확정한다.
Monitor는 선택한 노드가 인용한 자료만 표시한다. 전체 Cycle 자료를 자동 합쳐 보여주지 않는다.

## 6. 구현 순서와 검수

| 단계 | 작업 | 통과 조건 |
| --- | --- | --- |
| G5-A | typed 입력·예상 owner·receipt·메타데이터 조회 계약 확정 | 기존 호출/구형 Project와 오류 의미 비교, 승인 후 구현 |
| G5-B | 실험용 format-5 fixture의 Session/adapter 작성 연결 | 실제 stdio에서 close→재사용→읽기, 실패 원자성 |
| G5-C | 후보 Skill의 작성 안내와 합성 종단 시나리오 | 코드만 생성하고 첨부됐다고 주장하지 않음, 누락/불필요 구분 |
| G5-D | 기존 Project 이관·백업·구형 writer 검수 | 별도 승인, 기존 기록 보존과 복구 가능한 백업 검증 |
| G5-E | 플랫폼·패키징·실제 Host 검수 | macOS/Windows 각각 증거, 후보 설치·게시 별도 승인 |

필수 시나리오:

1. PNG 한 장과 여러 장의 순서·caption·alt·출처 보존, 설명에 HTML이 있어도 비실행.
2. not_needed, missing, 일부 첨부+missing, 선언 생략과 모순된 선언.
3. 닫힌 Report 재첨부, 다른 단계의 파일 등록, 다른 Project 참조, 오래된 예상 owner 거절.
4. 원본 교체·삭제 후 원래 이미지 표시; Analysis/Outcome/Cycle/다음 Define에서 재사용.
5. 경로 탈출·링크·파일 교체·손상 PNG·한도 초과 거절, 오류 때 Report/세계 상태 보존.
6. 저장 전 실패·객체 저장 후 실패·commit 뒤 응답 유실·동시 close, 중복 참조 방지.
7. format 4 텍스트 회귀, 시각 입력 명시 거절, 자동 이관 없음.
8. 실제 Host에서 새 Verify 노드 이미지와 채팅 확인; 앱/서버 재시작 뒤 동일 자료 조회.
9. 안전한 첨부 구현이 없는 플랫폼은 명시적으로 거절. 빌드 성공을 Windows 쓰기 검수로 세지 않음.

이 표는 **계획이며 전체 통과 장부가 아니다**. 이번에 실행한 범위는 §8에 구분한다.

## 7. 개발 후보 wire 계약

`cargo build --locked -p gil --features visual-authoring`로 명시적으로 켠다.
기본 기능에는 포함하지 않으며 기본 `tools.json`·CLI 문법·descriptor 판·Project
기본 format은 바꾸지 않았다. 후보 Skill의 조건부 작성 안내는 G5-C에서 추가했다.
이 개발 빌드의 배포 호환성 판정은 G5-E에 남는다.
기본 빌드는 19개 도구, 개발 빌드는 모델 전용 `gil_report_visuals`가 추가된 20개 도구다.
App-only 이미지 조회는 그대로이며 App에 쓰기 도구를 노출하지 않는다.

개발 빌드의 `gil_close`는 기존 호출 또는 아래 typed 호출을 받는다.

```json
{
  "project_root": "<사용자가 선택한 절대경로>",
  "report": "execution: 실제 실행과 관측\nresult: 결과\n",
  "expected_owner": {"kind":"step", "ref":"step:C2/S3"},
  "visuals": {
    "attachments": [{"key":"screen", "path":"evidence/screen.png",
      "source_kind":"captured", "source_description":"검증 중 직접 캡처한 화면"}],
    "uses": [{"source":{"kind":"new", "key":"screen"},
      "caption":"검증한 화면", "alt":"화면의 핵심 내용", "purpose":"성공 조건의 관측 근거"}],
    "declaration": {"state":"attached"}
  }
}
```

보고서 본문은 기존 단계별 문법을 그대로 통과해야 한다. 위 예시는 첨부 구조 설명이며 특정
실험의 성공을 대신 확정하는 템플릿이 아니다. 새 자료 key는 중복·미사용을 거절한다.
attachments/uses는 각각 최대 8개, 설명 문자열은 UTF-8 4096 bytes 이하, report는
256 KiB 이하, typed 요청 JSON은 512 KiB 이하다. 정적 PNG의 기존 한도도 별도로 검사한다.

기존 자료의 source는 아래와 같다. 이 값은 앞선 receipt/조회에서 받은 것을 그대로 쓴다.

```json
{"kind":"stored", "owner":{"kind":"step","ref":"step:C2/S3"},
 "asset_id":"1", "sha256":"<확정 자료의 64자리 sha256>"}
```

원출처 보고서에 실제로 연결된 자료를 **현재 요청의 Project 안에서** 검증하고 digest를
대조한다. digest만으로 접근 권한을 부여하지 않는다. 다른 Report에서의 새 사용 설명은
caption/alt/purpose에 적는다. 새 파일이 없으면 attachments는 빈 배열이다.
비-Verify에서는 declaration을 생략한다(null은 거절). Verify에서 not_needed는 이유와 빈
uses, missing은 이유·follow_up 및 선택적 uses를 받는다. 선언 없는 format-5 Verify close는
기존 텍스트 호출로 우회할 수 없다. format-4 typed close는 `visual_upgrade_required`로 거절한다.

읽기 전용 `gil_report_visuals` 입력은 project_root와 owner다. 정확한 root만 사용하고,
잠긴 ReadOnlySession으로 조회한다. recovery·원본 파일 읽기·이미지 bytes 전송은 하지 않는다.
닫힌 보고서면 `confirmed:true`, 열린 보고서면 false이며, 찾을 수 없거나 판독이 불가능하면
오류다. `report_sha256`는 정렬된 Report 필드 맵의 serde_json UTF-8 직렬화 SHA-256이다.
시각 목록에는 기존 projection의 자료 ID·설명·출처·digest가 포함된다.

typed close 성공도 이 projection을 반환한다. `display_confirmed:false`는 저장 성공을
화면 관측으로 오해하지 않도록 한다. confirmed는 저장된 보고서의 존재이지 특정 요청자가
그것을 썼다는 인증은 아니다. 응답 유실 뒤에는 report_sha256와 시각 목록을 예상 내용과
비교하고 일치하지 않거나 조회가 실패하면 재시도하지 않는다. receipt에는 이미지 bytes나
첨부 원본 절대경로를 넣지 않는다. 판독 응답의 전체 1 MiB 상한도 유지한다.

기본 빌드에서는 visuals/expected_owner 입력을 조용히 무시하지 않고 거절한다.
새 CLI 쓰기 명령, 사용자 프로젝트 이관, 설치본의 작성 안내와 실제 설치는 아직 없다.

## 8. 구현 및 검수 기록

- 새 자료 확정은 기존 Session의 bounded PNG·경로·Snapshot 검증과 원자 commit을 사용한다.
- 비-Verify/최종 Cycle의 참조 close도 Session의 dirty gate를 거치고 복제 후보를 저장한 뒤
  메모리에 반영한다. 예상 owner를 잠금 안에서 확인해 지연된 요청의 다른 Report close를 막는다.
- 실제 stdio 합성 시험: 새 PNG 확정 → 서버 재시작 후 동일 receipt 조회 → Analysis,
  Outcome, 최종 Cycle, 다음 Cycle Define의 재사용. 원본 PNG 삭제 후 저장 이미지 조회.
- 거절/보존: 오래된 owner, 경로 탈출, 손상 PNG, 잘못된 Report, 잘못된 digest,
  알 수 없는 필드, 미사용 생성 key, dirty 세계, format-4 시각 입력.
- 선언: not_needed, attached, 일부 첨부+missing. format-5 선언 생략 거절.
- 장애: Verify 및 참조 close의 state 쓰기 실패 후 기록 byte 불변·열림 확인, 확인 후 재시도.
  성공 응답 유실을 가정한 새 서버의 읽기 확인, 두 서버의 동시 close 중 정확히 하나만 성공.
- 단위 시험: null/공백/필수 값 누락/비정규 주소/크기 초과·명시적 opt-in 경계.

첫 컴파일은 source_kind 필드 오기, 확장 시험은 Opened receipt와 StepRef 타입 혼동으로
실패했고 보정했다. 첫 전체 회귀에서 기본 19개 도구 기대값이 개발 기능의 20개와 달랐으며
기능별 계약으로 보정했다. 고지 시험은 Cargo.toml의 기능 선언 추가로 지문이 달라 실패했고,
의존성·라이선스 변경이 없음을 확인해 세 정책의 입력 지문만 갱신했다.

이는 기존 합성 fixture를 통한 G5-A/B 개발 빌드 시험이다. 후속 캡처·화면 검수는 §9와
구분한다. 디스크 고갈/강제 종료의 모든 시점, Windows 첨부 안전성,
사용자 이관·공개 배포까지 완료했다는 뜻은 아니다.

최종 검증: 개발 기능을 켠 전체 Core suite 1,306 통과(33개 결과 그룹), 이후 동시 close
시나리오 추가 후 MCP 파일 전체 21 통과. 기본 기능 전체 Core suite 1,303 통과(33개 그룹).
JS·패키징·Windows inventory 합계 179건 중 175 통과/기존 4 skip/0 실패.
개발 기능 on/off 양쪽 all-targets 빌드와 `git diff --check` 통과.
Windows inventory는 Windows 실행 시험이 아니다. 마지막 로컬 바이너리는 기본 기능으로
빌드했으며, 실행 중인 G4 연결이나 공개 Plugin 설치본을 교체하지 않았다.

## 9. G5-C 후보 지침 및 실제 캡처 검수 (2026-10-13)

후보 SKILL.md에 지원 schema와 모델 조회 도구의 동시 확인, 승인된 범위 캡처와 보존 안내,
Verify의 세 선언, typed close, 불확실 응답 후 읽기 대조, 같은 Project 재사용 및 닫힌
보고서 불변 지침을 추가했다. 기본 설치본에서 기능을 추측하거나 자동 이관하지 않는다.
단일 Skill 파일에 조건부 절을 두어 현재 패키징의 허용 파일 목록은 변경하지 않았다.

검수 fixture는 `examples/visual_g5_prepare.rs`가 **존재하지 않는 새 디렉터리만** 받아
open Verify까지 준비한다. 앞선 Interview는 합성 이력이며 실제 사용자 인터뷰로 세지 않는다.
그 뒤 Agent가 로컬 HTML 곱셈 계산기를 작성하고 실제 Chrome UI에서 12와 7을 입력해
`12 × 7 = 84`를 관측했다. CUA JPEG 캡처를 PNG로 변환했고 변환 사실을 출처에 기록했다.

- 별도 개발 바이너리의 실제 stdio `tools/list`로 authoring 지원을 확인한 뒤
  `gil_report_visuals`의 미확정 응답을 확인하고 typed `gil_close`를 한 번 호출했다.
  설치된 Plugin 도구로 작성한 것이 아니라 격리된 개발 stdio 클라이언트 검수다.
- Verify C2/S3에 captured PNG 1개(1470×836, 113,573 bytes)가 확정됐다.
  새 서버 프로세스의 메타데이터 조회에서 보고서 SHA-256, 이미지 SHA-256, caption을
  원래 입력과 대조했다. 응답 확인을 위해 close를 반복하지 않았다.
- 기존 승인된 G4 임시 MCP 서버로 이 새 scope의 표시를 요청했다. 사용자가 새 카드를
  펼친 뒤 실제 Codex MCP App에서 해당 Verify의 이미지·caption·용도·출처를 DOM,
  naturalWidth/Height 및 스크린샷으로 확인했다. 프로젝트 기록은 clean이었다.
- Define 노드를 선택하면 시각 자료가 없고, Verify로 돌아오면 동일한 이미지·설명·출처가
  다시 표시됐다. Step/Cycle 자료 전환 버튼 없이 선택 보고서에만 연결되는 경계를 확인했다.
- 기본 CSP를 유지했다. Blob이 차단된 뒤 같은 검증 bytes의 data PNG 표시가 성공했다.
- 기존 G4 Project·사용자 Project·공개 Plugin 설치본은 수정하지 않았다. 로컬 웹 서버는
  합성 계산기 검수용일 뿐 Monitor의 대체 서버가 아니다.

후보 Skill 형식 검사 및 계약 12건 통과. JS·패키징·Windows inventory 합계 181건 중
177 통과, 기존 4 skip. 개발 기능 MCP 21건은 최초 샌드박스 실행에서 기존 OS watcher
시험 1건이 30초 타임아웃(20 통과)했고, 권한을 받은 샌드박스 밖 재실행에서는 21 통과했다.
지침 시험은 정적 계약 검사이며 모든 Agent의 준수를 증명하지 않는다. 실제 UI 검수는
macOS/Codex 한 환경이며 채팅 동시 사용 지속성·앱 완전 재시작·설치 후보 배포·Windows
첨부 작성 검수를 대신하지 않는다.
기본 기능 all-targets 빌드와 `git diff --check`도 통과했다. 이번 턴은 Core 동작을
바꾸지 않았으며 Core 전체 suite는 §8의 이전 결과와 구분한다.

## 10. G5-E 첫 조각 — 기본 읽기 패키지 검수 (2026-10-14)

G5-D 합성 이관 및 Host 재연결 검수 다음 단계다. 읽기 배포물과 실험용 작성 배포물을
구분한다. 이번에는 기존 기본 기능으로만 로컬 macOS arm64 개발 패키지를 만들며
`visual-authoring` 활성화, 신규 Project의 기본 format 변경, 자동 이관은 하지 않는다.
작성 기능을 사용자가 설치할 수 있는 후보는 별도 호환성·설치 검수 후 판단한다.

기존 macOS smoke 및 Windows spike의 17개 기대값이 현재 기본 19개 도구와 어긋남을
발견했다. 공통 `distribution/codex/tool-contract.mjs`로 이름 집합·중복·입력 schema·
visibility·annotations를 검사한다. 이미지 조회 두 도구는 app-only/read-only/명시적
scope+owner를 유지하고, 기본 close의 typed 첨부 확장과 개발용 `gil_report_visuals`
혼입을 거절한다. 도구 수만 느슨하게 늘려 통과시키지 않는다.

검수 결과:

- 신규 계약 시험 4건 포함 배포·고지·Windows inventory **88 통과 / 0 실패**.
- 공용 UI·Plugin **93 통과 / 0 실패 / 기존 4 skip**.
- 공용 native spike를 macOS arm64 기본 바이너리에서 실행: 19개 도구, 내장 UI 해시,
  합성 Project prepare/read, OS watcher 확인. Windows 실행 결과가 아니다.
- `build-preview.mjs`로 release 빌드, 독립 패키지 생성, 압축/해제, 파일·권한·고지·
  로컬 경로 유출 검사, 최소 PATH에서 패키지 실행·stdio·내장 HTML 해시 검수 통과.
  아카이브 SHA-256: `a8e3139d167019cedec7cce059d88b276f8d5d30dad5ad17934462566e2ff3fe`.
  개발 dirty source snapshot이 receipt에 기록되며 `publishable:false`다.
- 패키지 속 실행 파일로 G5-D의 이관된 합성 Project를 읽었다. 별도 프로세스 종료/재시작
  후 prepare 없이 binding을 복원하고 과거 Define/Hypothesis·새 Verify 및 PNG 1,301 bytes가
  동일함을 확인했다. 프로젝트·백업 전체 지문은 G5-D 기준과 같다. Node는 검수 harness에만
  사용하고 자식 실행 환경 PATH는 `/usr/bin:/bin`으로 제한했다.

제품 Rust 코드는 변경하지 않아 전체 Core suite를 재실행하지 않았다. 위 패키지 빌드와
실제 패키지 stdio 시험은 이번 실행 결과다. 설치본 교체·공개 게시·CI dispatch·사용자 기록
이관은 하지 않았다. 패키지 설치 후 Host 화면, 작성 기능 포함 후보의 호환성, Windows native
첨부·이관 안전성은 미완이다. 다음은 작성 후보의 명시적 기능/형식 지원 계약을 확정하고
격리된 패키지에서 작성→재사용→재시작 검수를 연결하는 것이다. 읽기 패키지 성공을
일반 사용자의 이미지 작성 기능 출시로 표시하지 않는다.

## 11. G5-E 작성 개발 패키지 종단 검수 (2026-10-14)

사용자 승인 범위는 격리된 개발 패키지와 합성 Project다. `build-preview.mjs`의 명시적
`--visual-authoring` 옵션만 기능을 켜며 기본 실행은 이전과 동일하다. 개발 receipt에
`development_features:["visual-authoring"]`, 검사 결과에 profile을 남기고 PREVIEW에
실험용 작성 기능임을 표시한다. 공개 unsigned staging/prepare는 이 receipt를 거절한다.
기존 설치본, marketplace, 기본 Project format과 자동 이관 정책은 변경하지 않았다.

기본 19개/작성 20개 도구 계약을 분리 검수한다. 작성 profile은 typed close schema와
모델 전용 확인 도구를 정확히 검사하고, 나머지 도구와 App-only 이미지 읽기 권한은 기본
계약을 그대로 따른다. feature 선택 자체가 제품 호환성 승인이나 배포 승인은 아니다.

`examples/visual_package_fixture.rs`는 존재하지 않는 절대경로에만 합성 format-5 열린
Verify와 2×2 회색 PNG를 준비한다. 실제 사용자 인터뷰·실험 증거가 아니다. 검수 harness는
패키지 무결성을 먼저 검사하고 패키지 속 실행 파일에 stdio로 다음을 순차 수행했다.

1. 모델 조회로 Verify 미확정 확인 → 경로 탈출 첨부 거절 및 state 불변 → 정상 typed close.
2. 저장 receipt의 confirmed와 이미지 digest 확인. display_confirmed는 false다.
3. 첫 프로세스 종료 확인 → 새 프로세스에서 close 반복 없이 같은 receipt 조회.
4. MCP open/close로 Analysis·Outcome·Cycle Report·다음 Cycle Define에서 같은 asset을
   재사용. 각 Report의 독립 caption과 원래 Verify 출처·digest를 대조했다. Cycle은 정상
   종료한 뒤 다음 Cycle을 열었으며 revisit/닫힌 Report 수정은 하지 않았다.
5. 합성 입력 PNG만 삭제 → 두 번째 서버 종료 → 세 번째 서버에서 prepare 없이 scope 복구.
   다섯 Report의 메타데이터·본문 digest와 공용 App 조립 코드로 읽은 PNG bytes가 같다.
   최종 읽기 전후 state도 byte 동일하다. 원본 PNG 삭제는 이 합성 검수물에만 적용했다.

최종 개발 아카이브 SHA-256:
`b636f05fc2768e743b71191435b0c24c1b115b462f83d0b332d886a8c59d28f2`.
PNG 71 bytes SHA-256:
`fafaf93be0fce38b4190bb868deaa141decf6e3d8e5dd1dfc08d62c1f534162f`.
3개 프로세스/서버 세대가 서로 다름을 확인했다. 자식 PATH는 `/usr/bin:/bin`이며 Node는
개발 검수 harness에서만 쓴다. release 빌드·압축 왕복·최소 PATH 실행·20개 도구·내장 UI
해시·고지/무결성 검수와 **배포 회귀 90건 통과**. 제품 Rust 로직은 변경하지 않았고
전체 Core/UI suite는 이번에 재실행하지 않았다.

첫 회귀는 새 테스트를 비동기 manifest 초기화보다 먼저 등록해 1건 실패했다. 등록 순서를
고쳐 90건을 재실행했다. 첫 종단 harness는 이미지 읽기에 scope를 전달하지 않아 Verify
저장 뒤 멈췄다. 해당 기록을 보존하고 harness를 수정한 뒤 새 합성 Project에서 완주했다.
최종 소스 스냅샷으로 패키지를 다시 만들고 또 다른 새 합성 Project에서도 완주했다.

이는 실제 패키지 바이너리의 작성·재사용·재시작 검수다. Codex 설치본 Agent 호출과 화면,
Windows native 쓰기/이관, 실제 사용자 이관, 공개 배포는 아직 미검수/미실행이다. 다음은
별도 승인된 임시 연결로 이 패키지의 도구와 Skill을 실제 Host에서 함께 인수하는 것이다.

## 12. G5-E Codex 임시 연결 작성·노드 검수 (2026-10-14)

사용자가 연결한 `gil-visual-authoring-check` 개발 패키지의 실제 모델 노출 도구로
독립 합성 Project의 열린 Verify `step:C2/S3`를 처리했다. 설치된 preview 플러그인은
교체하지 않았다. 개발 패키지 Skill은 Agent가 명시적으로 읽고 적용했으며, 정식 설치로
Skill이 자동 로드되는 UX를 검수한 것은 아니다.

- schema와 context/help, `gil_report_visuals`의 `open_report`를 확인한 뒤, 생성 자료임을
  명시한 71-byte PNG와 caption/alt/purpose를 typed close 한 번으로 확정했다.
- `confirmed:true`, `display_confirmed:false` receipt와 후속 읽기 결과가 완전히 같다.
  Report digest는 `fd7e57b3430372619ff46a33ad114b0a4c0a954c4f84b84f7a07625066958619`다.
  PNG digest는 §11의 합성 PNG와 같다. 저장 응답을 화면 성공으로 해석하지 않았다.
- prepare/show를 한 번씩 호출했다. 실제 Codex MCP App 탭의 해당 Project를 확인하고
  Verify 노드를 선택했다. 닫힌 Report 본문, 이미지, caption, 용도, 생성 출처와 원래 Verify가
  DOM에 나타났다. 이미지 `complete:true`, 자연 크기 2×2와 화면 data PNG의 SHA-256이
  저장 receipt와 같음을 확인했다. Blob CSP 차단 뒤 동일 bytes의 data PNG 표시 경로이며
  보안 설정은 변경하지 않았다.
- 실제 App 스크린샷에서 DAG·선택 노드·Report·caption을 확인했다. 2×2 fixture는 작은
  점으로 표시되므로 일반 캡처 이미지의 가독성 합격 증거로 사용하지 않는다. App은 펼친
  상태를 표시하지만, Host 채팅과의 동시 가시성은 이번 캡처 범위 밖이며 사용자 확인이 남는다.

이번 변경은 검수 문서뿐이며 제품 코드·기존 설치본·실제 사용자 기록을 바꾸지 않았다.
전체 회귀 suite는 재실행하지 않았다. 이 임시 연결의 재시작 보존, 설치 Skill 자동 인수,
Windows native 작성/이관과 공개 배포까지 통과한 것으로 확대하지 않는다.

## 13. 실제 크기 자료의 후속 Report 인용 (2026-10-14)

사용자의 Esc·스크롤 실측 통과를 표시 계약과 로드맵에 기록한 뒤, 같은 임시 개발 연결의
합성 Project에서 Analysis `step:C3/S4`를 순서대로 열고 닫았다. 새 파일은 만들지 않고
Verify `step:C3/S3`의 asset `2`를 stored 참조로 인용했다. 새 설명·용도는 분석의 목적을
명시하며, 이전 캡처가 확대 조작을 새로 촬영한 증거는 아님을 구분한다.

- 이미지: 638×875, 209,990 bytes,
  SHA-256 `2a7f8cf696e8b0d9d53e1466bebdea9918d6074bcebac9d9cea8c26b8b67088c`.
- Analysis Report 확정 지문:
  `e9741d783ce74e7fe49e9b2237510e2f2da3ebe635622df2653c2755f3703751`.
- 기존 Verify Report 재조회 지문:
  `5ce92967ee78e90f6b041a5f1c74fe666b3f2b2038ca0a750a5f50b065fe2b5e`로 이전과 동일.
  원래 Verify 참조·captured 출처 설명·이미지 지문도 유지됐다.
- 실제 Codex에서 Analysis 노드의 이미지·새 설명·용도·원래 Verify 출처를 확인했다.
  클릭 확대는 정확히 638×875, 재클릭 복귀는 약 414.69×568.74였다.
- 쓰기 중 Monitor에 일시적 busy 안내와 마지막 보고서가 남았다. 쓰기 완료 후 Analysis를
  선택하자 오류가 해소되고 이미지가 로드됐다. 자동 복구까지 통과했다고 주장하지 않는다.

기존 설치본·사용자 기록·CSP는 변경하지 않았다. C3는 열린 채이며 다음 단계는 Outcome이다.
최종 Cycle Report 인용과 이 실제 크기 자료의 서버 재시작 보존은 후속 검수로 남긴다.

## 14. 최종 Cycle Report 인용과 독립 서버 재시작 (2026-10-14)

§13 이후 같은 합성 Project에서 Outcome `step:C3/S5`를 success로 확정한 뒤 Cycle C3를
동일 verdict로 닫았다. 새 Cycle은 열지 않았다. 최종 Report는 원래 Verify의 asset 2를
stored 참조하고 독립 caption/alt/purpose를 가지며 이미지 파일을 새로 만들지 않는다.
최종 Report 지문은 `4cc3b9285fdf557e3557c586fa1a72ad0c300d535da98b84230261abfd2af125`다.

같은 개발 패키지의 별도 stdio 서버를 실행·종료하고 새 프로세스로 시작했다. 종료된 PID의
소멸과 서로 다른 두 epoch를 확인했다. 두 번째 연결에서는 prepare를 반복하지 않았으며
저장된 scope로 Verify/Analysis/Cycle Report를 읽었다. 세 receipt 전체가 같고 각각의
Report 이미지 응답을 실제 조립한 209,990 bytes의 SHA-256도 §13의 원본과 같았다.
검수 전후 `.gil` 파일 8개의 이름·내용 지문 모두 불변이다. 원본 파일 삭제나 사용자 기록
이관은 하지 않았다. 독립 검수 설정 디렉터리 외 기존 설치 설정은 변경하지 않았다.

한계: 이 시험은 Codex에 연결된 서버 자체의 재시작이 아니라 동일 패키지의 프로세스 경계
보존 시험이다. Host 재연결 뒤 실제 화면 인수는 남는다. 현재 UI의 상세 renderer는
`{kind:'step', ref:node.step_ref}`만 선택하며 Cycle 버튼은 접기·펼치기만 한다.
따라서 Cycle Report 저장·이미지 조회 성공을 최종 Cycle 이미지의 화면 성공으로 세지 않는다.
다음 구현 후보는 기존 Step/Cycle 자료 전환 탭을 되살리는 것이 아니라, Cycle Report 자체를
명시적으로 선택하는 읽기 전용 진입점이다. 별도 명세 검토 후 진행한다.

## 15. Cycle 선택과 실제 Host 재연결 인수 (2026-10-14)

§14의 UI 한계는 명시적인 `사이클 보고서` 선택란으로 해소했다. 사용자가 임시 서버를
재연결한 뒤 새 Monitor에서 C3를 선택했다. canonical View의 보고서 요약과 Cycle owner가
인용한 이미지가 표시됐다. Step/Cycle 자료 전환 탭은 되살리지 않았다.

- 이미지 complete=true, 자연 크기 638×875. 설명은 `사이클 결론의 근거 — 기존 Verify 캡처`.
- 독립 용도·captured 출처 설명·원래 Verify `step:C3/S3`가 화면에 유지된다.
- 원본 크기 확대 및 미리보기 복귀를 실제 App에서 실행했다.
- Report 지문은 §14와 같고 이미지 지문은 §13과 같다. 쓰기 도구는 호출하지 않았다.
- Blob 차단 뒤 동일 검증 PNG의 data 표시를 사용했다. CSP 완화·별도 창 실행은 없다.

이는 macOS Codex 임시 개발 패키지의 재연결 후 선택·표시 인수다. 완벽한 마지막 선택의
자동 복원, Host 채팅 동시 가시성의 별도 사람 확인, 정식 설치 Skill 자동 로딩, Windows
native 작성/이관, 기존 사용자 기록 이관·공개 배포는 별도다. 다음은 현재 소스의 기본/
visual-authoring 전체 회귀를 대조하고, 이 미완 게이트를 유지한 개발 후보 인수 정리다.

## 16. 다음 개발 후보의 잔여 게이트 (2026-10-14)

기존 설치본과 사용자 기록을 보존한 채 다음 순서로 진행한다.

1. 기본/visual-authoring Core 전체 회귀와 공용 UI·패키징 계약을 대조한다. skip/ignored는
   통과로 더하지 않는다. 특히 명시적 옛 release 실행 파일을 요구하는 old-writer 시험은
   일반 suite에서 ignored이며, 이전 개별 검수와 이번 실행 결과를 혼동하지 않는다.
2. 설치 후보의 조건부 Skill 자동 로딩은 임시 MCP 수동 Skill 적용과 별도로 검수한다.
   기존 설치본 교체 없이 검수 가능한 격리 범위를 먼저 정하고, 설치 변경은 별도 승인한다.
3. Windows native 파일 핸들/링크/교체 및 이관 안전성을 별도로 닫는다. 현재 비-Unix
   새 첨부 경로는 명시적 거절이며 기능 동등성을 주장하지 않는다.
4. 실제 사용자 이관은 합성 검수 통과와 별개로 백업·복구·명시적 대상 승인을 요구한다.
   기본 format 변경·자동 이관·공개 staging의 개발 기능 차단 해제는 이번 작업이 아니다.

현재 개발 후보의 공개 staging 차단은 유지한다. 이 목록은 배포 승인이나 설치 요청이 아니다.

통합 회귀 실행 결과: `cargo test --locked -p gil` 1,318 통과 / 0 실패 / 2 ignored,
`cargo test --locked -p gil --features visual-authoring` 1,322 통과 / 0 실패 / 2 ignored
(각 34개 결과 그룹 합산). ignored는 crash-child 전용 helper와 명시적 구형 실행 파일이
필요한 old-writer 검사다. 최초 기본 suite는 샌드박스의 loopback bind 금지로 library
441 통과/50 실패/1 ignored에서 중단됐다. `Operation not permitted`를 확인하고 승인된
실행 권한으로 기본·작성 기능 전체를 순차 재실행해 위 결과를 얻었다. 제품 보안 설정이나
시험 assertion은 바꾸지 않았다. JS/UI/Plugin/패키징·고지 합계 178 통과 / 0 실패 /
4 기존 skip. `git diff --check` 정상. 이번 턴 제품 코드 변경은 없고 검수 문서만 갱신했다.
Tauri·Windows native·실제 사용자 이관·설치 변경·배포는 실행하지 않았다.

## 17. 격리 설치와 Skill 자동 발견 (2026-10-14)

사용자 승인으로 기존 설치본과 별개의 임시 Codex 프로필에 §15 개발 패키지를 공식 CLI
marketplace add → plugin add로 설치했다. 자식 프로세스에만 Codex 전용 home 경로를
지정했고 기존 인증 정보를 복사하지 않았다. 제품 이름·marketplace identity는 유지했다.
새 app-server의 `skills/list(forceReload:true)`가 후보 캐시의
`gil-companion-prototype:gil-companion`을 enabled=true로 자동 발견했고 오류는 없었다.
Skill 본문을 이 요청에 직접 주입하지 않았다. 설치본 Skill bytes는 패키지와 동일하며
SHA-256은 `794077fd470ec110ae123e3e3bceb8287d7f31ee655beff1d15f6bbf2316b583`이다.
typed close 지원 확인·세 선언·receipt 조회·재사용 경계가 들어 있다.

격리 설치 버전은 개발 패키지의 `0.2.0+codex.20260927174145`다. 공개 preview.3보다
새로운 배포 버전이라는 뜻이 아니며 release용 version 정리는 별도다. 기존 사용자 설정과
기존 preview.3 Skill의 전후 지문은 동일했다. 사용자 Project는 호출하지 않았다.

**부분 통과:** 설치 → Host Skill 발견 및 본문 일치는 확인했지만 모델의 자동 선택·본문
로드·실제 지침 준수는 아직 아니다. 격리 프로필의 `codex login status`가 Not logged in을
반환했다. 인증 정보를 기존 프로필에서 복제하거나 인증 경계를 우회하지 않았다.
다음은 사용자가 격리 프로필에 로그인한 뒤, 지침 본문/파일 경로를 prompt에 넣지 않은
자연어 요청으로 후보 Skill 선택·로드 흔적을 검사하는 것이다. 최초 요청은 읽기 전용으로
첨부 가능 여부와 작성 절차만 묻고, 합성 Project 외 쓰기·설치 변경은 금지한다. 단순한
정답 문구나 skills/list 성공을 모델 지침 준수로 세지 않는다. Desktop 화면 인수와도 구분한다.

### 17.1 로그인 후 실제 Agent 읽기 전용 검수 (2026-10-14)

사용자가 격리 프로필에 직접 로그인한 뒤, 새 ephemeral Codex 실행에 Skill 이름·경로·
본문을 넣지 않고 GIL 이미지 저장·최종 보고서 재사용·응답 유실·닫힌 보고서의 누락을
질문했다. 프로젝트 미선택 및 모든 작성/화면 열기 금지를 명시했다. Agent 자체 sandbox는
read-only를 유지했고 모델을 따로 지정하지 않았다.

첫 실행은 네트워크 제한으로 실패했다. 네트워크 승인 후 실행은 중첩 macOS sandbox에서
Skill 읽기가 막혀 도구 명세만으로 답했으므로 로드 통과로 세지 않는다. 외부 harness 실행
권한 승인 뒤 재실행에서는 후보 캐시의 SKILL.md를 자발적으로 cat한 command_execution이
exit 0으로 끝났다. 읽은 전체 출력 SHA-256은 §17의 패키지 Skill 지문과 정확히 같다.

**통과 범위:** 자연어 요청 → Skill 자동 선택 → 전체 본문 읽기 → 읽기 전용 작성 안내.
최종 답변에서 capability gate, 열린 Verify에서 PNG 작성, 사전 저장/전달 안내, 세 선언,
같은 Project의 owner/asset_id/sha256 재사용, 응답 유실 시 close 자동 반복 금지 및 digest
비교, 닫힌 보고서 불변·다음 순차 Cycle의 Verify, 저장/표시 성공 분리를 확인했다.
실제 작성 실행 준수·Desktop 자동 로드·반복 신뢰도 검수는 이번 판정에 포함하지 않는다.

실제 도구 호출은 gil_companion_status와 gil_help뿐이었다. 도움말 topic `report`를
추측해 한 번 거절받은 뒤 topic 없는 도움말을 읽었다. 안내 정확성의 미비이며 도움말 문법
성공으로 세지 않는다. manifest의 interface.defaultPrompt가 128자 초과로 무시된다는
Host 경고도 남았다. Skill 로드는 성공했지만 배포 전 별도 정리가 필요하다.

기존 사용자 config와 preview.3 Skill은 이전 지문과 동일했고 격리 workspace는 비어 있었다.
사용자 Project 호출·기존 설치본 교체·캡처·첨부·확정·이관·게시는 하지 않았다.
원본은 저장소 밖 임시 프로필 옆 behavior-readonly-host-events.jsonl 및
behavior-readonly-host-final.txt에 보관한다. 인증 데이터는 기록물에 복사하지 않았다.

## 18. 격리 Agent의 실제 작성·재사용 (2026-10-14)

§17.1의 두 안내 결함을 좁게 수정했다. Skill은 topic 없는 gil_help에서 시작해 실제 반환된
주소만 사용하도록 안내하며 report/close/visuals를 검색어처럼 추측하지 않는다. 연결 schema로
typed 첨부 지원을 판단한다. Codex manifest의 defaultPrompt는 103자로 줄였고 128자 상한
회귀시험을 추가했다. Core·저장 형식·도구 schema·identity는 바꾸지 않았다.

새 개발 작성 패키지의 archive SHA-256은
`a061d8970c2eed98de3817a019105319d7c2e4ab403524a017ffdf7ed78ec887`이다.
roundtrip/20-tool smoke가 통과했다. 공식 CLI로 **기존 임시 프로필 안의 후보만** 교체했다.
marketplace source 변경은 최초 add가 기존 source 충돌로 거절되어, 임시 marketplace를
공식 remove 후 add했다. 실제 사용자 설치본이나 registry를 손편집하지 않았다.

`examples/visual_agent_fixture.rs`가 새 폴더에 합성 Interview/Define/Hypothesis와 열린
Verify를 준비한다. 기존 디렉터리는 거절하며 사용자 이관 경로가 아니다. Agent에게는
선택한 합성 폴더와 “두 막대 PNG 작성·Verify 첨부·최종 Cycle에서 재사용”이라는 자연어
요청만 주고 Skill 이름·경로·본문이나 정답 호출을 넣지 않았다. workspace-write sandbox를
유지하고 다른 Project·설정·설치·화면 열기·다운로드를 금지했다.

**실제 관측:** 후보 Skill 전체 자동 로드, 저장 전 안내, topic 없는 도움말 → 반환된 주소,
Agent의 생성 스크립트 작성 → 실제 PNG 생성/파일 검사 → typed Verify close → Analysis →
Outcome → typed Cycle close → 두 보고서 receipt 조회가 이어졌다. source_kind는 generated이며
실제 사용자 캡처로 보고하지 않았다. 새 Cycle은 열지 않았다. 시작 프롬프트 경고와 알 수 없는
help topic 거절은 이번 실행에서 0건이었다.

**독립 감사 통과:** Verify step:C2/S3와 cycle:C2가 같은 asset 1 및 SHA-256
`4c98db47fb3eabb6039a41ac43cd2edff279582ed22e27d535b9be435a30f740`을 참조한다.
이미지는 640×400 PNG 4,532 bytes다. 원본 파일과 저장 blob은 byte 동일, 설명은 Report별로
다르며 원출처는 Verify다. 자산 registry는 1개다. 합성 기존 Report 7개가 모두 같고 기존
객체 파일 지문도 같다. Agent 종료 뒤 별도 서버를 두 번 새로 열어 읽은 두 receipt는 Agent가
받은 확정 응답과 동일하며, 읽기 전후 state도 동일했다. 감사자가 실제 PNG도 열어 확인했다.

**오류와 한계:** 무오류 실행은 아니다. Agent가 expected_owner의 step: 접두사를 빠뜨려
close 입력이 거절됐다. 조회로 미확정을 확인하고 열린 owner·파일 digest를 재확인한 뒤
수정해 확정했다. Analysis open은 goal/action 대신 objective/next_action이 필요하다는
거절을 받아 수정했고, Report 없는 gil_cycle close도 거절된 뒤 typed gil_close로 마무리했다.
모두 trace에 남겼으며 성공 응답 유실을 실제 주입한 시험은 아니다. Agent는 이미지 뷰어로
봤다고 적었지만 CLI JSONL에는 해당 뷰어 이벤트가 없어 그 부분을 독립 확인했다고 세지
않는다(파일 생성/검사 trace와 감사자의 별도 시각 확인은 있다). 단일 Agent 실행의 완주이며
모든 모델의 자동 준수·실제 Desktop 렌더링 성공으로 일반화하지 않는다.

Skill/manifest 집중 시험 22 통과, Skill validator 정상. Plugin/패키징/고지 회귀는
137건 중 133 통과·실패 0·기존 skip 4. 개발 release 빌드·새 fixture 실행 통과.
제품 Rust 소스가 바뀌지 않아 전체 Rust suite는 재실행하지 않았다. 기존 사용자 config와
preview.3 Skill 지문은 동일하다. 기존 설치본·사용자 Project·공개 배포는 바꾸지 않았다.
원본 trace·최종 답변·독립 감사 JSON은 임시 검수 프로필 옆 authoring-agent-events.jsonl,
authoring-agent-final.txt, authoring-audit.json으로 저장소 밖에 보관한다.

## 19. Agent 작성 보고서의 Desktop Monitor 연결 검수 (2026-10-14)

§18의 합성 Project를 기존 임시 작성 검수 서버로 prepare/show한 뒤 사용자가 새 카드를
펼쳤다. 실제 Codex MCP App에서 Verify step:C2/S3와 cycle:C2를 차례로 선택했다.
두 선택 모두 이미지 로딩 완료와 원본 크기 640×400을 DOM에서 확인했다. Verify의 생성
설명과 최종 Cycle의 재사용 설명은 서로 다르고, 두 보고서 모두 생성 자료라는 출처 설명과
원래 Verify step:C2/S3를 유지한다. Cycle 이미지의 원본 크기 보기와 미리보기 복귀도
실제 UI 상태 전환으로 확인했다. CSP 완화 없이 기존 검증 bytes의 data PNG 경로가 로드됐다.

**판정 범위:** 실제 Host App 연결·보고서 선택·이미지 로딩·설명·출처 DOM 검수는 통과했다.
App 스크린샷은 그래프와 보고서를 보여주지만 이미지가 보이는 하단까지 스크롤되지 않았다.
현재 자동 제어로 이 App의 스크롤을 수행할 수 없어 사용자에게 하단 확인을 요청했고,
사용자가 이미지·설명·출처가 잘 보인다고 확인했다. 따라서 C2 사이클 보고서의 하단
시각 자료 가독성은 사용자 인수 완료다. 전체 Codex 채팅 입력창과 이미지의 동시 가시성을
별도로 명시한 확인은 아니므로 그 항목까지 확대 판정하지 않는다.
이 검수는 임시 서버를 통한 표시이며, 격리 CLI 후보가 Desktop에 자동
설치·로드됐다는 검수가 아니다. 공개 설치본을 교체하지 않았다.

화면 조작 뒤 Project 기존 파일과 사용자 config·preview.3 Skill의 SHA-256이 검수 전과
동일했다. Project 재작성·새 Cycle·Companion 실행은 하지 않았다. 화면 캡처는 저장소 밖에
보관하며 제품 코드 변경이나 배포는 없다.

## 20. 배포 후보를 위한 최종 변경 감사 (2026-10-14)

**결론: macOS arm64의 격리·합성 개발 후보로 유지 가능. 일반 사용자용 이미지 작성
시험판으로의 승격은 보류한다.** 이번 감사는 설치·게시·기본 format 전환 승인이 아니다.

### 변경 범위와 패키지 대응

기준 HEAD는 `a2e4f073ad0437860b6e42f430f9e5253d6f2463`, 작업 브랜치는
`recovery/t1-2-20261013`이다. 변경은 109파일(수정 44, 신규 65)이다. Cargo 2,
src 28, tests 9, examples 6, UI 4, MCP App 10, Plugin 6, 배포/고지 38, 명세 6으로
구분했다. 이미지 화면만의 패치가 아니라 저장·잠금·복원 경계 강화까지 포함한다. PR에서
이 경계를 별도 검토 항목으로 다루며, 보호 코드를 화면 패키지에서 임의로 빼지 않는다.

검수된 작성 패키지의 archive SHA-256은 §18과 동일하다. 파일 allowlist·bytes·권한·
manifest·고지·archive digest를 재검사했고 현재 Skill/manifest/UI와 일치했다. Agent 작성
완료 체크포인트 이후 달라진 파일은 Roadmap과 이 계약 문서 두 개뿐이다. 기존 패키지의
source receipt는 이전 빌드 시점 그대로 보존한다. 현재 문서를 포함한 새 후보로 제출할 때는
새 source snapshot으로 재빌드해야 하며 과거 receipt를 최신 소스 증명으로 바꿔 쓰지 않는다.

변경 파일의 NUL·개인 개발 경로·대표 credential 패턴 및 `.gil`·설정·캐시·실행 파일·이미지
혼입 검사는 발견 0건이었다. 제품 패키지는 allowlist만 싣고 example·합성 Project·Agent
trace·인증 자료를 싣지 않는다. 이는 제한된 패턴 검사이며 모든 비밀이나 취약점 부재를
증명하지 않는다. 이번 감사에서 외부 취약점 DB 신규 조회나 Windows native 검수는 안 했다.

### 안전 경계 검토와 재시험

typed close의 예상 owner·같은 Project의 저장 참조/digest, App-only 읽기 schema,
보고서 범위 이미지 조회, PNG 전체 디코딩/용량 한도, Unix no-follow 파일 읽기·재검증,
state의 임시 파일 배타 생성/동기화/원자 교체, DOM textContent와 검증 PNG 표시 경로를
점검했다. 이미지 조회는 외부 URL/임의 경로를 받지 않고 CSP를 완화하지 않는다.
검토한 범위에서 새 차단 결함을 찾지 못했으며 아래 배포 게이트는 그대로 유지한다.

- 기본 Core 전체: **1,318 통과 / 0 실패 / 2 ignored**, 34 결과 그룹.
- visual-authoring Core 전체: **1,322 통과 / 0 실패 / 2 ignored**, 34 결과 그룹.
- UI/Plugin/macOS 패키징/고지/Windows 정적 JS: **188 통과 / 0 실패 / 4 기존 skip**.
- `cargo build --locked -p gil --all-targets`, macOS 고지 93패키지 대조,
  `git diff --check`: 통과. Windows 정적 시험은 Windows 실행 성공이 아니다.
- ignored는 명시적 구형 writer 검수와 강제종료 자식 helper다. 이번 전체 suite에서 구형
  writer의 개별 검수를 다시 실행한 것은 아니며 이전 이관 장부의 증거와 구분한다.

최초 sandbox 실행은 loopback bind 거절로 442 통과/50 실패/1 ignored에서 중단됐다.
허용된 환경에서 재실행할 때 감사자가 기본/작성 빌드를 같은 target에 병렬 실행해,
MCP 시험이 공용 `target/debug/gil`의 기본 19도구 실행 파일을 읽는 간섭이 발생했다
(작성 suite의 MCP 시험 4건 실패). 기본 실행 종료 뒤 작성 suite를 **단독** 재실행해 위
전체 통과 결과를 얻었다. assertion이나 제품 보안 설정은 바꾸지 않았다. 후속 검수는
두 feature 구성을 순차 실행하거나 target 디렉터리를 분리해야 한다.
고지 도구에 `--help`를 보낸 첫 호출은 지원하지 않는 인수로 거절됐으며 안내된 `check`로
재실행해 통과했다. 이런 실행 오류도 무오류 검수로 보고하지 않는다.

### 공개 후보 승격 전 남은 결정

1. 신규 Project는 여전히 format 4이며 typed 첨부에는 명시적 format 5가 필요하다.
   현재 이관 도구는 개발 example이다. 따라서 현 패키지만 설치한다고 일반 사용자가
   곧바로 이미지 첨부를 시작할 수 있다는 안내는 틀리다. 신규 생성/기존 Project 승인 이관의
   제품 경로와 실패·복구 안내를 먼저 확정한다. 자동 이관은 추가하지 않았다.
2. descriptor의 storage 범위는 여전히 4..4, action surface는 1이다. format 5와 typed
   작성 schema를 지원하는 공개 후보의 호환성 계약을 확정하고 실제 descriptor·패키징·
   업데이트/구형 writer 시험을 함께 맞춰야 한다. 숫자만 임의로 올리지 않는다.
3. 후보 manifest는 여전히 개발 버전 `0.2.0+codex.20260927174145`이며 dirty 소스다.
   검토된 PR/커밋·새 후보 버전·고정 source 증거가 필요하다. 공개 preview와 같은 버전으로
   다른 bytes를 덮어쓰지 않는다. `development_features`와 `publishable:false`, 공개
   staging의 authoring 차단은 유지한다.
4. Windows의 안전한 이미지 파일 읽기/쓰기 및 원본 이관은 미검수/거절 경로다. 이번 macOS
   검수로 기능 동등성을 선언하지 않는다. macOS 한정 후보와 Windows 후속 검수는 구분한다.

기존 합성 Project 파일 8개와 사용자 config/preview.3 Skill 2개 지문은 이전과 같았다.
실제 사용자 Project를 열거나 이관하지 않았다. 이번에는 감사 명세/장부만 갱신했으며
제품 코드 수정·커밋·push·CI dispatch·설치 교체·공개 staging·게시는 하지 않았다.
파일별 지문과 패턴 검사 결과는 저장소 밖 `final-candidate-audit-20261014.json`에 보존한다.

## 21. 일반 사용자 진입 절차 확정 (2026-10-14, 구현 전)

제품 절차의 정본은 [이관 계약 §5](GIL_Visual_Report_Migration_v0.1.md#5-일반-사용자용-생성승인-이관-계약--확정-구현-전-2026-10-14)다.
지원되는 향후 작성 후보는 신규 Project를 최초 commit부터 format 5로 생성한다. 사용자에게
형식 번호를 선택시키지 않으며 시작 요청 → 가로/fullscreen Monitor → 첫 Interview를 유지한다.
기존 4는 읽기/Monitor/Plugin 업데이트만으로 바꾸지 않는다. 이미지 쓰기에는 백업 준비 승인과
검수 결과를 본 뒤의 원본 적용 승인을 분리한다. 열린 Cycle/Step을 유지하고 새 Cycle을 만들지
않는다. 과거 Report는 미기재 그대로 보존한다. 응답 유실 때 자동 apply/close를 반복하지 않는다.

완전한 작성 후보의 제품 목표는 format 지원 4..5·신규 기본 5·action surface 2다. 실제 코드,
descriptor, schema, 업데이트 정책과 이전 binary 검수를 함께 바꾸기 전에는 광고하지 않는다.
현재 개발 기능과 staging 차단은 그대로 유지한다. 첫 구현은 신규 생성의 원자성/실패 보존과
첫 Verify 선언을 합성 Project에서 검사하는 N1/N2다. 기존 사용자 이관은 별도 승인 대상이며,
이 문서 확정으로 설치본 변경·제품 구현·게시·시험 통과를 주장하지 않는다.

### 후속 N1/N2 구현

개발 library의 `ProjectSession::start_visual`과 합성 실패 보존 검수를 추가했다.
최초 format 5 확정과 첫 Verify 선언 요구를 확인했으며 상세는 이관 계약 §6에 기록한다.
기존 CLI/MCP start와 설치본 기본값·제품 capability 선언은 바꾸지 않았다. 다음 N3에서
이 생성 Core를 Agent 시작 흐름에 연결하고 새 생성·기존 4/5·불명 상태를 구분해 검수한다.

### 후속 N3 개발 도구 연결

개발 feature의 `gil_start`를 연결했다. 후보 Skill은 생성 안내와 실패 시 보존/읽기 확인을
구분하며 `gil_status/context`도 비수정 판독 경로를 쓴다. 새 Project의 첨부·재사용 stdio
검수와 한계는 이관 계약 §7을 따른다. 기본 빌드/CLI/설치본은 교체하지 않았다. 실제 새 후보의
Host 화면·독립 Agent 실행은 이 연결 시점에는 미검수였다. 후속 격리 인수와 사용자 확인은
이관 계약 §8을 따른다. 완전한 작성 capability나 게시 완료로 세지 않는다.

## 22. 관측 방법을 구분하는 작성 지침 (2026-10-14)

N3 실제 Agent 검수에서 파일 디코딩/픽셀 검사만으로 ‘이미지를 열람했다’고 서술한 문제를
후보 Skill의 좁은 보정으로 다룬다. 저장 schema·도구·닫힌 Report는 변경하지 않는다.

- **파일 검사:** 디코딩, 크기, hash, 실제 픽셀 assertion 등 실행한 검사와 그 결과만 보고한다.
- **이미지 열람:** 실제 이미지 입력/열람 도구를 통해 렌더링된 이미지를 관측했을 때만
  그렇게 보고한다. 생성 코드·metadata·첨부 성공·디코딩 성공은 열람이나 가독성의 증거가 아니다.
- 열람하지 않았다면 ‘파일 검사 통과, 이미지 직접 열람/가독성 미실시’를 명시한다.
  유효한 첨부는 저장할 수 있고, 열람 미실시만으로 첨부 자체를 `missing`으로 바꾸지 않는다.
  가독성이 성공 조건에 포함된 경우에는 그 조건을 미확인으로 남긴다.
- 나중의 사용자 Monitor 확인은 그 사람과 시점의 관측이다. 작성 Agent의 과거 관측으로
  소급하지 않고 닫힌 보고서도 고치지 않는다. 추가 작업은 기존 새 Cycle 규칙을 따른다.

정형화된 새 필드나 형식 선택 질문을 추가하지 않는다. 기존 Report 본문에서 검사 방법과
결과를 정직하게 구분하는 지침이며 모든 작업에 이미지 열람을 강제하는 정책이 아니다.

### 격리 Agent 재검수

보정 Skill을 새 macOS 개발 패키지로 묶어 기존 로그인된 **격리 프로필만** 공식 Plugin
명령으로 교체했다. 기존 일반 설치본, 이전 합성 프로젝트와 닫힌 보고서는 그대로 두었다.
새 합성 Project에서 다음 큰 흐름 두 개를 실제 Agent로 확인했다.

1. **작성·파일 검사·저장·재사용:** 파일 검사만 수행하고 직접 이미지 열람은 하지 않는
   조건을 명시했다. 후보 Skill을 자동으로 읽은 Agent가 새 format-5 Project의 Interview와
   실험 하나를 완주하고 960×540 합성 PNG를 Verify에 첨부, 최종 Cycle에서 같은 자산을
   재사용했다. Verify/최종 Report와 최종 답변 모두 PNG 디코딩·CRC·크기·픽셀 검사를
   구체적으로 적고, 이미지 직접 열람·가독성·Host 검수를 **미실시**로 명시했다.
   첨부 선언은 `attached`로 유지했다. 이미지 SHA-256은
   `bbe708bf1a3fe04a7f102c2c77506492d02fc151cff1fed1f545b2502cb8b345`이다.
2. **실제 이미지 입력의 별도 관측:** 위 PNG를 새 읽기 전용 Agent의 이미지 입력으로
   전달했다. Agent는 파랑 A=30/주황 B=60, 같은 너비·밑선, 약 두 배 높이를 설명했고,
   이를 이번 열람의 별도 관측으로 한정했다. 파일 검사 명령이나 GIL 도구는 실행하지
   않았다. 이전 보고서로 소급하지 않았으며 기존/신규 Project와 설치 설정 보호 파일
   **26개 지문 불변**을 확인했다.

이는 ‘직접 열람 금지’를 명시한 통제 시나리오와 실제 이미지 입력의 대비 시험이다.
임의의 모든 Agent가 암묵적인 상황에서도 항상 정확히 구분한다는 보장은 아니다. 이전
N3 보고서의 근거 부족 문구는 그대로 보존하며, 후속 인수로 과거 관측을 증명하지 않는다.
새 Monitor 화면 검수는 반복하지 않았고 앞선 사용자 화면 인수 범위를 유지한다.

첫 작성 실행에서 문법/인수 거절 **9건**이 있었으며 Agent가 거절·조회 결과를 읽고
수정한 뒤 완료했다. 무실패 UX로 세지 않는다. 이미지 입력 검수의 첫 실행은 CLI의
이미지 옵션이 프롬프트까지 인수로 소비해 Agent 실행 전에 실패했다. 인수 구분자를
고친 두 번째 실행이 통과했으며 실패 로그는 보존했다. 제품 코드 변경은 없었다.

검증: Skill validator, 기존 Skill 계약 12건, JS/UI/Plugin **98 통과 / 4 기존 skip**,
개발 패키지의 20도구 smoke·압축 왕복 통과. 새 문자열 매칭 시험으로 행동 통과를 대신하지
않았다. Rust 구현은 변경하지 않아 전체 Rust suite는 다시 실행하지 않았다. package는
`publishable: false`이며 archive SHA-256은
`a7c4f1ccd50b3ff85ece6ffa298805f5e732097db77fd3db15f0d5001876daba`이다.
도구 schema·저장 형식·설치본·기존 Report 수정·게시·커밋·push는 없다.

## 23. Windows 작성 경계 단계 1 (2026-10-18)

§6 항목 9("안전한 첨부 구현이 없는 플랫폼은 명시적으로 거절")의 Windows 구현을 시작했다.
범위·위협·인수 기준은 `GIL_Windows_Visual_Support_Contract_v0.1.md`, 저장 경계 상세는 저장
계약 §25다. 새 format-5 생성·Verify 첨부·저장 이미지 인용·재시작 보존만 대상이며 Windows
이관은 계속 거절한다. 작성 profile의 Windows CI(`windows-spike.yml` matrix `visual-authoring`)가
Rust 경계 시험과 24-도구 smoke를 실행한다. 빌드·runner 성공을 실제 PC·Host 화면 인수로
세지 않으며, Skill·설치 문서의 Windows 안내는 runner 결과 뒤 별도 갱신한다.
