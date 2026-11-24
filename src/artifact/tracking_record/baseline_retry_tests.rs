//! 보관 대기 사건의 명시 재시도(`RetryAttempt`·`run_retry`) 시험 — **실제 임시 폴더와 실제 객체 창고**, 그리고 **시험용 저장 경계**를 쓴다.
//!
//! - 폴더·객체: 시험마다 배타적으로 새로 만든 임시 루트에 합성 파일을 두고, 기존 `capture_scoped`·`observe_scoped`·`ObjectStore` 를
//!   그대로 지난다. 마지막 확정 세계 A1 은 그 폴더를 P1 로 실제 보관한 manifest 다. 끝나면 그 루트만 지운다.
//! - 기록: 정책 판·사건·보충 칸·시도 결과는 시험이 만든 값이다. 모니터가 승인한 기록이 아니고 `state.yaml` 도 `gil start` 도 쓰지 않는다.
//!   승인 저장(저장 1)은 「이미 저장됐다고 건넨 입력」일 뿐이다.
//! - 저장 경계(`Desk`): 메모리의 기록을 고쳐 쓰는 **시험용 어댑터**다. 그 실패 주입은 실제 저장의 원자성·프로세스 강제 종료·전원 장애
//!   내구성의 증거가 아니다. 다시 읽기도 그 메모리 값의 복사다.
//! - 「새로 만든 값으로 다시 읽는다」는 순수 판정의 반복이며 제품 재시작 시험이 아니다. 실제 Host·사람의 승인·필드 인수의 증거도 아니다.
//!
//! ```text
//! 폴더            src/a.txt  src/b.txt  data/raw.csv  notes/todo.md   (+ 루트 .gil/ 창고)
//! P1  "notes/"    data/ 와 src/ 를 추적          A1 = P1 로 보관한 { data/raw.csv, src/a.txt, src/b.txt }
//! P2  "data/"     notes/ 와 src/ 를 추적         계속 추적 = src/*   ·  새로 추적 = notes/*  ·  범위 밖 = data/*
//! 체인            chain:C1  cycle:C1 Interview(뿌리, 닫힘) [└ cycle:C2 Interview(닫힘) …]   모든 Entry·Exit A1
//! 사건            M1 start(없음→P1, 기준점 A1)   M2 scope_change(P1→P2, C1 에서 승인, 발급기 2, world_before A1, 기준점 없음)
//! ```

use std::fs;
use std::path::{Path, PathBuf};

use super::event_cycles_tests::{a, c, ch, close_interview, m, open_interview, rules, x1};
use super::experiment_exit_tests::p;
use super::*;
use crate::artifact::gilignore::{ApprovedPolicy, EngineVersion};
use crate::artifact::scoped::probe;
use crate::artifact::{EntryPath, ObjectStore, SnapshotRegistry, capture_scoped, digest_from_raw, digest_of};
use crate::Chain;
use crate::cycles::Cycles;

use AttemptSeen::{Archived, NotArchived};
use BaselineSlot::{Unwritten, Written};
use EventKind::{ScopeChange, Start};

const P1_TEXT: &[u8] = b"notes/\n";
const P2_TEXT: &[u8] = b"data/\n";

// ── 폴더와 창고 ────────────────────────────────────────────────────────────

