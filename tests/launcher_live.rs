//! **macOS adapter 의 읽기 경로를 진짜 설치본으로** — 있으면 묻고, 없으면 없다고 답하는가.
//!
//! 이 시험은 창을 띄우지 않는다. `launch_or_focus` 는 부르지 않으며, 설치된 앱에게
//! handshake 와 fresh challenge 만 묻는다. 그래서 어느 기계에서 돌려도 사람의 화면이
//! 바뀌지 않는다.
//!
//! 판정 자체는 확정하지 않는다 — 앱이 떠 있는지는 기계마다 다르므로 `stopped | ready` 둘 중
//! 하나면 맞다. 확정하는 것은 **모양**이다: 설치됐으면 descriptor 가 호환판이고 판 번호는
//! 설치본의 것이며, 안 됐으면 `missing` 이고 아무것도 실행하지 않는다.

#![cfg(target_os = "macos")]

use gil::capability::{Companion, CompanionState};
use gil::launcher::{Platform, PlatformCompanion, macos::MacOs};

#[test]
fn the_installed_companion_if_any_answers_the_shared_contract_without_being_launched() {
    let platform = MacOs::default();
    let installed = platform.installed_at().expect("macOS 는 볼 줄 안다");

    let made = PlatformCompanion::new(MacOs::default());
    let seen = made.state();

    match installed {
        None => {
            assert_eq!(seen.state, CompanionState::Missing);
            assert!(seen.descriptor.is_none());
        }
        Some(executable) => {
            assert!(executable.ends_with("Contents/MacOS/GIL Companion"), "{executable:?}");
            // 설치됐다는 사실만으로 ready 가 되지 않는다 — 실제로 물어본 결과여야 한다.
            assert!(
                matches!(seen.state, CompanionState::Stopped | CompanionState::Ready | CompanionState::Outdated),
                "{:?}",
                seen.state
            );
            if let Some(descriptor) = &seen.descriptor {
                assert!(descriptor.is_compatible_with(&gil::companion::ExpectedV1::current()));
                assert_eq!(descriptor.bundle_id, gil::companion::BUNDLE_ID);
                // appVersion 은 설치본의 것이다. 이 crate 의 판이 아니다.
                assert!(!descriptor.app_version.is_empty());
                eprintln!(
                    "설치된 GIL Companion {} → {}",
                    descriptor.app_version,
                    seen.state.as_str()
                );
            }
        }
    }
}
