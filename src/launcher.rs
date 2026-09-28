//! **바깥 세계에 닿는 문** — Companion 이 설치됐는지, 답하는지, 띄울 수 있는지.
//!
//! `server.mjs` 의 `realPorts().companion` 을 옮긴 것이다. 판정은 여기 없다 — 그것은
//! [`crate::companion::classify`] 가 하고, 조율은 [`crate::capability::open_monitor`] 가 한다.
//! 여기 남는 것은 **platform 이 다르면 달라지는 네 동작**뿐이고, 그것이 [`Platform`] 이다.
//!
//! ```text
//! installed_at            설치 자리를 찾는다 — 있다는 사실은 증거가 아니다
//! descriptor_of           설치된 binary 에게 handshake 를 묻는다 (process 하나, 짧게)
//! answers_fresh_challenge 실행 중인 것이 **새 challenge** 를 그대로 돌려주는가
//! launch_or_focus         identity 로 띄우거나 앞으로 가져온다
//! ```
//!
//! # 명령을 글자로 잇지 않는다
//!
//! 실행은 **고정된 bundle identifier** 하나로만 한다. 사용자의 입력도, 임의의 경로도
//! 명령줄에 이어 붙이지 않는다. shell 을 거치지 않는다.
//!
//! # 이 조각에서 platform 은 macOS 하나다
//!
//! Windows 는 같은 trait 의 자리만 있고 typed 로 `unsupported` 를 답한다. 거짓 `missing` 을
//! 만들지 않는다 — 설치가 안 된 것과 이 platform 이 아직 볼 줄 모르는 것은 다른 사실이다.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use crate::capability::{Companion, MonitorIntent, Seen};
use crate::companion::{ARG, BUNDLE_ID, DescriptorV1, ExpectedV1, InstallationState, PROBE_ARG, RuntimeReplyV1, classify};

/// platform 이 다르면 달라지는 네 동작. 나머지는 전부 platform 중립이다.
pub trait Platform {
    /// 설치된 app 의 실행 파일. 없으면 `None`. **이 platform 이 볼 줄 모르면** `Err`.
    fn installed_at(&self) -> Result<Option<PathBuf>, Unsupported>;
    /// binary 에게 handshake 를 물어 descriptor 를 받는다. 답이 없거나 모양이 다르면 `None`.
    fn descriptor_of(&self, executable: &Path) -> Option<DescriptorV1>;
    /// 실행 중인 process 가 **새 challenge** 에 답하는가. 답은 descriptor 와 같아야 한다.
    fn answers_fresh_challenge(&self, executable: &Path, described: &DescriptorV1) -> bool;
    /// identity 로 띄우거나 앞으로 가져온다. 실패 이유는 버린다 — 거기엔 경로가 들어 있다.
    fn launch_or_focus(&self) -> bool;
}

/// 이 platform 의 launcher 가 아직 없다. `missing` 이 아니다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unsupported;

/// [`Platform`] 위에 [`Companion`] 을 짓는다 — `server.mjs` 의 `realPorts().companion` 과 같은 자리.
pub struct PlatformCompanion<P: Platform> {
    platform: P,
    /// `state()` 가 찾은 실행 파일. `probe_ready` 가 다시 찾지 않게.
    found: std::cell::RefCell<Option<PathBuf>>,
}

impl<P: Platform> PlatformCompanion<P> {
    pub fn new(platform: P) -> Self {
        Self {
            platform,
            found: std::cell::RefCell::new(None),
        }
    }
}

impl<P: Platform> Companion for PlatformCompanion<P> {
    fn state(&self) -> Seen {
        let Ok(place) = self.platform.installed_at() else {
            // 볼 줄 모르는 platform 이다. 설치가 안 된 것처럼 말하지 않지만, 표면으로는
            // 열 수 없으므로 `missing` 과 같은 자리(설치 안내)로 보낸다.
            *self.found.borrow_mut() = None;
            return Seen { state: InstallationState::Missing, descriptor: None };
        };
        let Some(executable) = place else {
            *self.found.borrow_mut() = None;
            return Seen { state: InstallationState::Missing, descriptor: None };
        };
        let described = self.platform.descriptor_of(&executable);
        *self.found.borrow_mut() = Some(executable.clone());
        let Some(described) = described else {
            return Seen { state: InstallationState::Outdated, descriptor: None };
        };
        // 판정은 공용 `classify` 하나다. 여기서 다시 적지 않는다.
        let challenge = fresh_challenge();
        let reply = self
            .platform
            .answers_fresh_challenge(&executable, &described)
            .then(|| RuntimeReplyV1 {
                schema_version: crate::companion::SCHEMA_VERSION,
                challenge: challenge.clone(),
                companion: described.clone(),
            });
        let state = classify(true, Some(&described), reply.as_ref(), &challenge, ExpectedV1::current());
        Seen { state, descriptor: Some(described) }
    }

