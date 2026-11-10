//! **사람에게 보이는 문장** — 다음 행동을 말하고, 구현 세부를 말하지 않는다.
//!
//! `plugins/gil-companion-prototype/say.mjs` 를 글자 그대로 옮긴 것이다. 이전이 끝날 때까지
//! JS 가 정본이고, `tests/companion_say.rs` 가 이 파일의 출력을 JS 의 출력과 byte 로 견준다.
//!
//! Distribution Model §7 은 port·localhost URL·binary path·package format·MCP transport 를
//! 사용자에게 보여 주지 않는다고 정한다. 그래서 문장은 **여기서만** 만든다. 바깥에서 받은 글을
//! 그대로 잇지 않는다 — OS 의 오류 문구에는 경로가 들어 있다.

use std::sync::OnceLock;

use crate::capability::{CompanionState, Outcome};

const APP_NAME: &str = "GIL Monitor";

/// Monitor 가 없어도 GIL 은 계속 쓸 수 있다 — 매번 같은 말로 붙인다.
fn still_usable() -> &'static str { crate::locale::text("companion.still_usable") }

/// 실제로 밟을 수 있는 다음 행동만 적는다. 없는 길을 안내하지 않는다.
fn next_in_text() -> &'static str { crate::locale::text("companion.next_in_text") }

pub fn outcome(outcome: Outcome) -> String {
    match outcome {
        Outcome::OpenedPersistentHost => crate::tr!("companion.opened_host", app = APP_NAME),
        Outcome::FocusedExistingCompanion => crate::tr!("companion.focused", app = APP_NAME),
        Outcome::StartedAndOpenedCompanion => crate::tr!("companion.started", app = APP_NAME),
        Outcome::NeedsCompanionInstall => crate::tr!("companion.needs_install", app = APP_NAME, still = still_usable()),
        Outcome::NeedsCompanionUpdate => crate::tr!("companion.needs_update", app = APP_NAME, still = still_usable()),
        Outcome::MonitorUnavailable => crate::tr!("companion.unavailable", still = still_usable(), next = next_in_text()),
    }
}

/// 네 상태를 사람 말로. 판 번호는 사람이 업데이트를 판단할 근거라서 남긴다.
/// `app_version` 이 없으면 JS 와 같이 "알 수 없음" 이다.
pub fn state(state: CompanionState, app_version: Option<&str>) -> String {
    let version = app_version.unwrap_or(crate::locale::text("companion.unknown_version"));
    match state {
        CompanionState::Missing => crate::tr!("companion.state.missing", app = APP_NAME),
        CompanionState::Stopped => crate::tr!("companion.state.stopped", app = APP_NAME, version = version),
        CompanionState::Outdated => crate::tr!("companion.state.outdated", app = APP_NAME),
        CompanionState::Ready => crate::tr!("companion.state.ready", app = APP_NAME, version = version),
    }
}

/// 응답에 섞이면 안 되는 것. `say.mjs` 의 `LEAKS` 와 같은 일곱 무늬다.
fn leaks() -> &'static [(regex::Regex, &'static str)] {
    static LEAKS: OnceLock<Vec<(regex::Regex, &'static str)>> = OnceLock::new();
    LEAKS.get_or_init(|| {
        [
            (r#"(^|[\s"'`(])[~/][^\s"'`)]{2,}"#, "경로"),
            (r"(?i)\.(app|sock|sh|exe|dmg|pkg)\b", "파일 이름"),
            (r"(?i)\b(?:localhost|127\.0\.0\.1|0\.0\.0\.0)\b", "loopback 주소"),
            (r"(?i)\b(?:port|포트)\s*[:=]?\s*\d{2,5}\b", "port"),
            (r":\d{2,5}\b", "port"),
            (r"(?i)\bPID\b|\bpid\s*[:=]?\s*\d+", "PID"),
            (r"(?i)\bsocket\b|\bsock\b", "socket"),
        ]
        .into_iter()
        .map(|(pattern, what)| (regex::Regex::new(pattern).expect("leak pattern"), what))
        .collect()
    })
}

/// 응답에 경로·PID·socket·port 가 섞이지 않았는지 문 앞에서 한 번 더 본다.
///
/// 문장을 여기서만 만들어도, 누군가 나중에 바깥 글을 이어 붙일 수 있다. 그때 조용히
/// 새어 나가지 않도록 **내보내기 직전에** 막는다.
pub fn assert_no_leak(text: &str) -> Result<&str, String> {
    for (pattern, what) in leaks() {
        if pattern.is_match(text) {
            return Err(format!("사용자 응답에 {what}가 섞였다: {text}"));
        }
    }
    Ok(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn js12_nothing_that_is_said_leaks_a_path_pid_socket_or_port() {
        for one in Outcome::ALL {
            assert_no_leak(&outcome(one)).unwrap_or_else(|why| panic!("{one:?} 가 샌다: {why}"));
        }
        for one in [
            CompanionState::Missing,
            CompanionState::Stopped,
            CompanionState::Outdated,
            CompanionState::Ready,
        ] {
            assert_no_leak(&state(one, Some("0.1.0"))).unwrap_or_else(|why| panic!("{one:?} 가 샌다: {why}"));
        }
    }

    #[test]
    fn js12_the_gate_actually_catches_each_kind_of_leak() {
        // 문지기가 실제로 잡는지 — 잡지 못하면 위의 통과는 의미가 없다.
        // 실제 사람의 계정 이름을 시험 자료에도 적지 않는다.
        for leak in [
            "/Applications/GIL Companion.app 에 있다",
            "~/Applications 에서 찾는다",
            "./companion/make-app.sh 를 돌려라",
            "127.0.0.1:8788 로 열었다",
            "PID 1653 이 답한다",
            "socket 이 없다",
        ] {
            assert!(assert_no_leak(leak).is_err(), "문지기가 놓쳤다: {leak}");
        }
    }

    #[test]
    fn an_unknown_version_is_said_the_way_js_says_it() {
        assert!(state(CompanionState::Stopped, None).contains("알 수 없음"));
        assert!(state(CompanionState::Ready, Some("0.2.0")).contains("0.2.0"));
    }
}