/// 이번 호출이 **새로 만든** 임시 루트. 이미 있는 이름은 건드리지 않고 다른 이름으로 다시 시도한다.
fn fresh_root(label: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    loop {
        let candidate = std::env::temp_dir().join(format!(
            "gil-baseline-retry-{}-{}-{}-{label}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        match fs::create_dir(&candidate) {
            Ok(()) => return candidate,
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(err) => panic!("시험 임시 루트를 만들지 못했다 {}: {err}", candidate.display()),
        }
    }
}

fn write(root: &Path, path: &str, bytes: &[u8]) {
    let full = root.join(path);
    fs::create_dir_all(full.parent().expect("루트 아래의 파일이다")).expect("시험 폴더를 만든다");
    fs::write(&full, bytes).unwrap_or_else(|err| panic!("{path} 를 쓰지 못했다: {err}"));
}

fn path(text: &str) -> EntryPath {
    EntryPath::parse(text).expect("시험 경로는 Artifact 경로다")
}

/// 합성 프로젝트 하나 — 임시 루트, 그 `.gil/` 의 실제 창고, 그리고 그 폴더에서 실제로 보관한 A1 을 가리키는 처음 기록.
struct Lab {
    root: PathBuf,
    store: ObjectStore,
    records: OwnedRecords,
}

impl Drop for Lab {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn record(number: u32, text: &[u8]) -> PolicyRecord {
    let rules = digest_of(text).expect("메모리의 바이트는 언제나 읽힌다");
    // 시험용 제안 참조 — 아무 제안 원장도 가리키지 않는 합성 값이다.
    let proposal = ProposalDigest(digest_from_raw([u8::try_from(number).unwrap(); 32]));
    PolicyRecord::new(p(number), EngineVersion::V1, rules, proposal)
}

/// 닫힌 Interview `count` 개의 직선 — 마지막 사이클에 서 있고 모든 Entry·Exit 은 A1 이다.
fn interviews(count: u32) -> Vec<Chain> {
    let mut cycles = Cycles::start_at(rules(), x1(), a(1), 1);
    close_interview(&mut cycles, "open_child");
    for _ in 1..count {
        open_interview(&mut cycles);
        close_interview(&mut cycles, "open_child");
    }
    vec![Chain::restore(ch(1), x1(), a(1), None, cycles)]
}

fn waiting(event: u32, at: u32, issuer: u32, failed: usize) -> EventCycleFacts {
    let baseline = BaselineFacts::new(m(event), ScopeChange, None, Unwritten, Unwritten, vec![NotArchived; failed]);
    EventCycleFacts::new(baseline, ch(1), Written(Some(c(at))), issuer)
}

impl Lab {
    /// 원장 `texts`(P1 부터)와 범위 변경 사건들 — 사건마다 (승인 경계, 승인 발급기, 적힌 실패 시도 수). 사건 k 는 P<k-1> → P<k> 다.
    /// 마지막 사건만 보관 대기이고 앞의 범위 변경은 기준점 없이 대체된 사건이다. 모든 `world_before_ref` 는 A1 이다.
    fn with(label: &str, texts: &[&[u8]], changes: &[(u32, u32, usize)], closed_interviews: u32) -> Lab {
        let root = fresh_root(label);
        fs::create_dir(root.join(".gil")).expect("새 루트 안에 .gil 을 만든다");
        write(&root, "src/a.txt", b"alpha");
        write(&root, "src/b.txt", b"beta");
        write(&root, "data/raw.csv", b"1,2,3");
        write(&root, "notes/todo.md", b"todo");
        let store = ObjectStore::at(&root.join(".gil"));

        // 승인 원문을 기존 보관 경계(3m)로 창고에 둔다 — 재시도는 원장의 주소로 이 원문을 다시 읽는다.
        for text in texts {
            let address = digest_of(*text).expect("메모리의 바이트는 언제나 읽힌다");
            store_policy_source(&store, EngineVersion::V1, &address, text).expect("시험 원문은 해석되고 보관된다");
        }
        // 마지막 확정 세계 A1 — 이 폴더를 P1 로 실제 보관한다.
        let first = ApprovedPolicy::compile(EngineVersion::V1, texts[0]).expect("시험 원문은 해석된다");
        let world = capture_scoped(&root, &first, &store).expect("조용한 시험 폴더는 관측된다");
        let mut registry = SnapshotRegistry::new();
        let named = registry.intern(store.put_manifest(&world.manifest).expect("manifest 를 둔다")).expect("이름을 준다");
        assert_eq!(named, a(1));

        let count = u32::try_from(texts.len()).unwrap();
        assert_eq!(changes.len() + 1, texts.len(), "판마다 그것을 도입한 사건이 하나다");
        let ledger = PolicyLedger::from_records(
            count + 1,
            texts.iter().zip(1..).map(|(text, number)| record(number, text)).collect(),
        )
        .expect("시험 원장은 꼴이 맞다");
        let mut links = vec![EventPolicyLink::new(m(1), Start, None, Some(p(1)))];
        let mut facts = vec![EventCycleFacts::new(
            BaselineFacts::new(m(1), Start, Some(a(1)), Written(None), Written(1), vec![Archived]),
            ch(1),
            Written(None),
            1,
        )];
        let mut worlds = vec![EventWorldFacts::new(m(1), Start, Written(None), Written(None))];
        for (event, (at, issuer, failed)) in (2..).zip(changes.iter().copied()) {
            links.push(EventPolicyLink::new(m(event), ScopeChange, Some(p(event - 1)), Some(p(event))));
            facts.push(waiting(event, at, issuer, failed));
            worlds.push(EventWorldFacts::new(m(event), ScopeChange, Written(Some(c(at))), Written(Some(a(1)))));
        }
        let records = OwnedRecords {
            ledger,
            links: EventPolicyLinks::from_links(count + 1, links).expect("시험 사건 이름은 꼴이 맞다"),
            facts,
            worlds,
            chains: interviews(closed_interviews),
            registry,
            marks: ScopeMarks::not_provided(),
        };
        Lab { root, store, records }
    }

    /// 그림의 기본 — P1·P2, M2 는 C1 에서 승인(발급기 2)했고 실패 시도 `failed` 개가 적혀 있다. 서 있는 경계는 C1 이다.
    fn new(label: &str, failed: usize) -> Lab {
        Lab::with(label, &[P1_TEXT, P2_TEXT], &[(1, 2, failed)], 1)
    }

    fn env(&self) -> RetryEnv<'_> {
        RetryEnv { root: &self.root, store: &self.store }
    }

    fn desk(&self, save: Save) -> Desk {
        Desk { records: self.records.clone(), save, reread_fails: false, record_fails: false, saves: 0, rereads: 0, failures: 0 }
    }

    /// 저장 경계가 지금 지닌 기록을 입력으로 명시 재시도 하나를 끝까지 실행한다.
    fn retry(&self, event: u32, desk: &mut Desk) -> Result<RetryRun, NotStarted> {
        let given = desk.records.clone();
        run_retry(m(event), given.inputs(), &self.env(), desk)
    }

    fn has_blob(&self, bytes: &[u8]) -> bool {
        let address = digest_of(bytes).expect("메모리의 바이트는 언제나 읽힌다");
        self.store.read_blob(&address, &mut Vec::new()).is_ok()
    }

    fn file(&self, path: &str) -> Vec<u8> {
        fs::read(self.root.join(path)).unwrap_or_else(|err| panic!("{path} 를 읽지 못했다: {err}"))
    }
}

// ── 시험용 저장 경계 ───────────────────────────────────────────────────────

/// 저장 요청에 저장 경계가 하는 일 — 시험이 고른다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Save {
    /// 저장하고 「저장했다」고 답한다.
    Store,
    /// 저장하지 않고 「저장하지 않았다」고 답한다.
    Refuse,
    /// 저장하고 「불확실하다」고 답한다 — 저장 뒤 오류 보고.
    StoreButSayUncertain,
    /// 저장하지 않고 「불확실하다」고 답한다.
    UncertainWithoutStoring,
    /// 저장하지 않고 「저장했다」고 답한다.
    SayStoredWithoutStoring,
    /// 실패 시도 하나를 끼워 넣고 저장한다 — 이 시도의 저장 단위와 다른 기록.
    StoreWithAnExtraFailure,
    /// 앞선 시도 기록을 지우고 저장한다 — 이력을 보존하지 않는 저장.
    StoreButForgetAttempts,
}

/// 메모리의 기록을 고쳐 쓰는 시험용 저장 경계. 호출 횟수를 센다.
struct Desk {
    records: OwnedRecords,
    save: Save,
    reread_fails: bool,
    record_fails: bool,
    saves: usize,
    rereads: usize,
    failures: usize,
}

impl Desk {
    fn calls(&self) -> (usize, usize, usize) {
        (self.saves, self.rereads, self.failures)
    }

    fn slot(event: EventName) -> usize {
        usize::try_from(event.number()).unwrap() - 1
    }

    fn attempts(&self, event: u32) -> Vec<AttemptSeen> {
        self.records.facts[Desk::slot(m(event))].baseline().attempts().to_vec()
    }

    /// 지금 기록이 저장 단위의 결속과 맞는가 — 대상 사건이 아직 기준점 없이, 시도가 묶인 때의 시도 수 그대로인가.
    fn fits(&self, binding: &AttemptBinding) -> bool {
        let baseline = self.records.facts[Desk::slot(binding.event())].baseline();
        baseline.snapshot().is_none() && baseline.attempts().len() == binding.attempts_before()
    }

