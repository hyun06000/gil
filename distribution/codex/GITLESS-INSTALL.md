# Git 없는 Mac 설치

대상은 macOS Apple Silicon이다. Windows에 이 절차를 적용하지 않는다.
**현재 버전·archive URL·크기·SHA-256·붙여넣기 요청문은 [macOS 설치 정본](INSTALL.md)을 따른다.**
이 문서에 별도 버전·핀을 복제하지 않는다. Claude Desktop 직접 등록은 [통합 설치 안내](../../INSTALL.md)의
Claude 절차를 따른다. 아래 과거 VM 검수는 현재 버전의 인수가 아니다.

## 현재 설치 순서

1. 설치 Agent가 OS·기존 연결을 확인하고 승인된 고정 archive를 내려받는다. Git·Command Line Tools·
   Node·Rust·Homebrew 설치를 사용자에게 요구하지 않는다.
2. 정본의 크기·SHA-256을 먼저 확인하고 tar 멤버의 절대경로·`..`·심볼릭/하드 링크·특수 파일을 거절한다.
3. 조상에 링크가 없는 새 사용자 전용 폴더(0700)에만 푼다. 일반 파일 0644·실행 파일 0755와 고지를 보존한다.
   검증한 manifest·release.json·Core를 확인하며, 기존 폴더를 덮어쓰지 않는다.
4. Codex는 공식 `plugin marketplace add <검증한 로컬 폴더>`와 `plugin add`로 설치한다. 로컬 출처에 Git
   ref를 사용하지 않는다. Claude는 Desktop 종료·백업 후 같은 Core를 직접 등록한다.
5. **신규 설치·삭제된 훅 복구·업데이트 모두 보호 훅을 등록한다.** 실제 MCP의 검증된 Core로
   `write-guard install --auto --host codex` 또는 `--host claude`를 실행한다. 요청받은 Host만 바꾸고,
   두 Host 모두 같은 Core를 쓸 때만 `--host both`를 사용한다. installer가 비-GIL 설정과 백업을 보존한다.
6. 같은 Core의 `write-guard status --auto`로 선택 Host의 `matches_current_core`와 네 event를 확인한다.
   앱 재시작·새 세션의 정상 훅 검토(Codex `/hooks`) 뒤 사용자가 선택한 버려도 되는 테스트 폴더에서
   자동 등록·Verify 밖 거절·Verify 안 허용을 실제 Host로 확인한다. Monitor 표시도 따로 확인한다.
   실제 보호를 확인하지 못하면 “설정 준비 완료, 실제 보호 확인 대기”로 보고한다.

