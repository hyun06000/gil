//! 준비 상태·제안·정식 시작 결과의 읽기 값 — 모델 도구와 Monitor 가 같은 값을 본다. capability 는 여기 없다.

use std::path::Path;

use base64::Engine as _;
use serde_json::{Value, json};

use super::ledger::{Disk, Event, Ledger, Proposal};
use super::{B64, StartedRecord, tr};
use crate::artifact::start_policy as sp;

/// 목록을 화면에 다 싣지 않는다 — 수와 앞부분만. 전체는 준비 기록에 있다.
const SHOWN: usize = 200;

pub(super) fn sync_word(base: &sp::SyncBase) -> &'static str {
    match base {
        sp::SyncBase::Absent => "absent",
        sp::SyncBase::Present(_) => "present",
        sp::SyncBase::Unconfirmed => "unconfirmed",
    }
}

fn disk_word(disk: &Disk) -> &'static str {
    match disk {
        Disk::Absent => "absent",
        Disk::Present { .. } => "present",
        Disk::Unconfirmed => "unconfirmed",
    }
}

/// 준비의 단계 — 저장된 기록에서만 유도한다. 연결 끊김·앱 종료는 단계가 아니다.
pub(super) fn phase(ledger: &Ledger, started: Option<&StartedRecord>) -> &'static str {
    if started.is_some() {
        return "started";
    }
    if ledger.canceled() {
        return "canceled";
    }
    let Some(latest) = ledger.latest() else { return "preparing" };
    match ledger.response(&latest.digest).map(|r| r.decision.as_str()) {
        Some("approve") => "approved",
        Some(_) => "changes_requested",
        None if !latest.blockers.is_empty() => "blocked",
        None => "awaiting_approval",
    }
}

pub(super) fn status(root: &Path, ledger: &Ledger, started: Option<&StartedRecord>) -> Value {
    let phase = phase(ledger, started);
    let mut out = json!({
        "phase": phase,
        "said": tr(&format!("start_prep.phase.{phase}")),
        "next": tr(&format!("start_prep.next.{phase}")),
        "pre_start_policy_file_base": disk_word(&ledger.base),
        "history": history(ledger),
        "project_root": root.display().to_string(),
    });
    if let Some(latest) = ledger.latest() {
        out["revision"] = json!(latest.revision);
        out["proposal_digest"] = json!(latest.digest);
    }
    if let Some(Event::CombineAnnounced { approved_revision, disk_digest }) = ledger.events.iter().rev()
        .find(|e| matches!(e, Event::CombineAnnounced { .. }))
        && !ledger.latest().is_some_and(|p| p.combination.is_some()) {
        out["combination_pending"] = json!({"approved_revision": approved_revision, "disk_digest": disk_digest});
    }
    if let Some(s) = started {
        out["started"] = started_fields(s);
    }
    out
}

/// 사람이 읽는 준비 이력 — 제안·응답·취소·재시작·안내·정책 파일 쓰기. 지운 것이 없다.
fn history(ledger: &Ledger) -> Value {
    let mut items: Vec<Value> = ledger.proposals.iter().map(|p| json!({
        "kind": "proposal", "revision": p.revision, "digest": p.digest,
        "response": ledger.response(&p.digest).map(|r| json!({"decision": r.decision, "source": r.source})),
        "combination_of": p.combination.as_ref().map(|c| c.approved_revision),
    })).collect();
    for e in &ledger.events {
        items.push(serde_json::to_value(e).unwrap_or(Value::Null));
    }
    json!(items)
}

fn started_fields(s: &StartedRecord) -> Value {
    json!({"policy": "policy:P1", "event": "event:M1", "snapshot": s.snapshot, "chain": s.chain,
           "first_interview": "cycle:C1", "proposal_digest": s.proposal_digest, "policy_file_sync_base": s.sync})
}

pub(super) fn started(root: &Path, s: &StartedRecord, repeated: bool) -> Value {
    json!({
        "phase": "started",
        "said": tr(if repeated { "start_prep.started_repeated" } else { "start_prep.started" }),
        "next": tr("start_prep.next.started"),
        "started": started_fields(s),
        "project_root": root.display().to_string(),
        // 승인 기록 저장·정책 파일·보관·정식 시작은 서로 다른 사실이다.
        "facts": {
            "approval_recorded": true,
            "policy_applied": "policy:P1",
            "initial_snapshot_archived": s.snapshot,
            "formal_start_committed": true,
            "policy_file_sync_base": s.sync,
        },
    })
}