    /// 한 저장 단위를 한 번에 적는다 — 세계 이름, `baseline` 의 경계·발급기·`snapshot_ref`, `archived` 시도.
    fn store(&mut self, commit: &BaselineCommit, extra_failure: bool, forget: bool) {
        if commit.issues_name() {
            let named = self.records.registry.intern(commit.manifest().clone()).expect("이름을 준다");
            assert_eq!(named, commit.snapshot(), "저장 단위의 세계 이름은 등록부의 다음 이름이다");
        }
        let binding = commit.binding();
        let at = Desk::slot(binding.event());
        let old = self.records.facts[at].clone();
        let mut attempts = if forget { Vec::new() } else { old.baseline().attempts().to_vec() };
        if extra_failure {
            attempts.push(NotArchived);
        }
        attempts.push(Archived);
        let baseline = BaselineFacts::new(
            binding.event(),
            ScopeChange,
            Some(commit.snapshot()),
            Written(Some(binding.boundary())),
            Written(binding.issuer()),
            attempts,
        );
        self.records.facts[at] = EventCycleFacts::new(baseline, old.chain(), old.boundary(), old.issuer());
    }
}

impl BaselineState for Desk {
    fn save_baseline(&mut self, commit: &BaselineCommit) -> SaveAnswer {
        self.saves += 1;
        let uncertain = || SaveAnswer::Uncertain { said: "시험이 넣은 저장 뒤 오류".to_string() };
        match self.save {
            // 결속이 지금 기록과 맞지 않는 저장 단위는 적지 않는다 — 저장 경계의 일이다.
            Save::Store if !self.fits(commit.binding()) => {
                SaveAnswer::NotStored { said: "저장 단위의 결속이 지금 기록과 다르다".to_string() }
            }
            Save::Store => {
                self.store(commit, false, false);
                SaveAnswer::Stored
            }
            Save::Refuse => SaveAnswer::NotStored { said: "시험이 넣은 저장 실패".to_string() },
            Save::StoreButSayUncertain => {
                self.store(commit, false, false);
                uncertain()
            }
            Save::UncertainWithoutStoring => uncertain(),
            Save::SayStoredWithoutStoring => SaveAnswer::Stored,
            Save::StoreWithAnExtraFailure => {
                self.store(commit, true, false);
                SaveAnswer::Stored
            }
            Save::StoreButForgetAttempts => {
                self.store(commit, false, true);
                SaveAnswer::Stored
            }
        }
    }

    fn reread(&mut self) -> Result<OwnedRecords, String> {
        self.rereads += 1;
        match self.reread_fails {
            true => Err("시험이 넣은 재조회 실패".to_string()),
            false => Ok(self.records.clone()),
        }
    }

    fn record_failure(&mut self, failure: &FailedAttempt) -> FailureRecord {
        self.failures += 1;
        if self.record_fails {
            return FailureRecord::NotRecorded { said: "시험이 넣은 실패 기록 저장 실패".to_string() };
        }
        if !self.fits(failure.binding()) {
            return FailureRecord::NotRecorded { said: "실패 시도의 결속이 지금 기록과 다르다".to_string() };
        }
        let at = Desk::slot(failure.binding().event());
        let old = self.records.facts[at].clone();
        let mut attempts = old.baseline().attempts().to_vec();
        attempts.push(NotArchived);
        let baseline = BaselineFacts::new(old.event(), ScopeChange, None, Unwritten, Unwritten, attempts);
        self.records.facts[at] = EventCycleFacts::new(baseline, old.chain(), old.boundary(), old.issuer());
        FailureRecord::Recorded
    }
}

fn failed(run: &RetryRun) -> (&FailureCause, &FailureRecord) {
    match &run.outcome {
        RetryOutcome::Failed { cause, record } => (cause, record),
        other => panic!("실패로 끝나야 한다: {other:#?}"),
    }
}

fn differences(cause: &FailureCause) -> (SeenAt, Vec<(String, DifferenceKind)>) {
    match cause {
        FailureCause::UnconfirmedChange { seen_at, differences } => {
            (*seen_at, differences.iter().map(|found| (found.path.as_str().to_string(), found.kind)).collect())
        }
        other => panic!("미확정 변경이어야 한다: {other:#?}"),
    }
}

/// 실패한 뒤에도 적용 정책 P2 · 마지막 확정 세계 A1(P1) · 보관 대기가 그대로다.
fn still_waiting(desk: &Desk, attempts: usize) {
    let reading = read_scope_change(m(2), &desk.records.inputs()).expect("실패 뒤의 기록도 읽힌다");
    assert_eq!(reading.applied().policy, p(2));
    assert_eq!((reading.confirmed().world(), reading.confirmed().policy()), (a(1), p(1)));
    assert_eq!(reading.standing(), EventStanding::Waiting { attempts });
    assert_eq!(desk.records.registry.next_id(), 2, "세계 이름이 발급되지 않았다");
}

// ── 성공 ───────────────────────────────────────────────────────────────────