    // 실행과 앞으로 가져오기는 macOS 에서 같은 동작이다 — 이미 떠 있으면 그 창이 앞으로
    // 오고, 아니면 뜬다. 둘을 따로 둔 것은 **뜻이 다르기 때문**이고, coordinator 가 그 둘을
    // 다른 결과로 보고하기 때문이다.
    fn launch(&self, _: MonitorIntent) -> bool {
        self.platform.launch_or_focus()
    }

    fn focus(&self, _: MonitorIntent) -> bool {
        self.platform.launch_or_focus()
    }

    fn probe_ready(&self, descriptor: &DescriptorV1) -> bool {
        match self.found.borrow().as_deref() {
            Some(executable) => self.platform.answers_fresh_challenge(executable, descriptor),
            None => false,
        }
    }
}

/// 매번 새로 짓는다. 한 번의 성공을 재사용하지 않는다.
fn fresh_challenge() -> String {
    use std::hash::{BuildHasher, Hasher};
    let mut hasher = std::collections::hash_map::RandomState::new().build_hasher();
    hasher.write_u128(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0),
    );
    hasher.write_u32(std::process::id());
    let first = hasher.finish();
    hasher.write_u64(first.rotate_left(17));
    format!("gil_mcp_{first:016x}{:016x}", hasher.finish())
}

// ── macOS ─────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
pub mod macos {
    use super::*;

    /// 설치되어 있을 수 있는 자리. 사용자 영역이 먼저다.
    fn places() -> Vec<PathBuf> {
        let app = format!("{}.app", crate::companion::APP_NAME);
        let mut seen = Vec::new();
        if let Some(home) = std::env::var_os("HOME") {
            seen.push(PathBuf::from(home).join("Applications").join(&app));
        }
        seen.push(PathBuf::from("/Applications").join(app));
        seen
    }

    fn binary_at(place: &Path) -> PathBuf {
        place.join("Contents").join("MacOS").join(crate::companion::APP_NAME)
    }

    /// 짧게 한 줄만 받는다. 매달리지 않는다.
    fn run_line(executable: &Path, arg: &[&str], timeout: Duration, cap: usize) -> Option<String> {
        let mut child = Command::new(executable)
            .args(arg)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .ok()?;
        let started = std::time::Instant::now();
        loop {
            match child.try_wait() {
                Ok(Some(status)) if status.success() => break,
                Ok(Some(_)) => return None,
                Ok(None) if started.elapsed() > timeout => {
                    let _ = child.kill();
                    let _ = child.wait();
                    return None;
                }
                Ok(None) => std::thread::sleep(Duration::from_millis(10)),
                Err(_) => return None,
            }
        }
        use std::io::Read as _;
        let mut said = String::new();
        child.stdout.take()?.take(cap as u64).read_to_string(&mut said).ok()?;
        (said.len() <= cap).then_some(said)
    }

    pub struct MacOs {
        pub probe_timeout: Duration,
    }

    impl Default for MacOs {
        fn default() -> Self {
            Self {
                probe_timeout: Duration::from_millis(crate::capability::Policy::default().probe_timeout_ms),
            }
        }
    }

    impl Platform for MacOs {
        fn installed_at(&self) -> Result<Option<PathBuf>, Unsupported> {
            Ok(places().into_iter().find(|place| place.exists()).map(|place| binary_at(&place)))
        }

        fn descriptor_of(&self, executable: &Path) -> Option<DescriptorV1> {
            let said = run_line(executable, &[ARG], self.probe_timeout, 16 * 1024)?;
            let one: DescriptorV1 = serde_json::from_str(said.trim_end()).ok()?;
            one.is_compatible_with(&ExpectedV1::current()).then_some(one)
        }

        fn answers_fresh_challenge(&self, executable: &Path, described: &DescriptorV1) -> bool {
            let challenge = fresh_challenge();
            let Some(said) = run_line(executable, &[PROBE_ARG, &challenge], self.probe_timeout, 32 * 1024) else {
                return false;
            };
            let Ok(replied) = serde_json::from_str::<RuntimeReplyV1>(said.trim_end()) else {
                return false;
            };
            replied.schema_version == crate::companion::SCHEMA_VERSION
                && replied.challenge == challenge
                && replied.companion == *described
        }

        /// **`open -b` 는 이 안에만 산다.** 인수는 identity 하나이고 조립하지 않는다.
        fn launch_or_focus(&self) -> bool {
            Command::new("open")
                .args(["-b", BUNDLE_ID])
                .stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
                .map(|status| status.success())
                .unwrap_or(false)
        }
    }
}

// ── Windows — 자리만 ──────────────────────────────────────────────────

#[cfg(windows)]
pub mod windows {
    use super::*;

