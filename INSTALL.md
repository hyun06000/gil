# GIL 설치 지침 (AI 에이전트용)

목표: 이 PC의 Claude Desktop(Claude Code 탭)과 Codex에 GIL MCP 서버 `gil`을 등록한다.
대상: Windows 10/11 x64. 관리자 권한은 필요 없다. 보안 설정·실행 정책·registry는 바꾸지 않는다.
실행 전: 아래 단계를 사용자에게 요약해 보여주고 승인을 받는다.

사람이 직접 설치하려면 PowerShell에 한 줄을 붙여 넣는다. 아래 단계와 같은 일을 한다.

```powershell
irm https://raw.githubusercontent.com/hyun06000/gil/main/install.ps1 | iex
```

## 단계 (PowerShell)

1. 폴더를 만든다.
   ```powershell
   New-Item -ItemType Directory -Force "$env:LOCALAPPDATA\Programs\gil"
   ```

2. 실행 파일과 체크섬을 내려받는다. 실행 중인 `gil.exe`를 덮어쓰지 않도록 임시 이름으로 받는다.
   ```powershell
   Invoke-WebRequest "https://github.com/hyun06000/gil/releases/latest/download/gil-x86_64-pc-windows-msvc.exe" -OutFile "$env:LOCALAPPDATA\Programs\gil\gil-new.exe"
   Invoke-WebRequest "https://github.com/hyun06000/gil/releases/latest/download/checksums.txt" -OutFile "$env:LOCALAPPDATA\Programs\gil\checksums.txt"
   ```

3. 체크섬을 검증한다. `checksums.txt`의 `gil-x86_64-pc-windows-msvc.exe` 줄과
   `(Get-FileHash "$env:LOCALAPPDATA\Programs\gil\gil-new.exe" -Algorithm SHA256).Hash`를 비교한다.
   다르면 `gil-new.exe`를 삭제하고 중단한다.

4. 등록한다. 실행 파일이 자신을 `gil.exe`로 옮기고(이전 판은 이름만 바꿔 보존) 두 Host 설정에 `gil` 항목을 쓴다.
   ```powershell
   & "$env:LOCALAPPDATA\Programs\gil\gil-new.exe" install
   Remove-Item "$env:LOCALAPPDATA\Programs\gil\gil-new.exe"
   ```

5. 점검한다.
   ```powershell
   & "$env:LOCALAPPDATA\Programs\gil\gil.exe" doctor
   ```

## 완료 조건

`doctor`의 종료 코드가 0이고, 이 PC에 설치된 Host마다 `OK`가 출력된다. 설치되지 않은 Host는 `NOT_INSTALLED`이며 오류가 아니다.

```text
binary: OK C:\Users\name\AppData\Local\Programs\gil\gil.exe (v0.3.0)
claude-desktop: OK
codex: OK
```

`claude-desktop: PENDING …`은 Claude Desktop이 실행 중이어서 도우미가 종료를 기다리는 상태다.
사용자가 Claude Desktop을 트레이에서 완전히 종료하면 도우미가 등록을 마치고 `install-log.txt`에 결과를 남긴다.
그 뒤 `doctor`를 다시 실행하면 `OK`가 된다. Claude Desktop 안에서 실행 중인 에이전트는 자기 앱을 종료할 수 없으므로 사용자에게 종료를 요청한다.

## 마지막 안내

사용자에게 Claude Desktop(트레이 아이콘에서 완전 종료)과 Codex를 다시 시작하라고 알린다. 재시작 전에는 도구가 보이지 않는다.
설치가 끝나면 새 대화에서 빈 테스트 폴더를 선택하고 “지금 폴더에서 GIL 프로젝트를 시작해 줘”라고 말해 Monitor가 보이는지 확인한다.

## 실패하면

출력 전문을 사용자에게 보여준다. 설정 파일을 직접 수정하지 않는다. `STALE`이면 `install`을 다시 실행한다.
`gil` 항목이 GIL이 아닌 다른 명령을 가리키면 `install`은 그 항목을 건드리지 않고 보고한다. 사용자가 교체를 원할 때만 `install --replace`를 쓴다.

## 업데이트

같은 단계를 다시 실행한다. 경로가 고정이라 재등록은 필요 없다. 열려 있는 대화는 이전 판을 계속 쓰고, 새 연결부터 새 판을 쓴다.

## 제거

```powershell
& "$env:LOCALAPPDATA\Programs\gil\gil.exe" uninstall
```

두 설정에서 `gil` 항목만 제거한다. 실행 파일까지 지우려면 `uninstall --remove-binary` 뒤 폴더를 삭제한다.
Project 폴더와 `.gil` 기록은 지우지 않는다.

## macOS

macOS Apple Silicon은 아직 Plugin/archive 채널이다. [상세 통합 안내](distribution/INSTALL-GUIDE.md)의 macOS 절을 따른다.
