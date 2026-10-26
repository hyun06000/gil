# Windows 설치 안내

> **제품 설치 코드는 있으나, 서명·실기기 인수 전이라 공개 설치본은 없다.**

이 저장소가 만드는 Windows `tar.gz`는 개발·검토용 후보이며 installer가 아니다. 과거
`windows-preview-*` archive를 수동으로 풀어 local marketplace로 등록하는 절차는 제품 설치
경로에서 폐기한다. 그 절차는 Host cache 안의 실행 파일을 직접 사용하므로 실행 중 교체와 제거를
안전하게 해결하지 못하고, 서명된 publisher·복구·제거·Companion 업데이트도 제공하지 않는다.

사용자와 설치를 돕는 Agent는 다음을 하지 않는다.

- archive를 일반 사용자용 installer라고 부르기
- Plugin cache, registry 또는 Codex 설정을 손으로 편집·삭제하기
- Defender, SmartScreen, 실행 정책 또는 ACL을 완화하기
- 관리자 권한으로 실패를 우회하기
- CI 성공을 실제 Codex 설치나 화면 성공으로 보고하기

현재 구현된 제품 경로는 current-user NSIS가 `%LOCALAPPDATA%\GIL\runtime`에 고정 런처와
버전별 Core를 설치하고, Companion의 **Codex 연결** 버튼이 사용자의 재확인 뒤 공식 CLI로
등록하는 방식이다. 설정 파일을 직접 편집하거나 기존 등록을 자동으로 덮어쓰지 않는다.
제거할 때는 Companion의 **Codex 연결 해제**를 먼저 누르고 열려 있는 GIL 대화를 닫은 뒤 Windows
앱 제거를 사용한다. 첫 판 uninstaller는 안전을 위해 runtime과 Project의 `.gil`을 보존한다.

서명·실기기 인수가 끝나면 이 문서는 다음 항목을 포함한 짧은 안내로 교체한다.

1. 공식 HTTPS 다운로드와 서명된 x64 installer(ARM64는 별도 검수 뒤 추가)
2. publisher, version, size와 digest 확인 방법
3. 사용자 단위 설치/복구/제거 동작
4. Codex의 공식 등록 절차
5. Companion의 **업데이트 확인** 및 사용자가 누르는 **업데이트** 버튼
6. 새 연결의 실제 Core identity로 적용을 확인하는 방법

구현 방향과 남은 게이트는 [Windows 배포 상태](README.md)와
[Companion 설치·업데이트 계약](../../spec/GIL_Companion_Install_Update_v0.1.md)을 따른다.
과거 공개 후보의 bytes와 검수 결과는 `PUBLICATION-*` 문서에 역사적 증거로만 남긴다.