pub(super) fn proposal(root: &Path, ledger: &Ledger, p: &Proposal) -> Value {
    let text = String::from_utf8_lossy(&B64.decode(&p.rules).unwrap_or_default()).into_owned();
    let disk_text = super::disk_now(root).ok().flatten().map(|(_, b)| String::from_utf8_lossy(&b).into_owned());
    let effect = match (&p.disk, disk_text.as_deref()) {
        (Disk::Absent, _) => json!({"disk": "absent", "effect": "create"}),
        (Disk::Present { digest }, _) if *digest == p.rules_digest => json!({"disk": "present", "effect": "unchanged"}),
        (Disk::Present { .. }, Some(now)) => json!({"disk": "present", "effect": "replace", "diff": line_diff(now, &text)}),
        (Disk::Present { .. }, None) => json!({"disk": "present", "effect": "replace"}),
        (Disk::Unconfirmed, _) => json!({"disk": "unconfirmed", "effect": "unknown"}),
    };
    let combination = p.combination.as_ref().map(|c| {
        let b = ledger.proposal(&c.approved_digest).map(|b| String::from_utf8_lossy(&B64.decode(&b.rules).unwrap_or_default()).into_owned()).unwrap_or_default();
        let c_text = String::from_utf8_lossy(&B64.decode(&c.disk_rules).unwrap_or_default()).into_owned();
        let b_tracked = ledger.proposal(&c.approved_digest).map(|b| b.tracked.clone()).unwrap_or_default();
        let c_tracked = sp::ApprovedPolicy::compile(sp::EngineVersion::V1, c_text.as_bytes()).ok()
            .and_then(|policy| crate::artifact::evaluate_scope(root, &policy).ok())
            .map(|e| e.tracked().iter().map(|f| f.path.as_str().to_owned()).collect::<Vec<_>>());
        json!({
            "approved_revision": c.approved_revision,
            "approved_rules": b,
            "disk_rules": c_text,
            "diff_vs_approved": line_diff(&b, &text),
            "diff_vs_disk": line_diff(&c_text, &text),
            "tracked_vs_approved": set_diff(&b_tracked, &p.tracked),
            "tracked_vs_disk": c_tracked.as_ref().map(|c| set_diff(c, &p.tracked)),
            "rule_order_effects": rule_effects(&text, &b_tracked, c_tracked.as_deref().unwrap_or(&[]), &p.tracked),
        })
    });
    json!({
        "phase": phase(ledger, None),
        "proposal": {
            "revision": p.revision,
            "digest": p.digest,
            "engine": sp::EngineVersion::V1.label().to_string() + "/" + &p.engine.to_string(),
            "rules": text,
            "rules_digest": p.rules_digest,
            "tracked_count": p.tracked.len(),
            "tracked": p.tracked.iter().take(SHOWN).collect::<Vec<_>>(),
            "tracked_truncated": p.tracked.len() > SHOWN,
            "excluded": p.excluded.iter().take(SHOWN).map(|e| json!({"path": e.path, "kind": e.kind, "reason": e.reason, "interior": e.interior})).collect::<Vec<_>>(),
            "excluded_count": p.excluded.len(),
            "blockers": p.blockers,
            "notices": p.notices,
            "recommendations": p.recommendations.iter().map(|r| json!({
                "key": r.key, "category": r.category, "sources": r.sources, "rules": r.rules,
                "default_selected": r.default_selected, "in_rules": r.in_rules, "reason": r.reason, "impacts": r.impacts,
            })).collect::<Vec<_>>(),
            "policy_file": effect,
            "impact": [tr("start_prep.impact.archive"), tr("start_prep.impact.restore"), tr("start_prep.impact.access"), tr("start_prep.impact.unexplored")],
            "system_protections": system_protections(),
            "combination": combination,
            "response": ledger.response(&p.digest).map(|r| json!({"decision": r.decision, "source": r.source})),
            "approvable": p.blockers.is_empty(),
        },
    })
}

