# GIL Runtime Update — U1 설계 결정안 v0.1

2026-10-23. [상위 계약](GIL_Runtime_Update_Contract_v0.1.md)의 U1 구체화.
**문서 설계 / 제품 미구현.** 새 MCP 도구·명령·receipt schema를 현재 제공한다고 주장하지 않는다.
기존 설치본·manifest·Skill·저장 형식과 unsigned 승인 정책은 변경하지 않는다.

## 1. 소스 대조와 구현 범위

- `plugins/gil-companion-prototype/.codex-plugin/plugin.json`: 함께 실은 Core에
  `mcp --serve`를 직접 전달한다. Windows는 `distribution/windows/candidate.mjs`가
  해당 CPU의 Core 경로로 manifest를 생성한다. 고정 실행기는 아직 없다.
- `src/agent.rs`: `--gil-agent-descriptor`가 protocol/action surface/storage 범위를 제공한다.
  설치 권한이나 특정 Core 쌍의 저장 호환성을 인증하는 API는 아니다.
- `distribution/codex/compatibility.mjs`: macOS/aarch64 검사다. Windows 검증기로 재사용하지 않는다.
- `distribution/codex/update-policy.mjs`: 제작자용 macOS release-record 검사다. `install_authorized`
  와 `publishable`은 false이며 `migration: requires_review` 또는 계약 차이는 차단한다.
  Windows 개발 version의 build metadata도 기존 release version 문법과 다르다. 이를 우회하기
  위해 입력 라벨·migration 선언을 바꾸지 않는다. 새 플랫폼 지원은 별도 schema/시험 변경이다.

첫 범위는 **동일 Plugin/Skill 계약 내 native Core-only 전환**이다. 새 도구 추가·schema 변경,
Skill 변경, 저장 migration, launcher 교체를 이 경로에 섞지 않는다. 지금의 dev 후보들을
아무 쌍이나 즉시 전환할 수 있다는 뜻이 아니다.

## 2. 구성 결정

| 구성 | 제안 역할 | 변경 주체/경계 |
| --- | --- | --- |
| Host Plugin | native 실행기, bootstrap Core, Skill, 신뢰·호환 기준 포함 | 공식 Host 설치/교체만. 자기 cache 편집 금지 |
| native 실행기 | 선택 검증, Core 시작, stdio 전달, 자식 수명 관리 | Rust 구현 방향. Node 설치 불요. 네트워크 자동 다운로드 없음 |
| native 업데이트 처리부 | 승인된 묶음 검증·준비·선택 확정·상태 조회 | 실행기와 같은 배포 단위의 기능으로 검토. 상주 서비스/관리자 broker는 도입하지 않음 |
| 버전 저장소 | immutable Core 묶음, 선택 기록, 최소 상태·증거 | 공식적으로 허용된 GIL 소유 영역. 실제 위치/접근 정책은 §3 검증 전 미확정 |
| Agent | 지원 확인·변경 설명·승인 전달·실행 결과 확인 | 셸 스크립트를 즉석 작성해 runtime을 갱신하지 않음. 제공 표면/schema는 후속 설계 |

실행기는 연결마다 한 Core만 시작한다. 자식에 MCP stdio를 전달하고 진단은 stderr로만 낸다.
Host stdin EOF를 자식에 전달하며 정상 종료를 기다린다. 고아 프로세스 방지·종료 제한 시간은
U2 시험으로 정한다. 기존 사용자 Core를 PID 이름으로 찾아 종료하지 않는다.
실행기 자신도 Host가 실행하는 바이너리이므로 자기 교체 문제는 남는다. 별도 제거/갱신 인수 없이
"설치 캐시 잠금 해결"이라고 말하지 않는다.

## 3. 저장 위치와 계정 — 먼저 닫아야 할 설계 조건

후보 위치는 OS의 사용자별 비동기화 로컬 앱 데이터 영역 아래 GIL 전용 저장소다. **경로 문자열을
확정하지 않는다.** 다음 조건을 만족하는 한 위치를 검증·확정한 뒤에만 제품에 적용한다.