#[test]
fn an_explicit_retry_observes_archives_saves_and_confirms_only_from_the_reread_record() {
    let lab = Lab::new("confirm", 1);
    let mut desk = lab.desk(Save::Store);
    let (observes, captures) = (probe::observes(), probe::captures());

    let run = lab.retry(2, &mut desk).expect("대기 사건의 재시도는 시작된다");

    // 한 시도는 사건·적용 정책·확정 정책·world_before·지금 경계·발급기·그때의 시도 수에 묶였다.
    let binding = &run.binding;
    assert_eq!((binding.event(), binding.chain(), binding.applied(), binding.confirmed_policy()), (m(2), ch(1), p(2), p(1)));
    assert_eq!((binding.world_before(), binding.boundary(), binding.issuer(), binding.attempts_before()), (a(1), c(1), 2, 1));
    assert_eq!(run.before.standing(), EventStanding::Waiting { attempts: 1 });

    let RetryOutcome::Confirmed { commit, delta, reading } = &run.outcome else {
        panic!("확정이어야 한다: {:#?}", run.outcome);
    };
    // 실제 관측 경로를 한 번씩 지났고, 저장 한 번·다시 읽기 한 번이며 실패 기록은 청하지 않았다.
    assert_eq!((probe::observes() - observes, probe::captures() - captures), (1, 1));
    assert_eq!(desk.calls(), (1, 1, 0));

    // 저장 단위 — 새 세계 이름 A2, 보관 경계 C1, 발급기 2.
    assert_eq!((commit.snapshot(), commit.issues_name()), (a(2), true));
    assert_eq!(desk.records.registry.resolve(a(2)), Some(commit.manifest()));
    // 범위 변화 — 계속 추적한 src/* 둘만 견줬고, notes/* 는 새로 추적하며, data/* 는 견주지도 보관하지도 않았다.
    assert_eq!(delta.compared(), 2);
    assert!(delta.differences().is_empty());
    assert_eq!(delta.newly_tracked(), [path("notes/todo.md")]);
    assert_eq!(delta.not_compared(), [path("data/raw.csv")]);
    let archived = lab.store.read_manifest(commit.manifest()).expect("보관한 manifest 가 창고에 있다");
    let paths: Vec<&str> = archived.paths().map(|path| path.as_str()).collect();
    assert_eq!(paths, ["notes/todo.md", "src/a.txt", "src/b.txt"]);
    assert!(lab.has_blob(b"todo"), "새로 추적하는 경로의 지금 내용이 최초 보관이다");
    assert_eq!(lab.file("data/raw.csv"), b"1,2,3", "범위 밖의 작업 파일을 건드리지 않는다");

    // 다시 읽은 기록 — 기준점 A2(P2), 적용 정책 P2, 앞선 실패 시도 뒤에 archived 하나.
    assert_eq!(
        reading.standing(),
        EventStanding::AlreadyConfirmed { snapshot: a(2), archived_at: c(1), archive_issuer: 2, attempts: 2 }
    );
    assert_eq!((reading.confirmed().world(), reading.confirmed().policy(), reading.applied().policy), (a(2), p(2), p(2)));
    assert_eq!(desk.attempts(2), [NotArchived, Archived]);
    // 기존 승인·정책·사건·기준점은 그대로다.
    assert_eq!(desk.records.ledger, lab.records.ledger);
    assert_eq!(desk.records.links, lab.records.links);
    assert_eq!(desk.records.facts[0], lab.records.facts[0]);
    assert_eq!(desk.records.worlds, lab.records.worlds);
    assert_eq!(desk.records.registry.resolve(a(1)), lab.records.registry.resolve(a(1)));
}

#[test]
fn a_retry_after_interviews_closed_archives_at_the_boundary_stood_on_now() {
    // 승인은 C1 에서(발급기 2), 그 뒤 인터뷰 C2·C3 이 닫혔다. 지금 서 있는 경계는 C3, 발급기는 4 다(§6.3).
    let lab = Lab::with("later-boundary", &[P1_TEXT, P2_TEXT], &[(1, 2, 0)], 3);
    let mut desk = lab.desk(Save::Store);
    let run = lab.retry(2, &mut desk).expect("시작된다");
    assert_eq!((run.binding.boundary(), run.binding.issuer()), (c(3), 4));
    assert_eq!((run.before.confirmed().world(), run.before.confirmed().policy()), (a(1), p(1)));
    let RetryOutcome::Confirmed { reading, .. } = &run.outcome else {
        panic!("확정이어야 한다: {:#?}", run.outcome);
    };
    assert_eq!(
        reading.standing(),
        EventStanding::AlreadyConfirmed { snapshot: a(2), archived_at: c(3), archive_issuer: 4, attempts: 1 }
    );
}

#[test]
fn the_same_tracked_world_reuses_its_name_and_the_event_keeps_its_own_policy() {
    // §5.8 「같은 세계」: P2 는 P1 의 규칙에 이 폴더에 없는 이름 하나만 더 제외한다 — 추적 세계가 A1 과 같다.
    let lab = Lab::with("same-world", &[P1_TEXT, b"notes/\nabsent/\n"], &[(1, 2, 0)], 1);
    let mut desk = lab.desk(Save::Store);
    let run = lab.retry(2, &mut desk).expect("시작된다");
    let RetryOutcome::Confirmed { commit, reading, .. } = &run.outcome else {
        panic!("확정이어야 한다: {:#?}", run.outcome);
    };
    assert_eq!((commit.snapshot(), commit.issues_name()), (a(1), false), "같은 세계는 기존 이름을 다시 가리킨다(§5.5)");
    assert_eq!(desk.records.registry.next_id(), 2);
    // 같은 이름 A1 이지만 사건의 기준점으로서의 확정 정책은 P2 다.
    assert_eq!((reading.confirmed().world(), reading.confirmed().policy()), (a(1), p(2)));
}

// ── 새로 관측한다 ──────────────────────────────────────────────────────────

#[test]
fn every_explicit_retry_observes_anew_and_a_fixed_file_is_read_fresh() {
    let lab = Lab::new("fresh", 0);
    let mut desk = lab.desk(Save::Store);
    let (observes, captures) = (probe::observes(), probe::captures());

    // 계속 추적한 파일이 밖에서 바뀌었다 — 첫 시도는 보관 전 관측에서 멈춘다.
    write(&lab.root, "src/a.txt", b"changed outside");
    let first = lab.retry(2, &mut desk).expect("시작된다");
    let (cause, record) = failed(&first);
    assert_eq!(differences(cause), (SeenAt::Observation, vec![("src/a.txt".to_string(), DifferenceKind::Modified)]));
    assert_eq!(record, &FailureRecord::Recorded);
    assert_eq!((probe::observes() - observes, probe::captures() - captures), (1, 0), "보관으로 가지 않았다");
    assert_eq!(desk.calls(), (0, 0, 1));
    still_waiting(&desk, 1);
    assert!(!lab.has_blob(b"changed outside"), "미확정 변경의 내용을 창고에 두지 않았다");

    // 사용자가 정리했다. 같은 P2 로 다시 시도하면 처음부터 다시 관측한다 — 앞선 시도의 결과를 쓰지 않는다.
    write(&lab.root, "src/a.txt", b"alpha");
    let second = lab.retry(2, &mut desk).expect("시작된다");
    assert_eq!((probe::observes() - observes, probe::captures() - captures), (2, 1));
    let RetryOutcome::Confirmed { reading, .. } = &second.outcome else {
        panic!("확정이어야 한다: {:#?}", second.outcome);
    };
    assert_eq!(second.binding.attempts_before(), 1, "두 번째 시도는 기록을 다시 읽어 앞선 실패 시도를 본다");
    assert_eq!(
        reading.standing(),
        EventStanding::AlreadyConfirmed { snapshot: a(2), archived_at: c(1), archive_issuer: 2, attempts: 2 }
    );
    assert_eq!(desk.attempts(2), [NotArchived, Archived]);
}

