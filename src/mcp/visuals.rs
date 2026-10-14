//! App-only visual reads. Never accept a filesystem path or caller-supplied digest.
use super::monitor::{Monitor, Refusal};
use crate::visual_report::{VisualAssetId, registry::ReportOwner};
use base64::Engine;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub const CHUNK: usize = 256 * 1024;
pub fn bounded(mut result: rmcp::model::CallToolResult) -> rmcp::model::CallToolResult {
    if serde_json::to_vec(&result).map_or(true, |bytes| bytes.len() > 1024 * 1024) {
        result = rmcp::model::CallToolResult::structured(
            json!({"code":"response_too_large","said":"시각 자료 응답이 한도를 넘었다"}),
        );
        result.is_error = Some(true);
    }
    result
}
#[derive(Deserialize)]
#[serde(
    tag = "kind",
    content = "ref",
    rename_all = "snake_case",
    deny_unknown_fields
)]
enum Owner {
    Step(String),
    Cycle(String),
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Request {
    scope_id: String,
    owner: Owner,
    asset_id: Option<String>,
    offset: Option<u64>,
}
fn invalid() -> Refusal {
    Refusal::new(
        "invalid_request",
        "정확한 보고서·자료 주소와 조각 위치가 필요하다",
    )
}
impl Request {
    fn owner(&self) -> Result<ReportOwner, Refusal> {
        match &self.owner {
            Owner::Step(s) => {
                let r: crate::StepRef = s.parse().map_err(|_| invalid())?;
                if r.to_string() != *s {
                    return Err(invalid());
                }
                Ok(ReportOwner::Step(r))
            }
            Owner::Cycle(s) => {
                let r: crate::CycleRef = s.parse().map_err(|_| invalid())?;
                if r.to_string() != *s {
                    return Err(invalid());
                }
                Ok(ReportOwner::Cycle(r))
            }
        }
    }
}
pub fn valid(input: &Value, image: bool) -> bool {
    let Ok(r) = serde_json::from_value::<Request>(input.clone()) else {
        return false;
    };
    let Some(hash) = r.scope_id.strip_prefix("project:") else {
        return false;
    };
    if hash.len() != 64
        || !hash
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        || r.owner().is_err()
    {
        return false;
    }
    if !image {
        return !input.as_object().unwrap().contains_key("asset_id")
            && !input.as_object().unwrap().contains_key("offset");
    }
    r.asset_id.as_ref().is_some_and(|s| {
        s.parse::<std::num::NonZeroU64>()
            .is_ok_and(|n| n.to_string() == *s)
    }) && r
        .offset
        .is_some_and(|n| n < 16 * 1024 * 1024 && n % CHUNK as u64 == 0)
}
pub fn call(monitor: &Monitor, input: &Value, image: bool) -> Result<Value, Refusal> {
    if !valid(input, image) {
        return Err(invalid());
    }
    let r: Request = serde_json::from_value(input.clone()).map_err(|_| invalid())?;
    let owner = r.owner()?;
    if !image {
        return monitor.report_visuals(&r.scope_id, owner);
    }
    let asset = VisualAssetId::new(
        r.asset_id
            .as_ref()
            .unwrap()
            .parse()
            .map_err(|_| invalid())?,
    );
    let verified = monitor.report_image(&r.scope_id, owner, asset)?;
    let bytes = verified.bytes();
    let offset = r.offset.unwrap() as usize;
    if offset >= bytes.len() {
        return Err(invalid());
    }
    let end = (offset + CHUNK).min(bytes.len());
    Ok(
        json!({"schema_version":1,"scope_id":r.scope_id,"owner":input["owner"],
        "asset_id":r.asset_id,"mime_type":"image/png","sha256":format!("{:x}",Sha256::digest(bytes)),
        "total_bytes":bytes.len(),"offset":offset,"next_offset":if end<bytes.len(){Some(end)}else{None},
        "data_base64":base64::engine::general_purpose::STANDARD.encode(&bytes[offset..end])}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bound_includes_structured_and_text_copies_without_truncation() {
        let huge =
            rmcp::model::CallToolResult::structured(json!({"caption":"x".repeat(1024*1024)}));
        let result = bounded(huge);
        assert_eq!(result.is_error, Some(true));
        assert_eq!(
            result.structured_content.unwrap()["code"],
            "response_too_large"
        );
        let chunk = rmcp::model::CallToolResult::structured(
            json!({"data_base64":base64::engine::general_purpose::STANDARD.encode(vec![0;CHUNK])}),
        );
        assert_ne!(bounded(chunk).is_error, Some(true));
    }
    #[test]
    fn strict_input_preserves_large_ids_and_refuses_null_extras() {
        let request = json!({"scope_id":format!("project:{}","a".repeat(64)),"owner":{"kind":"cycle","ref":"cycle:C1"},"asset_id":u64::MAX.to_string(),"offset":0});
        assert!(valid(&request, true));
        assert!(!valid(&request, false));
        let mut list = request.clone();
        list.as_object_mut().unwrap().remove("asset_id");
        list.as_object_mut().unwrap().remove("offset");
        assert!(valid(&list, false));
        list["offset"] = Value::Null;
        assert!(!valid(&list, false));
    }
}
