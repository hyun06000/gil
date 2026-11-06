# Host 훅 제거와 GIL 프로젝트 안내

GIL은 **전역·프로젝트 로컬·Plugin·Skill 훅을 설치하지 않는다.** 다른 프로젝트나 일반 세션의
Bash·exec_command·Write·Edit·사용자 입력·턴 종료에 개입하지 않는다. cwd가 다른 프로젝트라는
문자열 검사로 전역 훅을 면제하는 방식이 아니라, Host에 GIL 훅 자체를 등록하지 않는다.

## 범위와 이유

Codex와 Claude는 프로젝트 로컬 훅을 지원한다. 그러나 Host가 연 프로젝트와 MCP가 선택한 GIL
`project_root`는 다를 수 있다. 로컬 설정 지원만으로 현재 GIL 연결 방식의 다른 프로젝트 비간섭까지
검증했다고 주장하지 않는다. 사용자 결정에 따라 이번 제품에서는 로컬 훅도 제공하지 않는다.

- [Codex hook locations](https://learn.chatgpt.com/docs/hooks): 사용자·프로젝트·Plugin 설정을 병합하고 프로젝트 신뢰를 별도로 검토한다.
- [Claude hook locations](https://code.claude.com/docs/en/hooks#hook-locations): 사용자 전역·프로젝트·Plugin·Skill마다 범위가 다르다.

## 훅 없는 진행 안내

GIL을 요청한 선택 프로젝트에서만 공통 프롬프트와 `gil_start`·`gil_context`·`gil_open`·`gil_close`의
응답이 현재 단계에 맞춰 다음 호출 또는 대기를 안내한다. 시작은 Monitor와 Question, 사용자 답변·승인
대기는 실제 입력, 파일 작업은 성공적으로 열린 Verify, Verify 종료 후는 저장 증거 해석과 파일 변경 중지를 안내한다.
다른 프로젝트의 기록이 있다는 이유로 일반 개발·설치·웹 조사에 GIL을 요구하지 않는다.

프로젝트 파일 생성·수정·삭제·이동·외부 결과 가져오기는 열린 Verify에서 수행하도록 **안내**한다.
프로젝트 내부 저장을 기본으로 하고 필요한 외부 생성은 이유·경로·추적되지 않음을 먼저 설명한다.
이것은 OS 파일 차단이 아니다. 프롬프트가 모든 모델의 준수나 다른 세션의 쓰기 방지를 보장하지 않는다.
Core 자체의 전이·승인·Report·증거·dirty 검사는 유지한다. 승인을 만들어 내거나 기존 기록을 소급 변경하지 않는다.

## 기존 설치의 정확한 훅 제거

새 Core로 MCP를 갱신한 뒤 사용하는 Host만 정리한다. 두 Host 모두 요청받았을 때만 `both`를 쓴다.

```sh
"$GIL_CORE" write-guard uninstall --auto --host codex
"$GIL_CORE" write-guard status --auto
# Claude는 --host claude; 두 Host 모두 명시적으로 요청받았으면 --host both.
```

- `~/.codex/hooks.json`·`~/.claude/settings.json`에서 과거 GIL이 생성한 정확한 quoted command만 제거한다.
  구형 `gil`/`gil-write-guard`, `hook --auto [--host …]`, `hook --root …` 형식을 인식한다.
- 같은 행의 다른 훅·matcher·권한·sandbox·Plugin/MCP 항목은 보존한다. 변경 전 전체 원본을
  `~/.gil-host-guard`의 private 백업에 남긴다. 없는 설정은 만들지 않고 재실행은 변경하지 않는다.
- shell 접두사·접미사를 붙인 사용자 명령은 자동 삭제하지 않는다. `custom_review_required`이면
  정확한 명령을 검토한 뒤 GIL 부분만 정리한다. JSON 손상·symlink·경합은 변경 전에 중단한다.
- 예전 registry 파일·session/wait·프로젝트 기록·승인·Snapshot은 읽거나 갱신하지 않는다.
  registry 손상·잠금 때문에 일반 도구나 제거 작업이 막히지 않는다.
- `not_installed`는 검사한 Host JSON에 인식 가능한 GIL 명령이 없다는 뜻이다. 이미 실행 중인
  Host의 메모리나 임의 사용자 스크립트까지 검사했다는 의미가 아니다. 앱 재시작·새 세션에서 확인한다.
- `install --auto`는 더 이상 훅을 만들지 않고 제거 절차를 안내하며 거절한다. `config`는 빈 hooks를
  출력하고, 새 Core의 모든 구형 `hook` 호출은 HOME·cwd·registry·프로젝트 상태와 무관하게 빈 결과로 끝난다.
  옛 Core를 계속 가리키는 설정은 반드시 제거한다. 빈 설정을 덮어써 다른 훅까지 지우지 않는다.

신규 설치도 같은 정리 명령으로 오래된 설정 여부를 확인할 수 있다. 전역 보안 해제·자동 승인·신뢰 우회는 하지 않는다.
프로젝트 안에 `.codex`·`.claude`를 만들거나 새 폴더마다 사용자에게 훅 설정을 요구하지 않는다.

## 개발자 회귀 도구와 검증 한계

명시적으로 실행하는 `gil write-guard inspect --root …`와 `run --root … -- '명령'`은 개발용 정책·sandbox
회귀 도구로 남는다. 제품 Agent는 일반 작업에 이를 요구하지 않으며 Host 훅으로 등록하지 않는다.
기존 절대경로·cd·symlink·Verify 경계·손상·잠금·중첩 sandbox 검증은 이 명시적 도구에서 유지한다.
자동 `run --auto`는 사용하지 않는다. 실제 Host 외부 도구를 차단한다고 주장하지 않는다.

자동검증은 임시 HOME·합성 프로젝트에서 구형 훅의 무동작, 일반 셸 입력을 바꾸지 않음, 신규 설치의 무등록,
정확한 제거·백업·다른 설정 보존, GIL 응답의 단계별 안내와 Core 승인·증거 규칙을 확인한다.
실제 Codex/Claude의 재시작·설정 로딩·일반 프로젝트 비간섭과 모델의 안내 준수는 별도 필드테스트다.
