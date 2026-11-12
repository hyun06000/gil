# GIL — Windows x64 설치 (관리자 권한 불필요, 보안 설정 변경 없음)
#
#   irm https://raw.githubusercontent.com/hyun06000/gil/main/install.ps1 | iex
#
# 하는 일: 최신 gil.exe를 받아 SHA-256을 대조하고 %LOCALAPPDATA%\Programs\gil\gil.exe 에 놓은 뒤
# `gil install`로 Claude Desktop·Codex 설정에 등록하고 `gil doctor`로 확인한다. INSTALL.md 와 같은 순서다.
# 실행 중인 gil.exe 는 덮어쓰지 않고 이름을 바꿔 둔다(열린 대화는 그대로, 새 연결부터 새 판).

$ErrorActionPreference = 'Stop'
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$Repo = 'hyun06000/gil'
$Asset = 'gil-x86_64-pc-windows-msvc.exe'
$Base = "https://github.com/$Repo/releases/latest/download"
$Dir = Join-Path $env:LOCALAPPDATA 'Programs\gil'
$Exe = Join-Path $Dir 'gil.exe'
$New = Join-Path $Dir 'gil-new.exe'
$Sums = Join-Path $Dir 'checksums.txt'

if (-not [Environment]::Is64BitOperatingSystem) { throw '64비트 Windows가 필요합니다.' }
if ($env:PROCESSOR_ARCHITECTURE -eq 'ARM64') {
    Write-Host 'ARM64 Windows: x64 실행 파일을 에뮬레이션으로 실행합니다(검증되지 않음).'
}

New-Item -ItemType Directory -Force -Path $Dir | Out-Null

Write-Host "1/5 다운로드 $Base/$Asset"
Invoke-WebRequest -Uri "$Base/$Asset" -OutFile $New -UseBasicParsing
Invoke-WebRequest -Uri "$Base/checksums.txt" -OutFile $Sums -UseBasicParsing

Write-Host '2/5 SHA-256 대조'
$expected = (Get-Content $Sums | Where-Object { $_ -match "\s$([regex]::Escape($Asset))$" } | Select-Object -First 1) -replace '\s.*$', ''
if (-not $expected) { Remove-Item $New -Force; throw "checksums.txt 에 $Asset 항목이 없습니다." }
$actual = (Get-FileHash -LiteralPath $New -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actual -ne $expected.ToLowerInvariant()) {
    Remove-Item $New -Force
    throw "SHA-256 불일치: 받은 파일 $actual, 기대값 $expected. 설치를 중단합니다."
}

Write-Host '3/5 배치 및 Host 등록 (gil install)'
& $New install
$installExit = $LASTEXITCODE
Remove-Item $New -Force -ErrorAction SilentlyContinue
if ($installExit -ne 0) { throw "gil install 이 실패했습니다 (종료 코드 $installExit). 위 출력을 확인하세요." }

Write-Host '4/5 확인 (gil doctor)'
& $Exe doctor
$doctorExit = $LASTEXITCODE

Write-Host '5/5 완료 안내'
Write-Host "설치 위치: $Exe"
Write-Host 'Claude Desktop 은 트레이 아이콘에서 완전히 종료한 뒤 다시 열고, Codex 는 다시 시작하세요. 재시작 전에는 도구가 보이지 않습니다.'
Write-Host "Claude Desktop 이 실행 중이었다면 도우미가 종료를 기다려 등록을 마칩니다. 결과: $(Join-Path $Dir 'install-log.txt')"
if ($doctorExit -ne 0) {
    Write-Host "gil doctor 가 문제를 보고했습니다 (종료 코드 $doctorExit). PENDING 은 Claude Desktop 종료 뒤 자동으로 해결됩니다."
}
