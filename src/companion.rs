//! **Companion 설치 handshake v1 의 공용 계약** — 묻는 쪽과 답하는 쪽이 같은 한 벌을 쓴다.
//!
//! 이 파일은 `companion/src/handshake.rs` 에 있던 것을 **그대로 올린 것**이다. 올린 이유는
//! 하나다 — 묻는 쪽이 둘이 됐기 때문이다. JS launcher 가 상수와 판정을 제 손으로 베껴 두 벌을
//! 만들었고, Rust MCP 가 그 자리를 이어받으면 세 벌이 된다. 그래서 계약은 여기 한 곳에 살고,
//! Companion 앱은 답하는 쪽으로서, MCP 는 묻는 쪽으로서 **같은 것**을 `use` 한다.
//!
//! 여기 있는 것은 identity·범위·판정뿐이다. socket·process·Tauri 는 없다 — 그것은 답하는
//! 쪽(Companion crate)의 것이고, 파일 존재 확인·실행·`open -b` 는 묻는 쪽(platform port)의
//! 것이다. 이 파일은 바깥 세계에 손대지 않으므로 어느 crate 에서든 결정적으로 시험된다.
//!
//! **Agent Core 의 handshake([`crate::agent`])와 뜻이 다르다.** 그쪽은 "Agent 가 부를 Core 가
//! 이 판과 맞는가", 이쪽은 "사람이 볼 창이 떠 있는가" 다. 같은 모양이라고 합치지 않는다.

use serde::{Deserialize, Serialize};

pub const ARG: &str = "--gil-companion-handshake";
pub const PROBE_ARG: &str = "--gil-companion-probe";
pub const SCHEMA_VERSION: u32 = 1;
pub const PROTOCOL_VERSION: u32 = 1;
pub const PRODUCT: &str = "gil_companion";
pub const BUNDLE_ID: &str = "dev.ariadne.gil.companion";
/// macOS 의 app bundle 이름. `open -b` 는 identity 로 부르지만, 설치 자리를 찾을 때는 이 이름이다.
pub const APP_NAME: &str = "GIL Companion";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RangeV1 {
    pub min: u32,
    pub max: u32,
}

impl RangeV1 {
    pub fn exact(version: u32) -> Self {
        Self {
            min: version,
            max: version,
        }
    }

    pub fn contains(&self, version: u32) -> bool {
        self.min <= version && version <= self.max
    }
}

/// 설치된 binary 가 스스로 말하는 공개 계약. 경로·PID·Project 는 싣지 않는다.
///
/// 필드 순서가 곧 wire 의 순서다 — 바꾸면 JS launcher 가 보던 JSON 과 달라진다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DescriptorV1 {
    pub schema_version: u32,
    pub product: String,
    pub bundle_id: String,
    pub app_version: String,
    pub protocol: RangeV1,
    pub monitor_view_schema: RangeV1,
    pub node_detail_schema: RangeV1,
}

impl DescriptorV1 {
    /// **답하는 쪽**이 자기 판으로 짓는다. `app_version` 은 부르는 쪽이 넘긴다 — 이 crate 의
    /// `CARGO_PKG_VERSION` 을 쓰면 `gil` 의 판을 Companion 의 판이라고 말하게 된다.
    pub fn for_app(app_version: &str) -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            product: PRODUCT.to_string(),
            bundle_id: BUNDLE_ID.to_string(),
            app_version: app_version.to_string(),
            protocol: RangeV1::exact(PROTOCOL_VERSION),
            monitor_view_schema: RangeV1::exact(crate::SCHEMA_VERSION),
            node_detail_schema: RangeV1::exact(crate::SCHEMA_VERSION),
        }
    }

    pub fn is_compatible_with(&self, expected: &ExpectedV1) -> bool {
        self.schema_version == SCHEMA_VERSION
            && self.product == PRODUCT
            && self.bundle_id == BUNDLE_ID
            && self.protocol.contains(expected.protocol)
            && self.monitor_view_schema.contains(expected.monitor_view_schema)
            && self.node_detail_schema.contains(expected.node_detail_schema)
    }
}

/// 실행 중인 Companion 이 **지금** 답했다는 증거. 파일을 읽은 것과 구별된다.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeReplyV1 {
    pub schema_version: u32,
    pub challenge: String,
    pub companion: DescriptorV1,
}

/// 묻는 쪽이 요구하는 protocol 과 두 wire schema.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExpectedV1 {
    pub protocol: u32,
    pub monitor_view_schema: u32,
    pub node_detail_schema: u32,
}

impl ExpectedV1 {
    pub fn current() -> Self {
        Self {
            protocol: PROTOCOL_VERSION,
            monitor_view_schema: crate::SCHEMA_VERSION,
            node_detail_schema: crate::SCHEMA_VERSION,
        }
    }
}

/// Companion handshake 가 가르는 네 값. 이 밖의 값을 만들지 않는다.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallationState {
    Missing,
    Stopped,
    Outdated,
    Ready,
}

impl InstallationState {
    /// wire 와 사람 문장이 쓰는 그 글자.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Missing => "missing",
            Self::Stopped => "stopped",
            Self::Outdated => "outdated",
            Self::Ready => "ready",
        }
    }
}

