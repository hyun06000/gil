//! 범위 변경 도구의 App capability 경계 시험 — 실제 MCP 호출 함수(`call`)·카탈로그·입력 검사를 그대로 쓴다. 합성 값만 쓴다.
//!
//! 여기서 보는 것은 **경계**뿐이다: 어느 도구가 App 전용인가, capability 가 쓰임과 정확한 대상에 묶이고 한 번만 쓰이는가, 모델 꼴의
//! 입력이 승인·재시도·중단·해제의 자리를 얻지 못하는가. 프로젝트 기록의 전이는 `scope_change::tests` 가, 실제 stdio 는
//! `tests/tracking_scope_change.rs` 가 본다.

use super::*;

#[test]
fn approval_retry_halt_and_release_are_app_only_and_the_model_tools_carry_no_such_slot() {
    let tools = serde_json::to_value(catalog(Vec::new())).unwrap();
    let tools = tools.as_array().unwrap();
    assert_eq!(tools.len(), NAMES.len());
    for tool in tools {
        let name = tool["name"].as_str().unwrap();
        let visibility = &tool["_meta"]["ui"]["visibility"];
        match APP_NAMES.contains(&name) {
            true => assert_eq!(visibility, &json!(["app"]), "{name}"),
            false => {
                assert_eq!(visibility, &json!(["model"]), "{name}");
                let properties = tool["inputSchema"]["properties"].as_object().unwrap();
                for slot in ["token", "decision", "scope_id", "state_confirmed", "cleanup", "event"] {
                    assert!(!properties.contains_key(slot), "{name} 에 {slot} 자리가 있다");
                }
            }
        }
    }
    // 모델 꼴의 입력(project_root)은 App 전용 도구의 입력 검사를 지나지 못한다.
    for name in APP_NAMES {
        assert!(!valid(name, &json!({"project_root": "/tmp/x"})), "{name}");
    }
    assert!(valid("gil_scope_propose", &json!({"project_root": "/tmp/x", "rules": "a/\n"})));
    assert!(!valid("gil_scope_propose", &json!({"project_root": "/tmp/x", "rules": "a/\n", "approved": true})));
    assert!(!valid("gil_scope_status", &json!({"project_root": "/tmp/x", "retry": true})));
    // 해제는 화면에서 상태를 확인했다는 명시 입력이 없으면 꼴부터 맞지 않는다.
    assert!(!valid("gil_scope_release", &json!({"scope_id": "project:x", "at": "event:M2", "cleanup": "continue", "token": "t"})));
}

#[test]
fn a_capability_is_bound_to_its_scope_purpose_and_exact_target_and_is_spent_once() {
    let grants = Grants::default();
    let decide = grants.token("project:a", Purpose::Decide, "1:abc").unwrap();
    assert_eq!(decide.len(), 64);
    assert!(!format!("{grants:?}").contains(&decide), "Debug 출력에 capability 가 없다");
    // 같은 대상의 다시 읽기는 같은 값을 준다 — 화면을 다시 그려도 버튼이 무효가 되지 않는다.
    assert_eq!(grants.token("project:a", Purpose::Decide, "1:abc").unwrap(), decide);
    // 쓰임이 다르면 다른 값이다. 승인 capability 로 재시도·중단·해제를 하지 못한다.
    let retry = grants.token("project:a", Purpose::Retry, "event:M2").unwrap();
    let release = grants.token("project:a", Purpose::Release, "event:M2").unwrap();
    assert!(decide != retry && retry != release);
    for (purpose, target) in [(Purpose::Retry, "event:M2"), (Purpose::Halt, "halt"), (Purpose::Release, "event:M2")] {
        assert!(grants.take("project:a", purpose, target, &decide).is_err());
    }
    // 다른 scope, 다른 대상(다른 판·digest), 추측한 값, 새 프로세스(빈 grants).
    assert!(grants.take("project:b", Purpose::Decide, "1:abc", &decide).is_err());
    assert!(grants.take("project:a", Purpose::Decide, "2:abc", &decide).is_err());
    assert!(grants.take("project:a", Purpose::Decide, "1:abc", &"f".repeat(64)).is_err());
    assert!(Grants::default().take("project:a", Purpose::Decide, "1:abc", &decide).is_err());
    // 틀린 시도는 맞는 capability 를 소모하지 않는다. 맞는 것은 한 번만 쓰인다.
    assert!(grants.take("project:a", Purpose::Decide, "1:abc", &decide).is_ok());
    assert!(grants.take("project:a", Purpose::Decide, "1:abc", &decide).is_err(), "재전송은 기록 조회가 답한다");
    // 대상이 바뀌면(새 제안 판) 앞선 값은 더는 통하지 않는다.
    let newer = grants.token("project:a", Purpose::Decide, "2:def").unwrap();
    assert!(grants.take("project:a", Purpose::Decide, "1:abc", &newer).is_err());
    assert!(grants.take("project:a", Purpose::Decide, "2:def", &newer).is_ok());
    // 해제의 대상은 화면에 보인 표식의 자리다.
    assert!(grants.take("project:a", Purpose::Release, "cycle:C1", &release).is_err());
    assert!(grants.take("project:a", Purpose::Release, "event:M2", &release).is_ok());
}
