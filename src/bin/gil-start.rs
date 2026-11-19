//! MSIX Start-menu entry only. The execution alias still targets the unchanged gil.exe.
//! No project discovery, configuration writes, child processes or MCP transport here.
use std::io::{self, BufRead, IsTerminal, Write};
use std::process::ExitCode;

const INTRO: &str = "GIL - Getting started / 시작 안내\n\n\
GIL works with your AI agent. The Monitor appears inside the connected app.\n\
GIL은 AI 에이전트와 함께 사용하는 도구입니다. Monitor는 연결한 앱 안에 표시됩니다.\n\n\
This screen only explains how to connect GIL; it does not change settings.\n\
이 화면은 연결 방법을 안내하며 설정을 변경하지 않습니다.\n";

const ENGLISH: &str = "\nEnglish - Connect and use GIL\n\n\
1. Install and open a supported app: Claude Desktop or Codex.\n\
2. Quit Claude Desktop completely, including its tray icon, before connecting.\n\
3. Open PowerShell or Command Prompt and run: gil install --no-wait\n\
4. Check the registration: gil doctor (details and helper log path: gil doctor --verbose)\n\
   NOT_INSTALLED means the app was not detected; it does not mean connected.\n\
5. Restart the connected app, then ask its agent to use GIL in your project.\n\
   Check that GIL tools and the Monitor actually appear in the app.\n\n\
Terminal help: gil --help\n\
If gil is not found, check Windows Settings > Apps > App execution aliases,\n\
then reopen your terminal. The gil.exe alias must be enabled for GIL.\n\
For updates, use Microsoft Store. To remove GIL, use Windows Settings > Apps.\n\
To disconnect the apps first, quit them and run: gil uninstall\n";

const KOREAN: &str = "\n한국어 - GIL 연결과 사용\n\n\
1. 지원 앱인 Claude Desktop 또는 Codex를 설치하고 한 번 실행하세요.\n\
2. 연결하기 전에 Claude Desktop을 트레이 아이콘에서도 완전히 종료하세요.\n\
3. PowerShell 또는 명령 프롬프트를 열고 실행하세요: gil install --no-wait\n\
4. 등록 상태를 확인하세요: gil doctor (진단 내용과 도우미 로그 위치: gil doctor --verbose)\n\
   NOT_INSTALLED는 앱을 찾지 못했다는 뜻이며 연결 완료가 아닙니다.\n\
5. 연결한 앱을 다시 시작하고 에이전트에게 프로젝트에서 GIL을 사용해 달라고 하세요.\n\
   앱 안에 GIL 도구와 Monitor가 실제로 나타나는지 확인하세요.\n\n\
터미널 도움말: gil --help\n\
gil 명령을 찾을 수 없으면 Windows 설정 > 앱 > 앱 실행 별칭에서\n\
GIL의 gil.exe 별칭이 켜져 있는지 확인하고 터미널을 다시 여세요.\n\
업데이트는 Microsoft Store에서, 앱 제거는 Windows 설정 > 앱에서 진행하세요.\n\
먼저 앱 연결을 해제하려면 연결한 앱을 종료하고 실행하세요: gil uninstall\n";

const MENU: &str = "\n[1] English  [2] 한국어  [0] Exit / 종료\n> ";

fn guide(input: &mut impl BufRead, output: &mut impl Write, interactive: bool) -> io::Result<()> {
    writeln!(output, "{INTRO}")?;
    // Redirected invocations never wait for input. This also permits bounded CI checks of
    // both translations without pretending a pipe is a visible Windows console.
    if !interactive {
        write!(output, "{ENGLISH}\n{KOREAN}")?;
        return output.flush();
    }
    loop {
        write!(output, "{MENU}")?;
        output.flush()?;
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 { return Ok(()); }
        match line.trim() {
            "0" => return Ok(()),
            "1" => write!(output, "{ENGLISH}")?,
            "2" => write!(output, "{KOREAN}")?,
            _ => writeln!(output, "Choose 1, 2 or 0. / 1, 2 또는 0을 입력하세요.")?,
        }
    }
}

fn main() -> ExitCode {
    if std::env::args_os().len() != 1 {
        eprintln!("gil-start takes no arguments. Use gil for CLI commands.\n\
            gil-start는 인자를 받지 않습니다. CLI 명령은 gil을 사용하세요.");
        return ExitCode::from(2);
    }
    let stdin = io::stdin();
    let stdout = io::stdout();
    let interactive = stdin.is_terminal() && stdout.is_terminal();
    match guide(&mut stdin.lock(), &mut stdout.lock(), interactive) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("GIL guide could not continue / GIL 안내를 계속할 수 없습니다: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_languages_are_selectable_and_zero_exits_without_consuming_more_input() {
        let mut input = io::Cursor::new(b"1\n2\n0\n1\n");
        let mut output = Vec::new();
        guide(&mut input, &mut output, true).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains(ENGLISH));
        assert!(output.contains(KOREAN));
        assert_eq!(output.matches(MENU).count(), 3);
        assert_eq!(input.position(), 6);
    }

    #[test]
    fn invalid_input_and_eof_are_bounded() {
        let mut output = Vec::new();
        guide(&mut io::Cursor::new(b"\nwrong\n"), &mut output, true).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert_eq!(output.matches("Choose 1, 2 or 0.").count(), 2);
        assert_eq!(output.matches(MENU).count(), 3);
    }

    #[test]
    fn redirected_guide_never_reads_input() {
        struct NoRead;
        impl io::Read for NoRead {
            fn read(&mut self, _: &mut [u8]) -> io::Result<usize> { panic!("must not read") }
        }
        impl BufRead for NoRead {
            fn fill_buf(&mut self) -> io::Result<&[u8]> { panic!("must not read") }
            fn consume(&mut self, _: usize) { panic!("must not read") }
        }
        let mut output = Vec::new();
        guide(&mut NoRead, &mut output, false).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains(ENGLISH) && output.contains(KOREAN));
        assert!(!output.contains(MENU));
    }

    #[test]
    fn output_errors_are_reported() {
        struct Broken;
        impl Write for Broken {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
            fn flush(&mut self) -> io::Result<()> { Ok(()) }
        }
        assert_eq!(guide(&mut io::empty(), &mut Broken, true).unwrap_err().kind(), io::ErrorKind::BrokenPipe);
    }
}