    /// 아직 launcher 가 없다. 네 동작 모두 typed 로 "볼 줄 모른다" 고 답한다.
    pub struct Windows;

    impl Platform for Windows {
        fn installed_at(&self) -> Result<Option<PathBuf>, Unsupported> {
            Err(Unsupported)
        }
        fn descriptor_of(&self, _: &Path) -> Option<DescriptorV1> {
            None
        }
        fn answers_fresh_challenge(&self, _: &Path, _: &DescriptorV1) -> bool {
            false
        }
        fn launch_or_focus(&self) -> bool {
            false
        }
    }
}

/// 이 기계의 platform. 없으면 `None` — 그때 coordinator 는 설치 안내 자리로 간다.
pub fn this_machine() -> Option<Box<dyn Platform>> {
    #[cfg(target_os = "macos")]
    {
        Some(Box::new(macos::MacOs::default()))
    }
    #[cfg(windows)]
    {
        Some(Box::new(windows::Windows))
    }
    #[cfg(not(any(target_os = "macos", windows)))]
    {
        None
    }
}

impl Platform for Box<dyn Platform> {
    fn installed_at(&self) -> Result<Option<PathBuf>, Unsupported> {
        (**self).installed_at()
    }
    fn descriptor_of(&self, executable: &Path) -> Option<DescriptorV1> {
        (**self).descriptor_of(executable)
    }
    fn answers_fresh_challenge(&self, executable: &Path, described: &DescriptorV1) -> bool {
        (**self).answers_fresh_challenge(executable, described)
    }
    fn launch_or_focus(&self) -> bool {
        (**self).launch_or_focus()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    /// 가짜 platform — 파일도 process 도 없다.
    struct Fake {
        installed: Result<Option<PathBuf>, Unsupported>,
        descriptor: Option<DescriptorV1>,
        answers: bool,
        launched: RefCell<u32>,
    }

    impl Platform for Fake {
        fn installed_at(&self) -> Result<Option<PathBuf>, Unsupported> {
            self.installed.clone()
        }
        fn descriptor_of(&self, _: &Path) -> Option<DescriptorV1> {
            self.descriptor.clone()
        }
        fn answers_fresh_challenge(&self, _: &Path, described: &DescriptorV1) -> bool {
            self.answers && Some(described) == self.descriptor.as_ref()
        }
        fn launch_or_focus(&self) -> bool {
            *self.launched.borrow_mut() += 1;
            true
        }
    }

    fn fake(installed: Option<&str>, descriptor: Option<DescriptorV1>, answers: bool) -> PlatformCompanion<Fake> {
        PlatformCompanion::new(Fake {
            installed: Ok(installed.map(PathBuf::from)),
            descriptor,
            answers,
            launched: RefCell::new(0),
        })
    }

    #[test]
    fn the_four_states_come_out_of_the_four_platform_facts() {
        let d = DescriptorV1::for_app("0.1.0");
        assert_eq!(fake(None, None, false).state().state, InstallationState::Missing);
        assert_eq!(fake(Some("/x"), None, false).state().state, InstallationState::Outdated);
        assert_eq!(fake(Some("/x"), Some(d.clone()), false).state().state, InstallationState::Stopped);
        assert_eq!(fake(Some("/x"), Some(d), true).state().state, InstallationState::Ready);
    }

    #[test]
    fn an_unsupported_platform_is_not_reported_as_installed_or_ready() {
        let made = PlatformCompanion::new(Fake {
            installed: Err(Unsupported),
            descriptor: Some(DescriptorV1::for_app("0.1.0")),
            answers: true,
            launched: RefCell::new(0),
        });
        let seen = made.state();
        assert_eq!(seen.state, InstallationState::Missing);
        assert_eq!(seen.descriptor, None);
        assert!(!made.probe_ready(&DescriptorV1::for_app("0.1.0")), "찾지도 못했는데 ready 라 했다");
    }

    #[test]
    fn the_app_version_comes_from_the_installed_descriptor_not_this_crate() {
        let seen = fake(Some("/x"), Some(DescriptorV1::for_app("7.7.7")), false).state();
        assert_eq!(seen.descriptor.expect("descriptor").app_version, "7.7.7");
    }

    #[test]
    fn probe_ready_reuses_what_state_found_and_refuses_before_it() {
        let d = DescriptorV1::for_app("0.1.0");
        let made = fake(Some("/x"), Some(d.clone()), true);
        assert!(!made.probe_ready(&d), "state() 전에 ready 라 했다");
        made.state();
        assert!(made.probe_ready(&d));
    }

    #[test]
    fn every_challenge_is_fresh_and_well_formed() {
        let a = fresh_challenge();
        let b = fresh_challenge();
        assert_ne!(a, b);
        assert!(crate::companion::valid_challenge(&a), "{a}");
    }
}