1. Agent 셸과 MCP 서버가 동일한 사용자/저장소를 보는지 확인한다. 프로세스 환경 변수나
   sandbox 임시 디렉터리를 지속 저장소의 정본으로 삼지 않는다. 다른 토큰이면 공유 가능성을
   추측하지 않고 거절한다. 기존 권한을 넓히거나 관리자 계정을 중개하지 않는다.
2. 공식 Host 정책상 그 영역의 실행·쓰기와 외부 Core 선택이 허용돼야 한다. 별도 App Server
   실험 성공은 이 조건의 확인이 아니다. 허용되지 않으면 기능을 중단하고 공식 Plugin 교체만 남긴다.
3. 사용자·publisher/channel·OS/CPU·Plugin 계약 cohort별 namespace를 분리한다. 경로 이름에
   외부 입력을 그대로 넣지 않는다. 계정·소유권·reparse·hardlink·동기화 영역을 검증한다.
4. 같은 실행 컨텍스트에서 버전 생성·파일 공개·선택 commit·새 프로세스 실행이 가능한지 본다.
   현재 관측된 디렉터리 rename EPERM을 피하기 위해 임의 위치를 순회하지 않는다.
5. probe와 설정은 임시 시험 영역에서만 검증한다. 기존 사용자 설치/Project를 접근성 시험에 쓰지 않는다.

이 검증이 끝나기 전 제품 경로 자동 생성, 기존 설치본 이동, 시스템/사용자 환경 변수 등록은 없다.

## 4. 계약 cohort와 선택 기록

cohort는 Plugin identity, Skill bytes, launcher ABI, MCP tool/schema, protocol/action surface,
Monitor/NodeDetail schema 및 지원 profile을 함께 식별하는 **제안된 배포 계약 ID**다.
일치하지 않는 판은 Core-only 경로에서 거절한다. Core 내 Manual/UI와 Skill 의미 호환은
실제 쌍 시험으로 확인하며 hash 일치만으로 판정하지 않는다.

현재 대화가 실제 로드한 Skill을 실행기가 알아낼 수 있다고 가정하지 않는다. 첫 범위는 Skill과
도구 계약이 바뀌지 않는 cohort로 제한한다. Plugin/Skill 교체는 별도 공식 절차와 새 연결 인수를
필요로 한다. 다중 Core writer의 같은 Project 동시 사용도 U3 검수 전에는 지원하지 않는다.

선택 기록의 최소 제안 필드: schema, generation, cohort, OS/CPU, release ID,
manifest digest, previous selection digest, 승인·검수 근거 식별자. 이것은 기존 receipt schema가 아니다.
문자열 version 정렬이나 "가장 새 디렉터리" 탐색으로 실행판을 선택하지 않는다.

승인은 정확한 후보 digest·대상 cohort·이전 선택 generation과 결합한다. 요청 중 선택이 바뀌면
충돌로 중단한다. 로컬 `approved:true` 자체를 사용자 승인이나 publisher 인증의 증거로 쓰지 않는다.
서명/배포 출처 검증과 사용자 승인 전달 방식은 독립 검토 대상이다. 기존 `requires_review` 판은
명시적 쌍 검수 없이는 거절하고 선언을 `none`으로 바꾸지 않는다.

## 5. 상태와 실패 동작

| 상태 | 실행 동작 | 사용자에게 말할 수 있는 것 |
| --- | --- | --- |
| 최초 미설정 | 공식 Plugin의 검증된 bootstrap Core만 사용 | 기본 버전 실행 중 |
| 후보 준비/검증 중 | 기존 선택 유지, 미완성 묶음은 선택 불가 | 후보 준비 중 |
| 선택 확정 | 기존 프로세스 불변, 다음 연결만 새 판 | 다음 연결부터 적용 예정 |
| 새 연결 응답 확인 | 실제 build/cohort 확인 후 적용 판정 | 새 연결에 적용 확인 |
| 선택/묶음 불일치·손상 | 실행 거절, 임의 구판 fallback 금지 | 실행판 확인 실패, 기록 보존 |
| 정책/권한 차단 | 선택 변경 금지, 강제 해결 없음 | 이 환경에서 업데이트 차단 |

