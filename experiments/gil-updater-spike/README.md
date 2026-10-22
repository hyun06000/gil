# 삭제 없는 Core 전환 — 격리 설계 실험

2026-10-23, macOS. 제품 코드·Codex 설정/cache·실제 GIL 설치본은 변경하지 않는다.
실험은 Node 표준 라이브러리와 가짜 Core만 사용한다. 실제 GIL 또는 네트워크 다운로드는 없다.

## 실행

```sh
node --test launcher.test.mjs
```

시험별로 OS 임시 폴더에 `gil-launcher-spike-*`를 만든다. 진단용으로 남겨 두며
실제 프로젝트를 탐색하지 않는다. 합성 프로젝트 대조 파일은 각 시험 종료 후 그대로인지 검사한다.

## 확인한 구조

- 새 Core와 manifest를 별도 staging에 기록한 뒤 버전 폴더로 확정한다.
- 이미 확정된 버전 폴더는 덮어쓰지 않는다. 기존 버전 삭제 기능은 없다.
- 명시 승인, 정확한 Skill hash·protocol·storage 계약, Core·manifest hash,
  프로젝트 접근 없는 probe를 통과해야 `active.json`을 교체한다.
- 한 번 실행된 자식 프로세스는 시작 때 선택한 버전을 계속 사용한다.
- 다음 launch는 새 선택을 사용한다. 실행 중 자식의 핫스왑은 하지 않는다.
- probe 실패는 전환 전 실패이므로 기존 선택이 남는다. 활성화 이후 실패 시
  무조건 이전 Core로 복귀하지 않는다. 프로젝트 기록을 되돌리는 기능도 없다.
- update.lock은 배타 생성하며, 충돌 시 거절한다. 잠금을 강제로 회수하지 않는다.

## 테스트 결과

Mac 실행: 10/10 통과. 항목:

1. v1 실행 중 v2 준비·선택, v1 계속 응답, 신규 실행은 v2, v1 바이트 불변.
2. 다운로드에 해당하는 입력 바이트 hash 불일치 거절, v1 유지.
3. Skill 계약 불일치 거절.
4. 승인 없음 거절.
5. probe 실패 시 v1 실행 가능.
6. 선택 기록 직전 실패 주입 시 v1 유지.
7. 같은 버전 덮어쓰기 거절.
8. 활성 Core 변조 시 실행 거절, 무조건 rollback 없음.
9. 저장 계약 변경 거절, 이관 없음.
10. 동시 활성화 요청의 잠금 충돌 거절.

## 이 실험이 증명하지 않는 것

- Windows native executable의 파일 점유·rename·잠금·내구성.
- Codex 제한 토큰에서 GIL 전용 저장소를 읽거나 실행할 수 있는지.
- 새 대화를 열면 반드시 새로운 MCP 프로세스가 생긴다는 보장.
- 실제 MCP stdio 전달, 종료/취소 전달, 프로세스 수명 관리.
- Core·Skill의 의미적 호환성: 여기서는 고정 계약의 문자열/숫자 일치만 검사한다.
- 실제 프로젝트에서 신·구 Core 동시 사용의 안전성.
- 신뢰할 배포자 인증: hash는 시험 입력으로 고정했으며 서명·키 관리·배포 체인은 구현하지 않았다.
- Windows reparse/ACL/TOCTOU 방어. 이 파일 경로 코드는 제품용 보안 구현이 아니다.
- 전원 차단 내구성: fsync/디렉터리 flush는 구현하지 않았다.
- 비정상 종료 후 stale lock 해제: 자동 복구하지 않는다.
- 사용자 승인 증거의 인증: `approved`는 시험 호출 인자일 뿐이다.
- 현재 4584dfe4 설치본에서 실행기 구조로 넘어가는 최초 전환.

## 다음 판단

삭제와 버전 선택을 분리하는 구조는 Mac 합성 실험에서 성립했다.
이 코드를 그대로 GIL에 넣거나 설치 절차로 쓰면 안 된다.
다음 단계는 별도 승인 아래 Windows CI에서 native 가짜 Core 두 개로 같은 수명 시험을
수행하는 것이다. 그 뒤에만 실제 Codex 제한 실행 환경·Skill 계약·최초 전환을 검토한다.
기존 PR, 게시물, 설치본을 바꿀 필요는 없다.

## 네이티브 후속 준비 (2026-10-23)

`native-core.rs`는 파일·레지스트리·네트워크 접근 없는 가짜 Core다.
`node run-native.mjs`는 rustc로 v1/v2/시작 실패 fixture를 컴파일한 뒤 네이티브
프로세스로 실행한다. Node 가짜 Core 시험과 같은 실행기 경로를 사용한다.

- Mac ARM64: 기존 10개 + 네이티브 6개 모두 통과.
- 실제 관측: 구 프로세스가 활성 선택 변경 후 계속 응답, 신 프로세스는 v2,
  구 실행 파일 바이트 불변. 시작 실패·hash·Skill·승인·전환 직전 실패도 검증.
- 파일명이 `.exe`여도 Mac에서 만든 것은 Mach-O이다. Windows 실행 증거가 아니다.
- Windows에서는 native Node architecture와 MSVC rustc host를 검사한 뒤 빌드한다.
- `.github/workflows/gil-updater-spike.yml`은 x64/ARM64 runner용이다.
  실험 브랜치 push 또는 수동 dispatch로만 실행한다.
  GIL 소스 빌드·설치·프로젝트 접근·자산 게시는 없다.
- 로컬에 Windows 실행 환경과 PowerShell이 없어 Windows 결과는 아직 없다.
- 실험 파일·workflow의 별도 branch commit/push와 Windows CI 실행은 승인된 범위다.
  제품 브랜치 수정·PR 생성·병합·배포·사용자 PC 설치는 이 실험 범위에 포함하지 않는다.
- 실행기 선택 파일의 Windows rename 동작도 이 시험 대상이다. 실패하면 성공으로 간주하지 않는다.