#[test]
fn a_change_a_deletion_and_a_new_file_on_a_continuously_tracked_path_are_each_refused() {
    let cases: [(&str, fn(&Path), (&str, DifferenceKind)); 3] = [
        ("modified", |root| write(root, "src/b.txt", b"other"), ("src/b.txt", DifferenceKind::Modified)),
        ("deleted", |root| fs::remove_file(root.join("src/b.txt")).unwrap(), ("src/b.txt", DifferenceKind::Deleted)),
        ("added", |root| write(root, "src/new.txt", b"new"), ("src/new.txt", DifferenceKind::Added)),
    ];
    for (label, disturb, (path, kind)) in cases {
        let lab = Lab::new(label, 0);
        let mut desk = lab.desk(Save::Store);
        disturb(&lab.root);
        let run = lab.retry(2, &mut desk).expect("시작된다");
        let (cause, _) = failed(&run);
        assert_eq!(differences(cause), (SeenAt::Observation, vec![(path.to_string(), kind)]), "{label}");
        assert_eq!(desk.calls(), (0, 0, 1), "{label}: 저장을 청하지 않았다");
        still_waiting(&desk, 1);
    }
}

#[test]
fn paths_outside_the_applied_scope_and_newly_tracked_paths_are_not_compared() {
    // data/ 는 적용 정책 P2 가 제외했다 — 그 안의 변경·삭제·새 파일은 미확정 변경이 아니고, 건드리지도 않는다.
    // notes/ 는 P1 이 제외했다가 P2 가 포함한다 — 비교할 과거 기준이 없고 지금 내용이 최초 보관이다.
    let lab = Lab::new("scope-effect", 0);
    let mut desk = lab.desk(Save::Store);
    write(&lab.root, "data/raw.csv", b"9,9,9");
    write(&lab.root, "data/extra.csv", b"extra");
    write(&lab.root, "notes/todo.md", b"rewritten");
    write(&lab.root, "notes/more.md", b"more");
    let run = lab.retry(2, &mut desk).expect("시작된다");
    let RetryOutcome::Confirmed { commit, delta, .. } = &run.outcome else {
        panic!("확정이어야 한다: {:#?}", run.outcome);
    };
    assert_eq!(delta.newly_tracked(), [path("notes/more.md"), path("notes/todo.md")]);
    assert_eq!(delta.not_compared(), [path("data/raw.csv")]);
    let archived = lab.store.read_manifest(commit.manifest()).expect("보관한 manifest 가 창고에 있다");
    let paths: Vec<&str> = archived.paths().map(|path| path.as_str()).collect();
    assert_eq!(paths, ["notes/more.md", "notes/todo.md", "src/a.txt", "src/b.txt"]);
    assert!(lab.has_blob(b"rewritten") && !lab.has_blob(b"9,9,9") && !lab.has_blob(b"extra"));
    assert_eq!((lab.file("data/raw.csv"), lab.file("data/extra.csv")), (b"9,9,9".to_vec(), b"extra".to_vec()));
}

#[test]
fn a_path_excluded_by_an_intermediate_board_is_newly_tracked_when_included_again() {
    // P1 은 data/ 를 추적했고, 적용됐지만 기준점이 없는 P2 가 제외했고, 적용 정책 P3 이 다시 포함한다(§5.8). M2 는 M3 으로 대체됐다.
    let through = Lab::with("reincluded", &[P1_TEXT, P2_TEXT, b"notes/\n"], &[(1, 2, 0), (1, 2, 0)], 1);
    let mut desk = through.desk(Save::Store);
    write(&through.root, "data/raw.csv", b"changed while excluded");
    let run = through.retry(3, &mut desk).expect("시작된다");
    assert_eq!((run.binding.confirmed_policy(), run.binding.applied()), (p(1), p(3)));
    let RetryOutcome::Confirmed { delta, .. } = &run.outcome else {
        panic!("확정이어야 한다: {:#?}", run.outcome);
    };
    assert_eq!(delta.newly_tracked(), [path("data/raw.csv")], "제외됐다가 다시 포함된 경로는 새로 추적한다");
    assert_eq!(delta.compared(), 2);
    assert!(through.has_blob(b"changed while excluded"));
    // 대체된 M2 는 다시 보관하지 않는다 — 시도를 만들지 않고 아무것도 부르지 않는다.
    let calls = desk.calls();
    assert!(matches!(through.retry(2, &mut desk), Err(NotStarted::Replaced { .. })));
    assert_eq!(desk.calls(), calls);

    // 중간 판이 그 경로를 제외하지 않았다면 같은 변경은 계속 추적한 경로의 미확정 변경이다.
    let straight = Lab::with("kept-tracked", &[P1_TEXT, b"notes/\nnothing-here/\n", b"notes/\n"], &[(1, 2, 0), (1, 2, 0)], 1);
    let mut desk = straight.desk(Save::Store);
    write(&straight.root, "data/raw.csv", b"changed while tracked");
    let run = straight.retry(3, &mut desk).expect("시작된다");
    let (cause, _) = failed(&run);
    assert_eq!(differences(cause), (SeenAt::Observation, vec![("data/raw.csv".to_string(), DifferenceKind::Modified)]));
}

#[test]
fn a_proposal_that_was_not_approved_does_not_narrow_the_comparison() {
    // 다음 제안이 src/ 를 제외하려 한다 — 디스크의 .gilignore 에 적혀 있고 그 원문이 창고에도 있지만 원장의 판이 아니다.
    let lab = Lab::new("proposal", 0);
    let proposal: &[u8] = b"data/\nsrc/\n";
    write(&lab.root, ".gilignore", proposal);
    lab.store.put_blob(proposal).expect("제안 원문을 창고에 둔다");
    write(&lab.root, "src/a.txt", b"hidden by the proposal?");
    let mut desk = lab.desk(Save::Store);
    let run = lab.retry(2, &mut desk).expect("시작된다");
    let (cause, _) = failed(&run);
    assert_eq!(differences(cause), (SeenAt::Observation, vec![("src/a.txt".to_string(), DifferenceKind::Modified)]));
    still_waiting(&desk, 1);
    assert_eq!(lab.file(".gilignore"), proposal, "정책 파일을 읽지도 쓰지도 않았다");
}

// ── 걸음마다: 검사 뒤의 변화 · 불안정 · 관측 불가 ───────────────────────────

/// 기다리는 요청을 실제로 실행하고 그 보고로 한 걸음 나아간다.
fn step(attempt: &mut RetryAttempt<'_>, lab: &Lab, desk: &mut Desk) -> RetryStep {
    let report = attempt.perform(&lab.env(), desk).expect("끝나지 않은 시도다");
    attempt.advance(report).expect("방금 낸 보고다")
}