최초 미설정과 기존 선택 손실을 구분하는 저장소 초기화 상태가 필요하다. 사용 이력 있는
저장소에서 선택 파일이 없어졌다고 bootstrap으로 조용히 downgrade하지 않는다. 이 구분과
불완전 초기화 복구는 U2 항목이다. 새 Core에 세션을 넘긴 뒤에는 자동 rollback하지 않는다.

다운로드·압축 검증 → immutable 묶음 완성 확인 → 격리 probe → cohort/쌍 검수·승인 확인 →
generation 비교 → 선택 commit 순서다. 구현용 원자성 원시 연산은 아직 확정하지 않는다.
U2에서 디렉터리 rename 의존 방식과 최종 위치 배타 생성+완성 마커 방식을 비교할 수 있으나,
완성 마커나 선택 파일 일반 덮어쓰기를 안전한 commit으로 간주하지 않는다. 부분 파일·경합·
디스크 고갈·중단 때 신규 실행이 미검증 bytes를 볼 수 없어야 한다.

## 6. 초기 도입과 업데이트 표면

새 설치는 bootstrap Core가 포함된 자족형 Plugin을 공식 경로로 받는 방향이다. 기존 direct-Core
설치본의 최초 교체는 여전히 공식 Host가 수행해야 한다. 실행 중 cache 삭제 실패에 대한
비개발자 완료 경로는 미확정이며, 새 marketplace 이름으로 우회해 두 GIL을 노출하지 않는다.

초기 도입, Core 선택, Skill 변경, 실행기 자체 변경, 제거는 각각 승인/인수 범위가 다르다.
현재 없는 updater 도구를 Skill에 쓰거나 내부 App Server API를 제품의 원격 제어 수단으로
채택하지 않는다. Agent용 status/plan/prepare/select의 구체 schema와 사용자 승인 결합은 후속이다.
제품화 전 준비된 상태 조회 수단이 실제 실행 build와 다음 선택을 구별해야 한다.

## 7. U1 판정과 다음 실험

결정한 방향: native 고정 실행기, bundled bootstrap, 같은 cohort의 Core-only 전환, 기존 연결
고정, 권한 차단 시 중단, 자동 GC/rollback 없음. **U1 전체 완료는 아니다.**

남은 결정:
- 지속 저장소의 공식 Host 허용 범위·실제 토큰별 접근 가능성.
- native 실행기의 자식 수명, 초기화/선택 공개 원자성 및 안전 원시 연산.
- publisher 신뢰 근거·승인 결합 schema·cohort 및 쌍 검수 record 규격.
- 기존 direct-Core 설치에서 공식 무터미널 도입 및 launcher 자체 교체 경로.

다음 최소 실험은 **지속 저장소 후보의 접근 계약과 파일 수준 commit 가능성**을 확인하는
격리 capability probe 설계다. Agent 셸과 별도 MCP 컨텍스트에서 같은 시험 저장소 식별,
배타 파일 생성·읽기·flush·파일 교체·새 연결 읽기, 사후 digest/종료/증거 보존을 검사한다.
허용된 시험 영역 하나만 쓰며 제품 경로 생성·기존 GIL 변경·승격·권한 확대는 하지 않는다.
이 probe를 실행하기 전에 위치·연산·남길 증거·중단 조건을 구체화한다. 이번 문서는 실행 승인이 아니다.

2026-10-23 추가: 사용자에게 긴 시험 코드나 PowerShell 실행을 전달하는 방식은 중단한다(전달 코드 해시
불일치로 미실행, 원인 미확인). 이후 순서는 구현 → Mac 검수 → Windows CI → 검증된 후보 빌드·prerelease →
Windows에서 짧은 설치 프롬프트로 사용자 인수다. probe는 저장소 안의 자동 시험
(`distribution/runtime-update/`, workflow `u2-capability-probe.yml`)으로 옮겼고, 실행 전 계획 출력·격리
폴더 1곳·연산별 독립 판정·rename 실패 주입·연산 예외 주입·쓰기 불가 중단·같은 실행의 증거 수집을
갖춘다. 일반 runner 시험은 Codex Desktop의 제한 토큰·샌드박스·실제 MCP 연결·지속 저장소 접근을
재현하지 않으며, 성공을 실제 GIL 업데이트 성공이나 원자성·전원 차단 내구성 보장으로 읽지 않는다.
