# Windows release readiness

현재 판정: **서명된 일반 공개판은 NO-GO. 친구·테스터용 unsigned x64 preview는 제한 인수 완료.**

## 출시 필수 게이트

- [ ] x64 사용자 단위 installer와 동일한 방식의 uninstall/repair
- [ ] Authenticode 서명 및 clean PC에서 publisher 표시 확인
- [ ] Companion, 고정 진입점, Core, Skill/manifest의 한 release cohort
- [ ] Host cache 밖 GIL 소유 설치 경로와 정확한 ACL/소유권 검증
- [ ] 사용자의 버튼 클릭 전에는 다운로드·설치를 시작하지 않는 업데이트 UI
- [ ] immutable version 준비, digest/서명/호환성 검증, 원자적 selection commit
- [ ] 기존 연결 유지, 새 연결 identity 확인, 실패/중단 뒤 기존 선택 보존
- [ ] Job Object 또는 동등한 자식 수명 관리와 고아 Core 부재 확인
- [ ] 디스크 고갈·네트워크 중단·서명 불일치·동시 실행·재부팅 시험
- [ ] standard-user clean Windows x64에서 설치→Codex 등록→Monitor→업데이트→제거 인수
- [ ] ARM64 지원 여부를 별도 결정하고, 지원 시 x64와 독립적으로 같은 인수를 통과
- [ ] Project와 `.gil`이 설치·업데이트·제거 전후 보존됨을 확인
- [ ] support/rollback 정책과 서명된 고정 다운로드 URL 게시

## 현재 확보한 하위 증거

- native Core의 Windows x64/ARM64 CI 빌드 및 MCP smoke
- 서명되지 않은 Plugin review archive 생성과 파일 무결성 검사
- 합성 Core를 이용한 불변 버전 선택 및 새 연결 전환 시험
- runtime 상태 판정과 capability probe의 단위/CI 시험
- current-user NSIS 구성과 사용자가 누르는 서명 updater
- 고정 런처의 receipt/digest 검사, 직전 Core 폴백 및 Job Object 구현
- 버전별 Core 설치와 선택 기록 전환/복원 설치 훅
- 사용자가 확인한 뒤 `codex mcp add gil -- <launcher> mcp --serve`를 호출하는 연결 버튼
- 실제 서명 비밀이 있을 때만 실행되는 Windows 제품 후보 CI와 설치 후 런처/Codex CLI smoke
- 정확히 소유한 등록만 해제하는 버튼과 Companion 제거 뒤 runtime/Project 보존 계약
- Windows x64 실기기에서 unsigned `0.1.1-owner.3` 설치, Companion 실행, Codex 연결·이전 연결
  교체, 파일 생성과 사용자 버튼 업데이트 완료(2026-10-27 사용자 보고)

이 증거는 Authenticode 서명 installer, SmartScreen publisher, clean standard-user PC,
제거 또는 모든 실패 복구의 종단 증거가 아니다. 특히 testbed 전용 코드와
`publishable: false` archive는 출시 자산으로 승격하지 않는다.

## 출시 결정 규칙

모든 필수 게이트에 재현 가능한 증거가 있을 때만 별도 release record에서 GO를 선언한다.
부분 통과를 합쳐 종단 성공으로 만들지 않는다. OS 또는 Host가 차단하면 우회하지 않고 NO-GO를
유지한다. 날짜가 붙은 기존 문서는 당시 실험 기록이며 이 체크리스트의 현재 상태를 바꾸지 않는다.