#[test]
fn a_change_between_the_check_and_the_capture_is_not_confirmed_even_though_objects_remain() {
    let lab = Lab::new("after-check", 0);
    let mut desk = lab.desk(Save::Store);
    let given = desk.records.clone();
    let mut attempt = RetryAttempt::begin(m(2), given.inputs(), &lab.env()).expect("시작된다");

    assert!(matches!(step(&mut attempt, &lab, &mut desk), RetryStep::Next(RetryRequest::Capture { .. })), "앞선 검사는 통과했다");
    // 검사 뒤, 객체 확정 전에 계속 추적한 파일이 바뀐다.
    write(&lab.root, "src/a.txt", b"changed after the check");
    let RetryStep::Next(RetryRequest::RecordFailure { failure, .. }) = step(&mut attempt, &lab, &mut desk) else {
        panic!("capture 의 결과에도 같은 검사를 해야 한다");
    };
    assert_eq!(
        differences(failure.cause()),
        (SeenAt::Capture, vec![("src/a.txt".to_string(), DifferenceKind::Modified)])
    );
    assert!(matches!(step(&mut attempt, &lab, &mut desk), RetryStep::Done(RetryOutcome::Failed { .. })));

    // capture 가 둔 객체는 창고에 남았다. 그것은 성공이 아니다 — 이름도, 기준점도, 저장 요청도 없다.
    assert!(lab.has_blob(b"changed after the check"));
    assert_eq!(desk.calls(), (0, 0, 1));
    still_waiting(&desk, 1);
}

#[test]
fn an_observation_that_moves_between_its_two_scans_is_unstable_not_clean() {
    // ① 보관 전 관측이 흔들린다.
    let lab = Lab::new("unstable-observe", 0);
    let mut desk = lab.desk(Save::Store);
    let root = lab.root.clone();
    probe::between_next_scans(move || write(&root, "notes/todo.md", b"moved between scans"));
    let run = lab.retry(2, &mut desk).expect("시작된다");
    assert_eq!(failed(&run).0, &FailureCause::UnstableObservation { seen_at: SeenAt::Observation });
    assert_eq!(desk.calls(), (0, 0, 1));
    still_waiting(&desk, 1);

    // ② 객체를 확정하는 관측이 흔들린다 — 첫 훑기의 객체가 남아도 성공이 아니다.
    let lab = Lab::new("unstable-capture", 0);
    let mut desk = lab.desk(Save::Store);
    let given = desk.records.clone();
    let mut attempt = RetryAttempt::begin(m(2), given.inputs(), &lab.env()).expect("시작된다");
    assert!(matches!(step(&mut attempt, &lab, &mut desk), RetryStep::Next(RetryRequest::Capture { .. })));
    let root = lab.root.clone();
    probe::between_next_scans(move || write(&root, "notes/todo.md", b"moved during capture"));
    let RetryStep::Next(RetryRequest::RecordFailure { failure, .. }) = step(&mut attempt, &lab, &mut desk) else {
        panic!("흔들린 보관은 실패다");
    };
    assert_eq!(failure.cause(), &FailureCause::UnstableObservation { seen_at: SeenAt::Capture });
    assert!(lab.has_blob(b"todo"), "첫 훑기가 둔 객체는 미참조로 남는다");
    assert!(matches!(step(&mut attempt, &lab, &mut desk), RetryStep::Done(RetryOutcome::Failed { .. })));
    assert_eq!(desk.calls(), (0, 0, 1));
    still_waiting(&desk, 1);
}

#[test]
fn an_observation_that_is_refused_is_not_treated_as_no_change() {
    // 추적 범위 안의 하위 정책 파일은 관측 전체를 막는다.
    let lab = Lab::new("refused", 0);
    let mut desk = lab.desk(Save::Store);
    write(&lab.root, "src/.gilignore", b"*.tmp\n");
    let run = lab.retry(2, &mut desk).expect("시작된다");
    let (cause, record) = failed(&run);
    assert!(matches!(cause, FailureCause::ObservationRefused { seen_at: SeenAt::Observation, .. }), "{cause:#?}");
    assert_eq!(record, &FailureRecord::Recorded);
    assert_eq!(desk.calls(), (0, 0, 1));
    still_waiting(&desk, 1);
}

// ── 저장 결과 ──────────────────────────────────────────────────────────────

#[test]
fn a_confirmed_store_failure_keeps_the_policy_the_confirmed_world_and_the_wait() {
    let lab = Lab::new("not-stored", 0);
    let mut desk = lab.desk(Save::Refuse);
    let run = lab.retry(2, &mut desk).expect("시작된다");
    let (cause, record) = failed(&run);
    assert!(matches!(cause, FailureCause::StateNotStored { .. }), "{cause:#?}");
    assert_eq!(record, &FailureRecord::Recorded);
    assert_eq!(desk.calls(), (1, 0, 1), "저장하지 않았다고 확인됐으므로 다시 읽지 않는다");
    still_waiting(&desk, 1);
    // 객체는 창고에 놓였다. 그래도 기준점은 없다.
    assert!(lab.has_blob(b"todo"));
}

#[test]
fn a_failure_whose_record_cannot_be_stored_is_still_a_failure() {
    let lab = Lab::new("unrecorded", 0);
    let mut desk = lab.desk(Save::Store);
    desk.record_fails = true;
    write(&lab.root, "src/a.txt", b"changed outside");
    let run = lab.retry(2, &mut desk).expect("시작된다");
    let (cause, record) = failed(&run);
    assert!(matches!(cause, FailureCause::UnconfirmedChange { .. }));
    assert!(matches!(record, FailureRecord::NotRecorded { .. }));
    assert_eq!(desk.calls(), (0, 0, 1));
    // 시도 기록이 남지 않았다 — 기록이 없다는 사실이 성공을 뜻하지 않는다. 대기 그대로다.
    still_waiting(&desk, 0);
}

#[test]
fn an_error_reported_after_the_store_is_confirmed_by_the_reread() {
    let lab = Lab::new("stored-then-error", 0);
    let mut desk = lab.desk(Save::StoreButSayUncertain);
    let run = lab.retry(2, &mut desk).expect("시작된다");
    assert!(matches!(run.outcome, RetryOutcome::Confirmed { .. }), "{:#?}", run.outcome);
    assert_eq!(desk.calls(), (1, 1, 0));
}