권한이 필요하면 Host의 정상 승인 절차를 사용하고, 승인 뒤에는 현재 상태를 재조회해 미완료 단계만
이어간다. 전역 훅 삭제·자동 권한 허용·Gatekeeper/quarantine 변경·cache 손편집으로 우회하지 않는다.
훅을 원하지 않는 사용자에게 강제하지 않고 “MCP만 연결, 보호 미설정”으로 보고한다.
기존 보호 훅이 설치를 막으면 [좁은 설정 갱신 절차](INSTALL.md#별도로-설치한-전역-write-guard-갱신)를 따른다.
다운로드·준비 폴더는 실패했다고 자동 삭제하지 않는다. 이전 폴더와 사용자 프로젝트·기록을 보존한다.

## 유지관리자용 준비 스크립트

`prepare-gitless-macos.sh`는 현재 공개 archive의 고정 URL·크기·SHA-256·대상 CPU를 확인하고,
동봉된 `check-archive-members.sh`로 멤버를 검사한 뒤 새 폴더에만 푼다. `--archive` 시험 입력에도 같은
크기·해시 검증을 적용한다. 해시 검증 생략 옵션은 없다. 이 스크립트는 **준비만** 수행하며 Plugin/MCP·
보호 훅을 등록하지 않는다. 성공 뒤에도 위 4~6단계가 남는다. 사용자는 직접 shell 명령을 입력하지 않는다.
검증한 로컬 marketplace 폴더는 설치 출처이므로 유지하고, 업데이트는 새 버전·새 폴더로 수행한다.
정확한 실행 예시와 승인된 배포물은 [설치 정본](INSTALL.md)을 따른다.

## 확인한 범위와 남은 검수

### preview.4 로컬 격리 검수 — 2026-10-16 (개발 Mac, VM 아님)

- 회귀시험 `gitless.test.mjs`: 핀(URL·크기·해시·version)·목록 선검사·금지 명령 부재, 잘못된/같은 크기 위조/
  1바이트 부족 archive 거절, 기존 목적지·링크 archive·링크 조상·상대 경로 거절, 도달 불가 proxy로 끊긴
  다운로드에서 목적지 미생성, 실제 공개 archive로 준비한 루트의 권한·version·`publishable:false` 확인,
  멤버 검사기 단독 시험(공백·한글 이름 수용, 절대 경로·`..`·심볼릭/하드 링크 archive 실제 거절).
- 격리 `CODEX_HOME`(저장된 Git 인증정보·Codex 로그인 없음)에서 공개 URL 실제 다운로드 → 검증 → 해제 →
  공식 `codex plugin marketplace add <로컬 경로>` → `plugin add` → 설치 파일 7개 bytes/권한이 검수본과 동일,
  24 tools·format 4/5 합성 Project 읽기. 손상 archive·중단 다운로드에서 아무것도 등록되지 않음.
  기존 preview.3 등록이 있는 격리 profile에서 `codex plugin list`로 발견 → 승인 전 중단 →
  승인 후 공식 제거·새 로컬 루트 등록·설치로 preview.4 전환. 상세는 유지관리자 로컬 장부에 있다.
- 이 검수는 Git이 설치된 개발 Mac에서 Git을 **사용하지 않고** 수행한 것이며, Command Line Tools가
  없는 Mac에서의 설치 인수를 대신하지 않는다.

### Mac VM 사용자 검수 — 2026-10-16 (preview.4, 부분 성공)

개발 도구가 없는 Mac VM에서 사용자가 당시 붙여넣기 요청(권한 문구 추가 전 판)으로 시험했다.
다운로드·크기/해시 검증·멤버 검사·해제까지는 성공했다. 공식 `codex plugin marketplace add`가
Codex `config.toml` 저장 단계에서 `Operation not permitted`로 실패해 **최초 프롬프트만으로는 설치가
미완**이었다. 사용자가 Agent에게 필요한 권한을 정상 절차로 요청하도록 지시한 뒤 등록·설치가 완료됐고,
새 대화에서 Monitor 표시까지 성공했다. 이 결과를 반영해 요청문 [3]/[3-1]에 권한 요청·미완료 단계
이어가기·중단 조건을 추가했다. 보안 해제나 설정 직접 편집은 사용되지 않았다.

이 VM에서 이어진 실제 작업에서는 별개의 결함(열린 Interview 단계에서 `gil_close` 입력 거절 뒤 절차
이탈)이 관측됐다. 그 조사·보정은 설치 경로와 분리된 별도 변경으로 진행한다. VM의 HTML과 기록은
보존하며, 지나간 절차를 사후 기록하지 않는다.

### 남은 검수 (preview.4)

- 권한 문구가 추가된 **당시 수정판** 붙여넣기 요청만으로 깨끗한 Mac VM에서 설치가 끝나는지(최초 프롬프트
  단독 완료). 위 10-16 결과는 추가 지시가 필요했던 부분 성공이다.
- fullscreen 전환·앱 완전 종료 뒤 복원, 업데이트·제거/재설치의 사용자 실측.
- 업데이트·제거/재설치의 사용자 실측, Windows VM·Intel 실기기, Apple 서명·공증.

### Mac VM 사용자 인수 — 2026-10-07 (preview.3)

개발 도구가 없는 UTM Mac-test에서 사용자가 preview.3 Git 없는 경로로 설치 성공을 보고했다.
새 대화에서 계산기를 만들고 수정하는 동안 fullscreen의 가로 그래프와 채팅을 함께 사용했고,
노드 갱신·요약/상세·되돌아감과 이후 분기의 정상 동작을 확인했다. 당시 깨끗한 VM에서 Git 경로는
Apple Command Line Tools 부재로 실패했고, 그 결함이 이 Git 없는 경로를 만든 이유다.
이는 사용자 관측에 근거한 해당 VM의 preview.3 설치·사용 인수이며 preview.4로 승계되지 않는다.
앱을 완전히 종료하지 않았으므로 완전 종료 뒤 복원은 미검증이었다.

## 설치 안내 결함 기록

빈 Codex 대화에 로컬 실행 도구가 없을 수 있고, Git-backed 마켓플레이스는 Git 없는 Mac에서
복제하지 못한다. GIL runtime이 개발 도구를 요구하지 않는다는 사실을 설치 경로 전체의 무의존성으로
일반화하면 안 된다. 설치 완료 기준에는 이 최초 다운로드/등록 단계와 실제 화면을 함께 포함한다.