/// challenge 의 모양 — 묻는 쪽과 답하는 쪽이 같은 자로 잰다.
pub fn valid_challenge(challenge: &str) -> bool {
    (16..=128).contains(&challenge.len())
        && challenge
            .bytes()
            .all(|one| one.is_ascii_alphanumeric() || one == b'-' || one == b'_')
}

/// launcher 가 관측한 세 사실만으로 상태를 가른다. 경로가 있다는 사실만으로 `ready` 가 되지 않는다.
pub fn classify(
    installed: bool,
    descriptor: Option<&DescriptorV1>,
    runtime_reply: Option<&RuntimeReplyV1>,
    challenge: &str,
    expected: ExpectedV1,
) -> InstallationState {
    if !installed {
        return InstallationState::Missing;
    }
    let Some(descriptor) = descriptor else {
        return InstallationState::Outdated;
    };
    if !descriptor.is_compatible_with(&expected) {
        return InstallationState::Outdated;
    }
    match runtime_reply {
        Some(reply)
            if reply.schema_version == SCHEMA_VERSION
                && reply.challenge == challenge
                && reply.companion == *descriptor =>
        {
            InstallationState::Ready
        }
        _ => InstallationState::Stopped,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn descriptor() -> DescriptorV1 {
        DescriptorV1::for_app("0.1.0")
    }

    fn reply(challenge: &str) -> RuntimeReplyV1 {
        RuntimeReplyV1 {
            schema_version: SCHEMA_VERSION,
            challenge: challenge.to_string(),
            companion: descriptor(),
        }
    }

    #[test]
    fn the_descriptor_carries_the_app_version_it_was_given_not_this_crates() {
        let one = DescriptorV1::for_app("9.9.9");
        assert_eq!(one.app_version, "9.9.9");
        assert_ne!(one.app_version, env!("CARGO_PKG_VERSION"), "gil 의 판이 새어 들어왔다");
        assert_eq!(one.product, PRODUCT);
        assert_eq!(one.bundle_id, BUNDLE_ID);
    }

    #[test]
    fn the_wire_json_is_compact_and_in_the_order_the_js_launcher_read() {
        let said = serde_json::to_string(&descriptor()).expect("JSON");
        assert_eq!(
            said,
            r#"{"schema_version":1,"product":"gil_companion","bundle_id":"dev.ariadne.gil.companion","app_version":"0.1.0","protocol":{"min":1,"max":1},"monitor_view_schema":{"min":1,"max":1},"node_detail_schema":{"min":1,"max":1}}"#
        );
    }

    #[test]
    fn path_presence_alone_never_means_ready() {
        let expected = ExpectedV1::current();
        assert_eq!(classify(false, None, None, "abcdefghijklmnop", expected), InstallationState::Missing);
        assert_eq!(classify(true, None, None, "abcdefghijklmnop", expected), InstallationState::Outdated);
    }

    #[test]
    fn a_compatible_descriptor_distinguishes_stopped_from_ready() {
        let expected = ExpectedV1::current();
        let challenge = "abcdefghijklmnop";
        assert_eq!(
            classify(true, Some(&descriptor()), None, challenge, expected),
            InstallationState::Stopped
        );
        assert_eq!(
            classify(true, Some(&descriptor()), Some(&reply(challenge)), challenge, expected),
            InstallationState::Ready
        );
    }

    #[test]
    fn every_incompatible_boundary_is_outdated() {
        let expected = ExpectedV1::current();
        let mutations: [fn(&mut DescriptorV1); 6] = [
            |one| one.schema_version += 1,
            |one| one.product = "another".into(),
            |one| one.bundle_id = "dev.another.app".into(),
            |one| one.protocol = RangeV1::exact(2),
            |one| one.monitor_view_schema = RangeV1::exact(2),
            |one| one.node_detail_schema = RangeV1::exact(2),
        ];
        for mutate in mutations {
            let mut one = descriptor();
            mutate(&mut one);
            assert_eq!(
                classify(true, Some(&one), None, "abcdefghijklmnop", expected),
                InstallationState::Outdated
            );
        }
    }

    #[test]
    fn ready_requires_the_same_fresh_challenge() {
        assert_eq!(
            classify(
                true,
                Some(&descriptor()),
                Some(&reply("abcdefghijklmnop")),
                "ponmlkjihgfedcba",
                ExpectedV1::current(),
            ),
            InstallationState::Stopped
        );
    }

    #[test]
    fn a_reply_from_a_different_build_is_not_ready() {
        let mut other = reply("abcdefghijklmnop");
        other.companion.app_version = "0.0.1".into();
        assert_eq!(
            classify(true, Some(&descriptor()), Some(&other), "abcdefghijklmnop", ExpectedV1::current()),
            InstallationState::Stopped
        );
    }

    #[test]
    fn the_challenge_shape_is_the_one_both_sides_agree_on() {
        assert!(valid_challenge("abcdefghijklmnop"));
        assert!(valid_challenge(&"x".repeat(128)));
        for bad in ["", "short", &"x".repeat(129), "has space", "sem;colon"] {
            assert!(!valid_challenge(bad), "{bad:?}");
        }
    }
}