pub(super) fn recommendations(list: &[sp::Recommendation], text: &str) -> Value {
    let lines: std::collections::BTreeSet<&str> = text.lines().collect();
    json!(list.iter().map(|r| json!({
        "key": r.key, "category": r.category.code(), "rules": r.rules, "default_selected": r.preselected,
        "in_rules": !r.rules.is_empty() && r.rules.iter().all(|x| lines.contains(x.as_str())),
        "reason": r.reason.to_string(), "impacts": r.impacts.iter().map(ToString::to_string).collect::<Vec<_>>(),
    })).collect::<Vec<_>>())
}

pub(super) fn system_protections() -> Value {
    json!(sp::system_protections().iter().map(|p| json!({"code": p.protection.code(), "shown": p.shown, "selectable": false})).collect::<Vec<_>>())
}

fn set_diff(from: &[String], to: &[String]) -> Value {
    let a: std::collections::BTreeSet<&String> = from.iter().collect();
    let b: std::collections::BTreeSet<&String> = to.iter().collect();
    json!({
        "added": b.difference(&a).take(SHOWN).collect::<Vec<_>>(),
        "removed": a.difference(&b).take(SHOWN).collect::<Vec<_>>(),
    })
}

/// 결합안에서 B·C 와 판정이 갈린 경로마다 D 의 **마지막 일치 규칙**을 보여 준다 — 재포함 순서의 효과.
fn rule_effects(d_text: &str, b: &[String], c: &[String], d: &[String]) -> Value {
    let Ok(policy) = sp::ApprovedPolicy::compile(sp::EngineVersion::V1, d_text.as_bytes()) else { return json!([]) };
    let b: std::collections::BTreeSet<&String> = b.iter().collect();
    let c: std::collections::BTreeSet<&String> = c.iter().collect();
    let d: std::collections::BTreeSet<&String> = d.iter().collect();
    let all: std::collections::BTreeSet<&String> = b.union(&c).chain(d.iter()).copied().collect();
    json!(all.into_iter().filter(|p| b.contains(p) != d.contains(p) || c.contains(p) != d.contains(p)).take(SHOWN).map(|p| {
        let decided = crate::artifact::EntryPath::parse(p).map(|path| match policy.decide(&path, sp::PathKind::File) {
            sp::Decision::Tracked { by: Some(rule) } => format!("tracked by line {}: {}", rule.line, rule.pattern),
            sp::Decision::Tracked { by: None } => "tracked (no rule matched)".into(),
            sp::Decision::Excluded { rule } => format!("excluded by line {}: {}", rule.line, rule.pattern),
            other => format!("{other:?}"),
        }).unwrap_or_default();
        json!({"path": p, "approved": b.contains(p), "disk": c.contains(p), "combined": d.contains(p), "decided": decided})
    }).collect::<Vec<_>>())
}

/// 줄 단위 차이(최장 공통 부분열). 정책 파일은 짧다 — 너무 길면 차이 대신 그 사실만 말한다.
pub(super) fn line_diff(from: &str, to: &str) -> Value {
    let a: Vec<&str> = from.lines().collect();
    let b: Vec<&str> = to.lines().collect();
    if a.len() * b.len() > 4_000_000 {
        return json!({"too_large": true});
    }
    let mut table = vec![vec![0u32; b.len() + 1]; a.len() + 1];
    for i in (0..a.len()).rev() {
        for j in (0..b.len()).rev() {
            table[i][j] = if a[i] == b[j] { table[i + 1][j + 1] + 1 } else { table[i + 1][j].max(table[i][j + 1]) };
        }
    }
    let (mut i, mut j, mut out) = (0, 0, Vec::new());
    while i < a.len() || j < b.len() {
        if i < a.len() && j < b.len() && a[i] == b[j] { out.push(format!("  {}", a[i])); i += 1; j += 1; }
        else if j < b.len() && (i == a.len() || table[i][j + 1] >= table[i + 1][j]) { out.push(format!("+ {}", b[j])); j += 1; }
        else { out.push(format!("- {}", a[i])); i += 1; }
    }
    json!(out)
}