#[test]
fn an_uncertain_store_that_cannot_be_reread_is_unknown_neither_success_nor_a_settled_wait() {
    for save in [Save::StoreButSayUncertain, Save::Store] {
        let lab = Lab::new("unknown", 0);
        let mut desk = lab.desk(save);
        desk.reread_fails = true;
        let run = lab.retry(2, &mut desk).expect("시작된다");
        let RetryOutcome::Unknown { commit, why, .. } = &run.outcome else {
            panic!("결과 불명이어야 한다: {:#?}", run.outcome);
        };
        assert!(matches!(why, UnknownWhy::RereadUnavailable { .. }), "{why:#?}");
        assert_eq!(commit.snapshot(), a(2));
        assert_eq!(desk.calls(), (1, 1, 0), "결과를 모르므로 실패 시도를 적지 않는다");
    }
}

#[test]
fn an_uncertain_answer_followed_by_a_reread_of_the_unchanged_wait_is_a_confirmed_failure() {
    let lab = Lab::new("uncertain-not-stored", 0);
    let mut desk = lab.desk(Save::UncertainWithoutStoring);
    let run = lab.retry(2, &mut desk).expect("시작된다");
    let (cause, record) = failed(&run);
    assert!(matches!(cause, FailureCause::StateNotStored { .. }), "{cause:#?}");
    assert_eq!(record, &FailureRecord::Recorded);
    assert_eq!(desk.calls(), (1, 1, 1));
    still_waiting(&desk, 1);
}

#[test]
fn a_stored_answer_alone_is_not_a_confirmation() {
    // 저장 경계가 저장했다고 답했지만 다시 읽은 기록은 앞선 대기 그대로다.
    let lab = Lab::new("said-stored", 0);
    let mut desk = lab.desk(Save::SayStoredWithoutStoring);
    let run = lab.retry(2, &mut desk).expect("시작된다");
    assert!(
        matches!(&run.outcome, RetryOutcome::Unknown { why: UnknownWhy::StoredButStillWaiting, answer: SaveAnswer::Stored, .. }),
        "{:#?}",
        run.outcome
    );
    assert_eq!(desk.calls(), (1, 1, 0));
}

#[test]
fn a_reread_record_that_is_not_this_attempts_unit_or_lost_history_is_not_a_confirmation() {
    // 다시 읽은 기록의 시도 수가 이 시도의 저장 단위와 다르다.
    let lab = Lab::new("other-unit", 0);
    let mut desk = lab.desk(Save::StoreWithAnExtraFailure);
    let run = lab.retry(2, &mut desk).expect("시작된다");
    assert!(
        matches!(
            &run.outcome,
            RetryOutcome::Unknown { why: UnknownWhy::RereadDiffers { found: EventStanding::AlreadyConfirmed { attempts: 2, .. } }, .. }
        ),
        "{:#?}",
        run.outcome
    );

    // 다시 읽은 기록에서 앞선 실패 시도가 사라졌다 — 이력을 보존하지 않는 저장은 확정으로 보고하지 않는다.
    let lab = Lab::new("lost-history", 1);
    let mut desk = lab.desk(Save::StoreButForgetAttempts);
    let run = lab.retry(2, &mut desk).expect("시작된다");
    assert!(
        matches!(&run.outcome, RetryOutcome::Unknown { why: UnknownWhy::HistoryNotKept { .. }, .. }),
        "{:#?}",
        run.outcome
    );
}

// ── 시도를 만들지 않는 요청 ────────────────────────────────────────────────

#[test]
fn a_repeated_request_on_a_confirmed_event_makes_no_attempt_and_no_second_confirmation() {
    let lab = Lab::new("repeat", 0);
    let mut desk = lab.desk(Save::Store);
    let first = lab.retry(2, &mut desk).expect("시작된다");
    assert!(matches!(first.outcome, RetryOutcome::Confirmed { .. }));
    let (calls, observes, captures) = (desk.calls(), probe::observes(), probe::captures());
    let stored = desk.records.facts.clone();

    for _ in 0..2 {
        let Err(NotStarted::AlreadyConfirmed { reading }) = lab.retry(2, &mut desk) else {
            panic!("이미 확정된 사건에는 시도를 만들지 않는다");
        };
        assert_eq!(
            reading.standing(),
            EventStanding::AlreadyConfirmed { snapshot: a(2), archived_at: c(1), archive_issuer: 2, attempts: 1 }
        );
    }
    assert_eq!((desk.calls(), probe::observes(), probe::captures()), (calls, observes, captures), "아무것도 다시 부르지 않았다");
    assert_eq!(desk.records.facts, stored);
    assert_eq!(desk.records.registry.next_id(), 3, "세계 이름이 다시 발급되지 않았다");
}

#[test]
fn queries_rebuilt_readings_and_out_of_scope_requests_call_no_observation_and_no_state() {
    let lab = Lab::new("queries", 1);
    let mut desk = lab.desk(Save::Store);
    let (observes, captures) = (probe::observes(), probe::captures());

    // 조회 — 몇 번을 읽어도, 같은 기록을 새로 복사해 읽어도(새 판정 인스턴스) 같은 읽기이고 아무것도 실행되지 않는다.
    let first = read_scope_change(m(2), &desk.records.inputs()).expect("읽힌다");
    for _ in 0..3 {
        let copied = desk.records.clone();
        assert_eq!(read_scope_change(m(2), &copied.inputs()).expect("읽힌다"), first);
    }
    assert_eq!(first.standing(), EventStanding::Waiting { attempts: 1 });

    // 시작만 하고 실행하지 않은 시도 — 창고에서 원문과 기준 manifest 를 읽었을 뿐 관측도 저장 요청도 없다.
    let given = desk.records.clone();
    let idle = RetryAttempt::begin(m(2), given.inputs(), &lab.env()).expect("시작된다");
    assert!(matches!(idle.pending(), Some(RetryRequest::Observe { .. })));
    assert_eq!(idle.outcome(), None);
    drop(idle);

    // 보관 중단 표식이 있다고 건넨 기록, 시작 사건, 없는 사건 — 읽기가 돌려준 갈래 그대로이며 시도를 만들지 않는다.
    let mut halted = desk.records.clone();
    halted.marks = ScopeMarks { archive_halt: MarkSeen::Present, unfinished_restore: MarkSeen::Absent };
    assert_eq!(
        run_retry(m(2), halted.inputs(), &lab.env(), &mut desk).unwrap_err(),
        NotStarted::Reading(vec![ReadingProblem::OutOfScope(ScopeLimit::ArchiveHaltPresent)])
    );
    assert!(matches!(lab.retry(1, &mut desk), Err(NotStarted::Reading(_))));
    assert!(matches!(lab.retry(9, &mut desk), Err(NotStarted::Reading(_))));

    assert_eq!((probe::observes() - observes, probe::captures() - captures), (0, 0));
    assert_eq!(desk.calls(), (0, 0, 0));
    assert_eq!(desk.records.facts, lab.records.facts);
}

#[test]
fn a_retry_whose_policy_source_or_base_world_cannot_be_read_does_not_start() {
    // 적용 정책 P2 의 원문이 창고에 없다 — 원장의 주소만 있고 객체가 없다.
    let mut lab = Lab::new("no-source", 0);
    let absent: &[u8] = b"never-stored/\n";
    lab.records.ledger =
        PolicyLedger::from_records(3, vec![record(1, P1_TEXT), record(2, absent)]).expect("시험 원장은 꼴이 맞다");
    let mut desk = lab.desk(Save::Store);
    let (observes, captures) = (probe::observes(), probe::captures());
    let Err(NotStarted::PolicySource { policy, .. }) = lab.retry(2, &mut desk) else {
        panic!("원문을 확인하지 못하면 시작하지 않는다");
    };
    assert_eq!(policy, p(2));

    // 마지막 확정 세계의 이름은 등록부에 있지만 그 manifest 객체가 창고에 없다.
    let mut lab = Lab::new("no-base", 0);
    let nowhere = crate::ManifestAddress::from_parts("sha256", &[7u8; 32]).expect("꼴이 맞는 주소다");
    lab.records.registry = SnapshotRegistry::restore(2, vec![(1, nowhere)]).expect("시험 등록부는 꼴이 맞다");
    let mut other = lab.desk(Save::Store);
    let Err(NotStarted::BaseWorld { world, .. }) = lab.retry(2, &mut other) else {
        panic!("기준 manifest 를 읽지 못하면 시작하지 않는다");
    };
    assert_eq!(world, a(1));

    assert_eq!((probe::observes() - observes, probe::captures() - captures), (0, 0));
    assert_eq!((desk.calls(), other.calls()), ((0, 0, 0), (0, 0, 0)));
}

// ── 보고의 결속 ────────────────────────────────────────────────────────────

#[test]
fn reports_of_another_attempt_another_step_or_a_closed_attempt_are_refused_and_change_nothing() {
    let lab = Lab::new("tickets", 0);
    let mut desk = lab.desk(Save::Store);
    let given = desk.records.clone();
    let mut first = RetryAttempt::begin(m(2), given.inputs(), &lab.env()).expect("시작된다");
    let mut second = RetryAttempt::begin(m(2), given.inputs(), &lab.env()).expect("같은 입력으로 시작한 다른 시도다");

    let Some(RetryRequest::Observe { ticket: first_ticket }) = first.pending() else { panic!("첫 요청은 관측이다") };
    let Some(RetryRequest::Observe { ticket: second_ticket }) = second.pending() else { panic!("첫 요청은 관측이다") };
    assert_ne!(first_ticket, second_ticket, "시도마다 표가 다르다");

    // 첫 시도의 관측 보고 — 실제 관측의 결과다. 같은 걸음을 다시 실행하라는 요청은 거절되고 관측도 다시 일어나지 않는다.
    let observes = probe::observes();
    let observed = first.perform(&lab.env(), &mut desk).expect("끝나지 않은 시도다");
    assert_eq!(observed.ticket(), first_ticket);
    assert_eq!(
        first.perform(&lab.env(), &mut desk).err(),
        Some(RetryClosed::AlreadyPerformed { ticket: first_ticket })
    );
    assert_eq!(probe::observes() - observes, 1);

    // 다른 시도에 건네면 거절되고 그 시도는 그대로 관측을 기다린다 — 앞선 시도의 관측으로 새 관측을 생략할 수 없다.
    assert_eq!(
        second.advance(observed.clone()),
        Err(RetryOutOfOrder::OtherAttempt { awaiting: second_ticket, got: first_ticket })
    );
    assert_eq!(second.pending(), Some(RetryRequest::Observe { ticket: second_ticket }));

    // 제 시도에는 받아들여진다. 같은 보고를 다시 넣으면 지난 걸음의 보고라 거절된다.
    let RetryStep::Next(RetryRequest::Capture { ticket: capture_ticket }) = first.advance(observed.clone()).expect("제 보고다") else {
        panic!("관측 뒤에는 보관이다");
    };
    assert_eq!(
        first.advance(observed.clone()),
        Err(RetryOutOfOrder::OtherStep { awaiting: capture_ticket, got: first_ticket })
    );
    assert_eq!(first.pending(), Some(RetryRequest::Capture { ticket: capture_ticket }));

    // 끝까지 간 뒤에는 어떤 보고도 받지 않고, 실행할 요청도 없다.
    let mut last = None;
    while first.outcome().is_none() {
        let report = first.perform(&lab.env(), &mut desk).expect("끝나지 않은 시도다");
        last = Some(report.clone());
        first.advance(report).expect("방금 낸 보고다");
    }
    let outcome = first.outcome().cloned().expect("끝났다");
    assert!(matches!(outcome, RetryOutcome::Confirmed { .. }), "{outcome:#?}");
    let last = last.expect("마지막 보고가 있다");
    assert_eq!(first.advance(last.clone()), Err(RetryOutOfOrder::AlreadyClosed { got: last.ticket() }));
    assert_eq!(first.advance(observed), Err(RetryOutOfOrder::AlreadyClosed { got: first_ticket }));
    assert_eq!(first.perform(&lab.env(), &mut desk).err(), Some(RetryClosed::Finished));
    assert_eq!(first.outcome(), Some(&outcome), "끝난 시도의 결과는 바뀌지 않는다");
    assert_eq!(first.pending(), None);

    // 첫 시도가 확정한 뒤, 같은 옛 입력으로 시작해 둔 두 번째 시도를 끝까지 실행해도 중복 확정이 되지 않는다. 저장 경계는 결속이
    // 지금 기록과 다른 저장 단위를 적지 않았고, 기준점·시도 이력은 첫 시도가 남긴 그대로다.
    let (saves, stored) = (desk.saves, desk.records.facts.clone());
    while second.outcome().is_none() {
        let report = second.perform(&lab.env(), &mut desk).expect("끝나지 않은 시도다");
        second.advance(report).expect("방금 낸 보고다");
    }
    assert!(
        matches!(second.outcome(), Some(RetryOutcome::Failed { cause: FailureCause::StateNotStored { .. }, .. })),
        "{:#?}",
        second.outcome()
    );
    assert_eq!(desk.saves, saves + 1);
    assert_eq!(desk.records.facts, stored);
    assert_eq!(desk.attempts(2), [Archived]);
}
