//! 보관 대기 사건의 명시 재시도(`RetryAttempt`·`run_retry`) 시험 — **실제 임시 폴더와 실제 객체 창고**, 그리고 **시험용 저장 경계**를 쓴다.
//!
//! - 폴더·객체: 시험마다 배타적으로 새로 만든 임시 루트에 합성 파일을 두고, 기존 `capture_scoped`·`observe_scoped`·`ObjectStore` 를
//!   그대로 지난다. 마지막 확정 세계 A1 은 그 폴더를 P1 로 실제 보관한 manifest 다. 끝나면 그 루트만 지운다.
//! - 기록: 정책 판·사건·보충 칸·시도 결과는 시험이 만든 값이다. 모니터가 승인한 기록이 아니고 `state.yaml` 도 `gil start` 도 쓰지 않는다.
//!   승인 저장(저장 1)은 「이미 저장됐다고 건넨 입력」일 뿐이다.
//! - 저장 경계(`Desk`): 메모리의 기록을 고쳐 쓰는 **시험용 어댑터**다. 그 실패 주입은 실제 저장의 원자성·프로세스 강제 종료·전원 장애
//!   내구성의 증거가 아니다. 다시 읽기도 그 메모리 값의 복사다. 보관 중단 표식·미완 복원 흔적이 「없다」는 것도 시험이 건넨 말이다.
//! - 「새로 만든 값으로 다시 읽는다」는 순수 판정의 반복이며 제품 재시작 시험이 아니다. 실제 Host·사람의 승인·필드 인수의 증거도 아니다.
//!
//! ```text
//! 폴더            src/a.txt  src/b.txt  data/raw.csv  notes/todo.md   (+ 루트 .gil/ 창고)
//! P1  "notes/"    data/ 와 src/ 를 추적          A1 = P1 로 보관한 { data/raw.csv, src/a.txt, src/b.txt }
//! P2  "data/"     notes/ 와 src/ 를 추적         계속 추적 = src/*   ·  새로 추적 = notes/*  ·  범위 밖 = data/*
//! 체인            chain:C1  cycle:C1 Interview(뿌리, 닫힘) [└ cycle:C2 Interview(닫힘) …]   모든 Entry·Exit A1
//! 사건            M1 start(없음→P1, 기준점 A1)   M2 scope_change(P1→P2, C1 에서 승인, 발급기 2, world_before A1, 기준점 없음)
//! ```

use std::cell::RefCell;
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;

use super::baseline_retry::{BindingLost, BindingPart};
use super::event_cycles_tests::{a, c, ch, close_interview, m, open_interview, rules, x1};
use super::experiment_exit_tests::{at, p, restored};
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
    /// 그 루트의 `.gil/` 창고 — 재시도가 스스로 정하는 창고와 같은 자리다. 시험이 자료를 놓고 결과를 들여다보는 데만 쓴다.
    store: ObjectStore,
    records: OwnedRecords,
    /// 시험이 루트 옆에 더 만든 폴더 — 끝날 때 함께 지운다.
    beside: RefCell<Vec<PathBuf>>,
}

impl Drop for Lab {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
        for extra in self.beside.borrow().iter() {
            let _ = fs::remove_dir_all(extra);
        }
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
            // 실행 전제 — 보관 중단 표식과 미완 복원 흔적이 「없다」고 건넨다(시험이 건넨 말이다).
            marks: ScopeMarks { archive_halt: MarkSeen::Absent, unfinished_restore: MarkSeen::Absent },
        };
        Lab { root, store, records, beside: RefCell::new(Vec::new()) }
    }

    /// 그림의 기본 — P1·P2, M2 는 C1 에서 승인(발급기 2)했고 실패 시도 `failed` 개가 적혀 있다. 서 있는 경계는 C1 이다.
    fn new(label: &str, failed: usize) -> Lab {
        Lab::with(label, &[P1_TEXT, P2_TEXT], &[(1, 2, failed)], 1)
    }

    fn desk(&self, save: Save) -> Desk {
        Desk(Rc::new(RefCell::new(Held {
            records: self.records.clone(),
            save,
            reread_fails: false,
            reread_without_marks: false,
            record_fails: false,
            saves: 0,
            rereads: 0,
            failures: 0,
        })))
    }

    /// 저장 경계가 지금 지닌 기록을 입력으로 명시 재시도 하나를 끝까지 실행한다. 대상은 이 루트와 이 저장 경계다.
    fn retry(&self, event: u32, desk: &Desk) -> Result<RetryRun, NotStarted> {
        let given = desk.records();
        let mut boundary = desk.clone();
        run_retry(m(event), given.inputs(), &self.root, &mut boundary)
    }

    /// 루트 옆의 새 이름 — 시험이 폴더를 옮겨 둘 자리. 끝날 때 지운다.
    fn beside(&self, label: &str) -> PathBuf {
        let mut name = self.root.file_name().expect("임시 루트에는 이름이 있다").to_os_string();
        name.push(format!("-{label}"));
        let path = self.root.with_file_name(name);
        self.beside.borrow_mut().push(path.clone());
        path
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
    /// 결속이 지금 기록과 맞으면 저장하고 「저장했다」고, 맞지 않으면 저장하지 않고 「저장하지 않았다」고 답한다.
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
    /// 저장 단위는 그대로 적되, 유도 경로 위 인터뷰 C2 의 저장 Exit 을 등록부에 없는 A9 로 바꾼다 — 사건 칸·기준점·시도·등록부는 그대로다.
    StoreAndBendAnAncestor,
}

struct Held {
    records: OwnedRecords,
    save: Save,
    reread_fails: bool,
    /// 다시 읽기가 보관 중단 표식·미완 복원 흔적을 건네지 않는다.
    reread_without_marks: bool,
    record_fails: bool,
    saves: usize,
    rereads: usize,
    failures: usize,
}

/// 메모리의 기록을 고쳐 쓰는 시험용 저장 경계. 호출 횟수를 센다. 복제한 값은 **같은** 기록을 가리킨다 — 시도가 하나를 빌려 쓰는 동안
/// 시험이 다른 하나로 들여다본다.
#[derive(Clone)]
struct Desk(Rc<RefCell<Held>>);

impl Desk {
    /// (저장 요청, 다시 읽기, 실패 기록 요청)의 횟수.
    fn calls(&self) -> (usize, usize, usize) {
        let held = self.0.borrow();
        (held.saves, held.rereads, held.failures)
    }

    /// 지금 지닌 기록의 복사.
    fn records(&self) -> OwnedRecords {
        self.0.borrow().records.clone()
    }

    fn attempts(&self, event: u32) -> Vec<AttemptSeen> {
        self.0.borrow().records.facts[Held::slot(m(event))].baseline().attempts().to_vec()
    }

    fn set(&self, change: impl FnOnce(&mut Held)) {
        change(&mut self.0.borrow_mut());
    }
}

impl Held {
    fn slot(event: EventName) -> usize {
        usize::try_from(event.number()).unwrap() - 1
    }

    /// **쓰는 시점에** 지금 기록이 그 결속 전체 그대로의 보관 대기인가 — 기준점 없음과 시도 수만이 아니라 경계·발급기·적용 정책·마지막
    /// 확정 세계와 확정 정책·그 근거까지. 시도가 실패 시도 기록을 청하기 전에 하는 판정과 같은 [`AttemptBinding::holds_in`] 이다.
    fn fits(&self, binding: &AttemptBinding) -> bool {
        binding.holds_in(&self.records.inputs()).is_ok()
    }

    /// 한 저장 단위를 한 번에 적는다 — 세계 이름, `baseline` 의 경계·발급기·`snapshot_ref`, `archived` 시도.
    fn store(&mut self, commit: &BaselineCommit, extra_failure: bool, forget: bool) {
        if commit.issues_name() {
            let named = self.records.registry.intern(commit.manifest().clone()).expect("이름을 준다");
            assert_eq!(named, commit.snapshot(), "저장 단위의 세계 이름은 등록부의 다음 이름이다");
        }
        let binding = commit.binding();
        let at = Held::slot(binding.event());
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
        let mut held = self.0.borrow_mut();
        held.saves += 1;
        let uncertain = || SaveAnswer::Uncertain { said: "시험이 넣은 저장 뒤 오류".to_string() };
        match held.save {
            // 결속이 지금 기록과 맞지 않는 저장 단위는 적지 않는다 — 저장 경계의 일이다.
            Save::Store if !held.fits(commit.binding()) => {
                SaveAnswer::NotStored { said: "저장 단위의 결속이 지금 기록과 다르다".to_string() }
            }
            Save::Store => {
                held.store(commit, false, false);
                SaveAnswer::Stored
            }
            Save::Refuse => SaveAnswer::NotStored { said: "시험이 넣은 저장 실패".to_string() },
            Save::StoreButSayUncertain => {
                held.store(commit, false, false);
                uncertain()
            }
            Save::UncertainWithoutStoring => uncertain(),
            Save::SayStoredWithoutStoring => SaveAnswer::Stored,
            Save::StoreWithAnExtraFailure => {
                held.store(commit, true, false);
                SaveAnswer::Stored
            }
            Save::StoreButForgetAttempts => {
                held.store(commit, false, true);
                SaveAnswer::Stored
            }
            Save::StoreAndBendAnAncestor => {
                held.store(commit, false, false);
                let bent = restored(&held.records.chains[0], |nodes| nodes[at(2)].exit = Some(a(9)))
                    .expect("Cycles::restore 가 받아들인다");
                held.records.chains[0] = bent;
                SaveAnswer::Stored
            }
        }
    }

    fn reread(&mut self) -> Result<OwnedRecords, String> {
        let mut held = self.0.borrow_mut();
        held.rereads += 1;
        if held.reread_fails {
            return Err("시험이 넣은 재조회 실패".to_string());
        }
        let mut records = held.records.clone();
        if held.reread_without_marks {
            records.marks = ScopeMarks::not_provided();
        }
        Ok(records)
    }

    fn record_failure(&mut self, failure: &FailedAttempt) -> FailureRecord {
        let mut held = self.0.borrow_mut();
        held.failures += 1;
        if held.record_fails {
            return FailureRecord::NotRecorded { said: "시험이 넣은 실패 기록 저장 실패".to_string() };
        }
        if !held.fits(failure.binding()) {
            return FailureRecord::NotRecorded { said: "실패 시도의 결속이 지금 기록과 다르다".to_string() };
        }
        let at = Held::slot(failure.binding().event());
        let old = held.records.facts[at].clone();
        let mut attempts = old.baseline().attempts().to_vec();
        attempts.push(NotArchived);
        let baseline = BaselineFacts::new(old.event(), ScopeChange, None, Unwritten, Unwritten, attempts);
        held.records.facts[at] = EventCycleFacts::new(baseline, old.chain(), old.boundary(), old.issuer());
        FailureRecord::Recorded
    }
}

fn failed(run: &RetryRun) -> (&FailureCause, &AfterFailure) {
    match &run.outcome {
        RetryOutcome::Failed { cause, now } => (cause, now),
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

/// 실패한 시도가 **다시 읽은 기록에서** 확인한 것: 적용 정책 P2 · 마지막 확정 세계 A1(확정 정책 P1) · 그 결속의 보관 대기. 그 뒤 실패 시도
/// 기록을 청했고 그 답이 `record` 다. 세계 이름도 발급되지 않았다.
fn still_waiting<'a>(now: &'a AfterFailure, desk: &Desk, attempts_at_start: usize) -> &'a FailureRecord {
    let AfterFailure::StillWaiting { reading, record } = now else {
        panic!("다시 읽은 기록에서 그 결속의 대기를 확인해야 한다: {now:#?}");
    };
    assert_eq!(reading.applied().policy, p(2), "보관 실패가 적용 정책을 P1 로 되돌리지 않는다");
    assert_eq!((reading.confirmed().world(), reading.confirmed().policy()), (a(1), p(1)), "마지막 확정 세계는 A1, 확정 정책은 P1 이다");
    assert_eq!(reading.standing(), EventStanding::Waiting { attempts: attempts_at_start });
    let stored = desk.records();
    assert_eq!(stored.registry.next_id(), 2, "세계 이름이 발급되지 않았다");
    assert_eq!(stored.ledger.policies().len(), 2, "정책 원장은 그대로다");
    record
}

// ── 성공 ───────────────────────────────────────────────────────────────────

#[test]
fn an_explicit_retry_observes_archives_saves_and_confirms_only_from_the_reread_record() {
    let lab = Lab::new("confirm", 1);
    let desk = lab.desk(Save::Store);
    let (observes, captures) = (probe::observes(), probe::captures());

    let run = lab.retry(2, &desk).expect("실행 전제가 확인된 대기 사건의 재시도는 시작된다");

    // 한 시도는 사건·적용 정책·확정 정책·world_before·지금 경계·발급기·그때의 시도 수에 묶였다.
    let binding = &run.binding;
    assert_eq!((binding.event(), binding.chain(), binding.applied(), binding.confirmed_policy()), (m(2), ch(1), p(2), p(1)));
    assert_eq!((binding.world_before(), binding.boundary(), binding.issuer(), binding.attempts_before()), (a(1), c(1), 2, 1));
    assert_eq!(run.before.standing(), EventStanding::Waiting { attempts: 1 });
    // 대기 사건에 아직 없는 기준점 칸은 판단하지 않은 것으로 남지만 실행 전제의 미확인이 아니다 — 그것만 남은 읽기에서 시작했다.
    assert_eq!(run.before.not_judged(), [Unjudged::BaselineSlotsNotWritten { event: m(2) }]);

    let RetryOutcome::Confirmed { commit, delta, reading } = &run.outcome else {
        panic!("확정이어야 한다: {:#?}", run.outcome);
    };
    // 실제 관측 경로를 한 번씩 지났고, 저장 한 번·다시 읽기 한 번이며 실패 기록은 청하지 않았다.
    assert_eq!((probe::observes() - observes, probe::captures() - captures), (1, 1));
    assert_eq!(desk.calls(), (1, 1, 0));

    // 저장 단위 — 새 세계 이름 A2, 보관 경계 C1, 발급기 2. 그 manifest 는 이 루트의 `.gil/` 창고에 놓였다.
    let stored = desk.records();
    assert_eq!((commit.snapshot(), commit.issues_name()), (a(2), true));
    assert_eq!(stored.registry.resolve(a(2)), Some(commit.manifest()));
    // 범위 변화 — 계속 추적한 src/* 둘만 견줬고, notes/* 는 새로 추적하며, data/* 는 견주지도 보관하지도 않았다.
    assert_eq!(delta.compared(), 2);
    assert!(delta.differences().is_empty());
    assert_eq!(delta.newly_tracked(), [path("notes/todo.md")]);
    assert_eq!(delta.not_compared(), [path("data/raw.csv")]);
    let archived = lab.store.read_manifest(commit.manifest()).expect("보관한 manifest 가 이 루트의 창고에 있다");
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
    // 시작할 때 의존한 근거(A1, 확정 정책 P1, 그 유도 경로)가 다시 읽은 기록에서 사건보다 앞의 기록만으로 같은 값으로 다시 유도됐다.
    let again = reading.world_before();
    assert_eq!(again.approval, run.before.world_before().approval);
    assert_eq!(again.second, SecondBasis::ArchiveBoundary { basis: run.before.confirmed().clone() });
    assert_eq!((again.approval.world(), again.approval.policy()), (a(1), p(1)));
    assert_eq!(desk.attempts(2), [NotArchived, Archived]);
    // 기존 승인·정책·사건·기준점은 그대로다.
    assert_eq!(stored.ledger, lab.records.ledger);
    assert_eq!(stored.links, lab.records.links);
    assert_eq!(stored.facts[0], lab.records.facts[0]);
    assert_eq!(stored.worlds, lab.records.worlds);
    assert_eq!(stored.registry.resolve(a(1)), lab.records.registry.resolve(a(1)));
}

#[test]
fn a_retry_after_interviews_closed_archives_at_the_boundary_stood_on_now() {
    // 승인은 C1 에서(발급기 2), 그 뒤 인터뷰 C2·C3 이 닫혔다. 지금 서 있는 경계는 C3, 발급기는 4 다(§6.3).
    let lab = Lab::with("later-boundary", &[P1_TEXT, P2_TEXT], &[(1, 2, 0)], 3);
    let desk = lab.desk(Save::Store);
    let run = lab.retry(2, &desk).expect("시작된다");
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
    let desk = lab.desk(Save::Store);
    let run = lab.retry(2, &desk).expect("시작된다");
    let RetryOutcome::Confirmed { commit, reading, .. } = &run.outcome else {
        panic!("확정이어야 한다: {:#?}", run.outcome);
    };
    assert_eq!((commit.snapshot(), commit.issues_name()), (a(1), false), "같은 세계는 기존 이름을 다시 가리킨다(§5.5)");
    assert_eq!(desk.records().registry.next_id(), 2);
    // 같은 이름 A1 이지만 사건의 기준점으로서의 확정 정책은 P2 이고, 보관 전 근거의 확정 정책은 P1 그대로다.
    assert_eq!((reading.confirmed().world(), reading.confirmed().policy()), (a(1), p(2)));
    assert_eq!((reading.world_before().approval.world(), reading.world_before().approval.policy()), (a(1), p(1)));
}

// ── 실행 전제 ──────────────────────────────────────────────────────────────

#[test]
fn a_waiting_record_whose_execution_prerequisites_are_not_confirmed_starts_nothing() {
    let cases: [(&str, fn(&mut OwnedRecords), Vec<Unjudged>); 4] = [
        (
            "no-marks",
            |records| records.marks = ScopeMarks::not_provided(),
            vec![Unjudged::ArchiveHaltNotProvided, Unjudged::UnfinishedRestoreNotProvided],
        ),
        (
            "no-halt-mark",
            |records| records.marks = ScopeMarks { archive_halt: MarkSeen::NotProvided, unfinished_restore: MarkSeen::Absent },
            vec![Unjudged::ArchiveHaltNotProvided],
        ),
        (
            "no-restore-mark",
            |records| records.marks = ScopeMarks { archive_halt: MarkSeen::Absent, unfinished_restore: MarkSeen::NotProvided },
            vec![Unjudged::UnfinishedRestoreNotProvided],
        ),
        (
            "no-review",
            |records| records.worlds[1] = EventWorldFacts::new(m(2), ScopeChange, Written(None), Written(Some(a(1)))),
            vec![Unjudged::ReviewCycleAbsent { event: m(2), written_null: true }],
        ),
    ];
    for (label, withhold, expected) in cases {
        let mut lab = Lab::new(label, 0);
        withhold(&mut lab.records);
        let desk = lab.desk(Save::Store);
        let (observes, captures) = (probe::observes(), probe::captures());

        // 읽기는 된다 — 유효한 대기이고, 판단하지 않은 것이 결과에 그대로 남는다.
        let reading = read_scope_change(m(2), &lab.records.inputs()).expect("부분 읽기는 된다");
        assert_eq!(reading.standing(), EventStanding::Waiting { attempts: 0 }, "{label}");
        assert!(expected.iter().all(|entry| reading.not_judged().contains(entry)), "{label}: {:?}", reading.not_judged());

        // 그러나 실행에 필요한 확인이 빠졌으므로 시도를 만들지 않는다 — 관측·객체 쓰기·상태 저장·실패 기록이 없다.
        let Err(NotStarted::PrerequisitesUnconfirmed { reading: refused, unconfirmed }) = lab.retry(2, &desk) else {
            panic!("{label}: 실행 전제를 확인하지 못한 입력에서는 시작하지 않는다");
        };
        assert_eq!(unconfirmed, expected, "{label}: 정상 부재(기준점 칸 없음)는 여기 들지 않는다");
        assert_eq!(refused, reading, "{label}: 읽기의 한계는 그대로 돌려준다");
        assert_eq!((probe::observes() - observes, probe::captures() - captures), (0, 0), "{label}");
        assert_eq!(desk.calls(), (0, 0, 0), "{label}");
        assert_eq!(desk.records().facts, lab.records.facts, "{label}");
        assert!(!lab.has_blob(b"todo"), "{label}: 새로 추적할 파일을 창고에 두지 않았다");
    }
}

#[test]
fn a_reread_record_without_confirmed_prerequisites_is_not_a_confirmation() {
    // 저장 경계는 저장 단위를 적었지만, 다시 읽기가 보관 중단 표식·미완 복원 흔적을 건네지 않는다.
    let lab = Lab::new("reread-no-marks", 0);
    let desk = lab.desk(Save::Store);
    desk.set(|held| held.reread_without_marks = true);
    let run = lab.retry(2, &desk).expect("시작된다");
    let RetryOutcome::Unknown { why: UnknownWhy::PrerequisitesUnconfirmed { unconfirmed }, .. } = &run.outcome else {
        panic!("확인되지 않은 전제의 기록으로 확정을 보고하지 않는다: {:#?}", run.outcome);
    };
    assert_eq!(unconfirmed, &[Unjudged::ArchiveHaltNotProvided, Unjudged::UnfinishedRestoreNotProvided]);
}

// ── 새로 관측한다 ──────────────────────────────────────────────────────────

#[test]
fn every_explicit_retry_observes_anew_and_a_fixed_file_is_read_fresh() {
    let lab = Lab::new("fresh", 0);
    let desk = lab.desk(Save::Store);
    let (observes, captures) = (probe::observes(), probe::captures());

    // 계속 추적한 파일이 밖에서 바뀌었다 — 첫 시도는 보관 전 관측에서 멈춘다.
    write(&lab.root, "src/a.txt", b"changed outside");
    let first = lab.retry(2, &desk).expect("시작된다");
    let (cause, now) = failed(&first);
    assert_eq!(differences(cause), (SeenAt::Observation, vec![("src/a.txt".to_string(), DifferenceKind::Modified)]));
    assert_eq!(still_waiting(now, &desk, 0), &FailureRecord::Recorded);
    assert_eq!((probe::observes() - observes, probe::captures() - captures), (1, 0), "보관으로 가지 않았다");
    assert_eq!(desk.calls(), (0, 1, 1), "저장을 청하지 않았고, 지금 기록을 한 번 다시 읽은 뒤 실패 시도를 한 번 적었다");
    assert_eq!(desk.attempts(2), [NotArchived]);
    assert!(!lab.has_blob(b"changed outside"), "미확정 변경의 내용을 창고에 두지 않았다");

    // 사용자가 정리했다. 같은 P2 로 다시 시도하면 처음부터 다시 관측한다 — 앞선 시도의 결과를 쓰지 않는다.
    write(&lab.root, "src/a.txt", b"alpha");
    let second = lab.retry(2, &desk).expect("시작된다");
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
        let desk = lab.desk(Save::Store);
        disturb(&lab.root);
        let run = lab.retry(2, &desk).expect("시작된다");
        let (cause, now) = failed(&run);
        assert_eq!(differences(cause), (SeenAt::Observation, vec![(path.to_string(), kind)]), "{label}");
        assert_eq!(still_waiting(now, &desk, 0), &FailureRecord::Recorded, "{label}");
        assert_eq!(desk.calls(), (0, 1, 1), "{label}: 저장을 청하지 않았다");
    }
}

#[test]
fn paths_outside_the_applied_scope_and_newly_tracked_paths_are_not_compared() {
    // data/ 는 적용 정책 P2 가 제외했다 — 그 안의 변경·삭제·새 파일은 미확정 변경이 아니고, 건드리지도 않는다.
    // notes/ 는 P1 이 제외했다가 P2 가 포함한다 — 비교할 과거 기준이 없고 지금 내용이 최초 보관이다.
    let lab = Lab::new("scope-effect", 0);
    let desk = lab.desk(Save::Store);
    write(&lab.root, "data/raw.csv", b"9,9,9");
    write(&lab.root, "data/extra.csv", b"extra");
    write(&lab.root, "notes/todo.md", b"rewritten");
    write(&lab.root, "notes/more.md", b"more");
    let run = lab.retry(2, &desk).expect("시작된다");
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
    let desk = through.desk(Save::Store);
    write(&through.root, "data/raw.csv", b"changed while excluded");
    let run = through.retry(3, &desk).expect("시작된다");
    assert_eq!((run.binding.confirmed_policy(), run.binding.applied()), (p(1), p(3)));
    let RetryOutcome::Confirmed { delta, .. } = &run.outcome else {
        panic!("확정이어야 한다: {:#?}", run.outcome);
    };
    assert_eq!(delta.newly_tracked(), [path("data/raw.csv")], "제외됐다가 다시 포함된 경로는 새로 추적한다");
    assert_eq!(delta.compared(), 2);
    assert!(through.has_blob(b"changed while excluded"));
    // 대체된 M2 는 다시 보관하지 않는다 — 시도를 만들지 않고 아무것도 부르지 않는다.
    let calls = desk.calls();
    assert!(matches!(through.retry(2, &desk), Err(NotStarted::Replaced { .. })));
    assert_eq!(desk.calls(), calls);

    // 중간 판이 그 경로를 제외하지 않았다면 같은 변경은 계속 추적한 경로의 미확정 변경이다.
    let straight = Lab::with("kept-tracked", &[P1_TEXT, b"notes/\nnothing-here/\n", b"notes/\n"], &[(1, 2, 0), (1, 2, 0)], 1);
    let desk = straight.desk(Save::Store);
    write(&straight.root, "data/raw.csv", b"changed while tracked");
    let run = straight.retry(3, &desk).expect("시작된다");
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
    let desk = lab.desk(Save::Store);
    let run = lab.retry(2, &desk).expect("시작된다");
    let (cause, now) = failed(&run);
    assert_eq!(differences(cause), (SeenAt::Observation, vec![("src/a.txt".to_string(), DifferenceKind::Modified)]));
    assert_eq!(still_waiting(now, &desk, 0), &FailureRecord::Recorded);
    assert_eq!(lab.file(".gilignore"), proposal, "정책 파일을 읽지도 쓰지도 않았다");
}

// ── 걸음마다: 검사 뒤의 변화 · 불안정 · 관측 불가 ───────────────────────────

/// 기다리는 요청을 이 시도의 대상에 실제로 실행하고 그 보고로 한 걸음 나아간다.
fn step(attempt: &mut RetryAttempt<'_>) -> RetryStep {
    let report = attempt.perform().expect("끝나지 않은 시도다");
    attempt.advance(report).expect("방금 낸 보고다")
}

/// 실패 사유가 정해진 뒤의 걸음 — 지금 기록을 다시 읽고, 그 결속의 대기를 확인한 뒤에만 실패 시도를 적고 끝난다.
fn fail_through(attempt: &mut RetryAttempt<'_>) -> (FailureCause, AfterFailure) {
    assert!(matches!(attempt.pending(), Some(RetryRequest::Reread { .. })), "실패 뒤에는 지금 기록을 다시 읽는다");
    let RetryStep::Next(RetryRequest::RecordFailure { failure, .. }) = step(attempt) else {
        panic!("그 결속의 대기를 확인한 뒤 실패 시도를 적는다");
    };
    let RetryStep::Done(RetryOutcome::Failed { cause, now }) = step(attempt) else {
        panic!("실패로 끝난다");
    };
    assert_eq!(failure.cause(), &cause);
    (cause, now)
}

#[test]
fn a_change_between_the_check_and_the_capture_is_not_confirmed_even_though_objects_remain() {
    let lab = Lab::new("after-check", 0);
    let desk = lab.desk(Save::Store);
    let given = desk.records();
    let mut boundary = desk.clone();
    let mut attempt = RetryAttempt::begin(m(2), given.inputs(), &lab.root, &mut boundary).expect("시작된다");

    assert!(matches!(step(&mut attempt), RetryStep::Next(RetryRequest::Capture { .. })), "앞선 검사는 통과했다");
    // 검사 뒤, 객체 확정 전에 계속 추적한 파일이 바뀐다.
    write(&lab.root, "src/a.txt", b"changed after the check");
    assert!(matches!(step(&mut attempt), RetryStep::Next(RetryRequest::Reread { .. })), "capture 의 결과에도 같은 검사를 한다");
    let (cause, now) = fail_through(&mut attempt);
    assert_eq!(differences(&cause), (SeenAt::Capture, vec![("src/a.txt".to_string(), DifferenceKind::Modified)]));

    // capture 가 둔 객체는 창고에 남았다. 그것은 성공이 아니다 — 이름도, 기준점도, 저장 요청도 없다.
    assert!(lab.has_blob(b"changed after the check"));
    assert_eq!(desk.calls(), (0, 1, 1));
    assert_eq!(still_waiting(&now, &desk, 0), &FailureRecord::Recorded);
}

#[test]
fn an_observation_that_moves_between_its_two_scans_is_unstable_not_clean() {
    // ① 보관 전 관측이 흔들린다.
    let lab = Lab::new("unstable-observe", 0);
    let desk = lab.desk(Save::Store);
    let root = lab.root.clone();
    probe::between_next_scans(move || write(&root, "notes/todo.md", b"moved between scans"));
    let run = lab.retry(2, &desk).expect("시작된다");
    let (cause, now) = failed(&run);
    assert_eq!(cause, &FailureCause::UnstableObservation { seen_at: SeenAt::Observation });
    assert_eq!(still_waiting(now, &desk, 0), &FailureRecord::Recorded);
    assert_eq!(desk.calls(), (0, 1, 1));

    // ② 객체를 확정하는 관측이 흔들린다 — 첫 훑기의 객체가 남아도 성공이 아니다.
    let lab = Lab::new("unstable-capture", 0);
    let desk = lab.desk(Save::Store);
    let given = desk.records();
    let mut boundary = desk.clone();
    let mut attempt = RetryAttempt::begin(m(2), given.inputs(), &lab.root, &mut boundary).expect("시작된다");
    assert!(matches!(step(&mut attempt), RetryStep::Next(RetryRequest::Capture { .. })));
    let root = lab.root.clone();
    probe::between_next_scans(move || write(&root, "notes/todo.md", b"moved during capture"));
    assert!(matches!(step(&mut attempt), RetryStep::Next(RetryRequest::Reread { .. })), "흔들린 보관은 실패다");
    let (cause, now) = fail_through(&mut attempt);
    assert_eq!(cause, FailureCause::UnstableObservation { seen_at: SeenAt::Capture });
    assert!(lab.has_blob(b"todo"), "첫 훑기가 둔 객체는 미참조로 남는다");
    assert_eq!(desk.calls(), (0, 1, 1));
    assert_eq!(still_waiting(&now, &desk, 0), &FailureRecord::Recorded);
}

#[test]
fn an_observation_that_is_refused_is_not_treated_as_no_change() {
    // 추적 범위 안의 하위 정책 파일은 관측 전체를 막는다.
    let lab = Lab::new("refused", 0);
    let desk = lab.desk(Save::Store);
    write(&lab.root, "src/.gilignore", b"*.tmp\n");
    let run = lab.retry(2, &desk).expect("시작된다");
    let (cause, now) = failed(&run);
    assert!(matches!(cause, FailureCause::ObservationRefused { seen_at: SeenAt::Observation, .. }), "{cause:#?}");
    assert_eq!(still_waiting(now, &desk, 0), &FailureRecord::Recorded);
    assert_eq!(desk.calls(), (0, 1, 1));
}

// ── 한 시도의 실제 대상 ────────────────────────────────────────────────────

/// 그림의 폴더와 같은 파일을 새 디렉터리에 둔다 — 내용은 같지만 다른 디렉터리다.
fn lookalike(root: &Path) {
    fs::create_dir_all(root.join(".gil")).expect("새 폴더를 만든다");
    write(root, "src/a.txt", b"alpha");
    write(root, "src/b.txt", b"beta");
    write(root, "data/raw.csv", b"1,2,3");
    write(root, "notes/todo.md", b"todo");
}

#[test]
fn the_root_an_attempt_started_with_is_the_only_root_it_observes() {
    let lab = Lab::new("root-bound", 0);
    let desk = lab.desk(Save::Store);
    let given = desk.records();
    let mut boundary = desk.clone();
    let mut attempt = RetryAttempt::begin(m(2), given.inputs(), &lab.root, &mut boundary).expect("시작된다");
    let (observes, captures) = (probe::observes(), probe::captures());

    // 시작한 뒤 같은 경로에 다른 디렉터리가 놓였다 — 내용이 같아 보여도 이 시도가 시작한 대상이 아니다.
    let moved = lab.beside("moved");
    fs::rename(&lab.root, &moved).expect("시험 루트를 옮긴다");
    lookalike(&lab.root);

    assert!(matches!(step(&mut attempt), RetryStep::Next(RetryRequest::Reread { .. })), "달라진 대상의 관측으로 나아가지 않는다");
    let (cause, now) = fail_through(&mut attempt);
    assert!(matches!(cause, FailureCause::TargetChanged { .. }), "{cause:#?}");
    assert_eq!((probe::observes() - observes, probe::captures() - captures), (0, 0), "달라진 대상을 관측하지도 않았다");
    assert_eq!(desk.calls(), (0, 1, 1), "저장을 청하지 않았다");
    assert_eq!(still_waiting(&now, &desk, 0), &FailureRecord::Recorded);
    // 새로 놓인 디렉터리에는 아무것도 쓰지 않았다.
    assert!(!lab.root.join(".gil").join("artifacts").exists());
}

#[test]
fn the_store_an_attempt_started_with_is_the_only_store_it_writes() {
    let lab = Lab::new("store-bound", 0);
    let desk = lab.desk(Save::Store);
    let given = desk.records();
    let mut boundary = desk.clone();
    let mut attempt = RetryAttempt::begin(m(2), given.inputs(), &lab.root, &mut boundary).expect("시작된다");
    assert!(matches!(step(&mut attempt), RetryStep::Next(RetryRequest::Capture { .. })), "관측과 검사는 통과했다");
    let captures = probe::captures();

    // 관측 뒤, 객체 확정 전에 창고 자리에 다른 디렉터리가 놓였다.
    let artifacts = lab.root.join(".gil").join("artifacts");
    fs::rename(&artifacts, lab.beside("artifacts-moved")).expect("시험 창고를 옮긴다");
    fs::create_dir(&artifacts).expect("빈 폴더를 놓는다");

    assert!(matches!(step(&mut attempt), RetryStep::Next(RetryRequest::Reread { .. })), "달라진 창고에 보관하지 않는다");
    let (cause, now) = fail_through(&mut attempt);
    assert!(matches!(cause, FailureCause::TargetChanged { .. }), "{cause:#?}");
    assert_eq!(probe::captures() - captures, 0, "달라진 창고로 보관을 부르지 않았다");
    assert_eq!(fs::read_dir(&artifacts).expect("빈 폴더를 읽는다").count(), 0, "새로 놓인 창고 자리에 아무것도 쓰지 않았다");
    assert_eq!(desk.calls(), (0, 1, 1));
    assert_eq!(still_waiting(&now, &desk, 0), &FailureRecord::Recorded);
}

#[test]
fn a_root_without_its_store_is_not_a_target() {
    // `.gil/` 의 객체 창고가 없는 폴더 — 이 시도의 대상으로 정하지 못한다. 아무것도 만들지 않는다.
    let lab = Lab::new("no-store", 0);
    let desk = lab.desk(Save::Store);
    let empty = lab.beside("empty");
    fs::create_dir(&empty).expect("빈 폴더를 만든다");
    let given = desk.records();
    let mut boundary = desk.clone();
    assert!(matches!(run_retry(m(2), given.inputs(), &empty, &mut boundary), Err(NotStarted::Target { .. })));
    assert_eq!(fs::read_dir(&empty).expect("빈 폴더를 읽는다").count(), 0);
    assert_eq!(desk.calls(), (0, 0, 0));
}

// ── 저장 결과 ──────────────────────────────────────────────────────────────

#[test]
fn a_confirmed_store_failure_keeps_the_policy_the_confirmed_world_and_the_wait() {
    let lab = Lab::new("not-stored", 0);
    let desk = lab.desk(Save::Refuse);
    let run = lab.retry(2, &desk).expect("시작된다");
    let (cause, now) = failed(&run);
    assert!(matches!(cause, FailureCause::StateNotStored { .. }), "{cause:#?}");
    // 저장 경계가 저장하지 않았다고 답한 뒤 지금 기록을 다시 읽어, 적용 P2 / 마지막 확정 세계 A1(P1) / 기준점 없음을 확인했다.
    assert_eq!(still_waiting(now, &desk, 0), &FailureRecord::Recorded);
    assert_eq!(desk.calls(), (1, 1, 1));
    assert_eq!(desk.attempts(2), [NotArchived]);
    // 객체는 창고에 놓였다. 그래도 기준점은 없다.
    assert!(lab.has_blob(b"todo"));
}

#[test]
fn a_failure_whose_record_cannot_be_stored_is_still_a_failure() {
    let lab = Lab::new("unrecorded", 0);
    let desk = lab.desk(Save::Store);
    desk.set(|held| held.record_fails = true);
    write(&lab.root, "src/a.txt", b"changed outside");
    let run = lab.retry(2, &desk).expect("시작된다");
    let (cause, now) = failed(&run);
    assert!(matches!(cause, FailureCause::UnconfirmedChange { .. }));
    // 시도 기록이 남지 않았다 — 기록이 없다는 사실이 성공을 뜻하지 않는다. 다시 읽은 기록은 P2 / A1(P1) / 대기 그대로다.
    assert!(matches!(still_waiting(now, &desk, 0), FailureRecord::NotRecorded { .. }));
    assert_eq!(desk.calls(), (0, 1, 1));
    assert_eq!(desk.attempts(2), []);
}

#[test]
fn a_failure_that_cannot_reread_the_record_does_not_say_the_event_still_waits() {
    // 저장 경계가 저장하지 않았다고 답했고, 지금 기록은 다시 읽을 수 없다.
    let lab = Lab::new("failed-unverified", 0);
    let desk = lab.desk(Save::Refuse);
    desk.set(|held| held.reread_fails = true);
    let run = lab.retry(2, &desk).expect("시작된다");
    let (cause, now) = failed(&run);
    assert!(matches!(cause, FailureCause::StateNotStored { .. }), "{cause:#?}");
    assert!(matches!(now, AfterFailure::NotVerified { why: UnknownWhy::RereadUnavailable { .. } }), "{now:#?}");
    assert_eq!(desk.calls(), (1, 1, 0), "지금 기록을 확인하지 못했으므로 실패 시도를 덧붙이지 않았다");
    assert_eq!(desk.attempts(2), []);
}

#[test]
fn an_error_reported_after_the_store_is_confirmed_by_the_reread() {
    let lab = Lab::new("stored-then-error", 0);
    let desk = lab.desk(Save::StoreButSayUncertain);
    let run = lab.retry(2, &desk).expect("시작된다");
    assert!(matches!(run.outcome, RetryOutcome::Confirmed { .. }), "{:#?}", run.outcome);
    assert_eq!(desk.calls(), (1, 1, 0));
}

#[test]
fn an_uncertain_store_that_cannot_be_reread_is_unknown_neither_success_nor_a_settled_wait() {
    for save in [Save::StoreButSayUncertain, Save::Store] {
        let lab = Lab::new("unknown", 0);
        let desk = lab.desk(save);
        desk.set(|held| held.reread_fails = true);
        let run = lab.retry(2, &desk).expect("시작된다");
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
    let desk = lab.desk(Save::UncertainWithoutStoring);
    let run = lab.retry(2, &desk).expect("시작된다");
    let (cause, now) = failed(&run);
    assert!(matches!(cause, FailureCause::StateNotStored { .. }), "{cause:#?}");
    assert_eq!(still_waiting(now, &desk, 0), &FailureRecord::Recorded);
    assert_eq!(desk.calls(), (1, 1, 1), "저장 뒤의 다시 읽기 한 번으로 저장되지 않았음과 지금의 대기를 함께 확인했다");
}

#[test]
fn a_stored_answer_alone_is_not_a_confirmation() {
    // 저장 경계가 저장했다고 답했지만 다시 읽은 기록은 앞선 대기 그대로다.
    let lab = Lab::new("said-stored", 0);
    let desk = lab.desk(Save::SayStoredWithoutStoring);
    let run = lab.retry(2, &desk).expect("시작된다");
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
    let desk = lab.desk(Save::StoreWithAnExtraFailure);
    let run = lab.retry(2, &desk).expect("시작된다");
    assert!(
        matches!(
            &run.outcome,
            RetryOutcome::Unknown {
                why: UnknownWhy::RereadDiffers { found: EventStanding::AlreadyConfirmed { attempts: 2, .. }, .. },
                ..
            }
        ),
        "{:#?}",
        run.outcome
    );

    // 다시 읽은 기록에서 앞선 실패 시도가 사라졌다 — 이력을 보존하지 않는 저장은 확정으로 보고하지 않는다.
    let lab = Lab::new("lost-history", 1);
    let desk = lab.desk(Save::StoreButForgetAttempts);
    let run = lab.retry(2, &desk).expect("시작된다");
    assert!(
        matches!(&run.outcome, RetryOutcome::Unknown { why: UnknownWhy::HistoryNotKept { .. }, .. }),
        "{:#?}",
        run.outcome
    );
}

#[test]
fn a_reread_record_whose_lineage_basis_no_longer_stands_is_not_a_confirmation() {
    // 승인은 C1 에서, 지금 경계는 C3 — 마지막 확정 세계 A1(P1)의 근거는 C3 → C2 → C1 → start 의 계승이다. 저장 경계는 이 시도의 저장
    // 단위를 그대로 적었고 정책·사건 칸·기준점·시도·등록부도 그대로지만, 다시 읽은 기록에서는 그 경로 위 C2 의 저장 Exit 이 달라져 있다.
    let lab = Lab::with("basis-bent", &[P1_TEXT, P2_TEXT], &[(1, 2, 0)], 3);
    let desk = lab.desk(Save::StoreAndBendAnAncestor);
    let run = lab.retry(2, &desk).expect("시작된다");
    let stored = desk.records();
    assert_eq!(stored.facts[1].baseline().snapshot(), Some(a(2)), "저장 단위는 적혀 있다");
    assert_eq!(desk.attempts(2), [Archived]);
    let RetryOutcome::Unknown { why: UnknownWhy::RereadUnreadable { problems }, answer: SaveAnswer::Stored, .. } = &run.outcome else {
        panic!("근거가 다시 서지 않는 기록으로 확정을 보고하지 않는다: {:#?}", run.outcome);
    };
    assert!(
        problems.iter().any(|problem| matches!(
            problem,
            ReadingProblem::PriorBasis { at: BasisAt::Archive, boundary, .. } if *boundary == c(3)
        )),
        "보관 경계 C3 의 앞선 근거가 서지 않는다고 말해야 한다: {problems:#?}"
    );
}

// ── 오래된 시도와 지금 ─────────────────────────────────────────────────────

#[test]
fn an_older_attempt_that_fails_after_another_confirmed_reports_the_present_record_not_its_starting_one() {
    let lab = Lab::new("stale", 0);
    let desk = lab.desk(Save::Store);
    let given = desk.records();

    // 같은 옛 기록에서 시작해 둔 시도 — 아직 한 걸음도 실행하지 않았다.
    let mut held = desk.clone();
    let mut older = RetryAttempt::begin(m(2), given.inputs(), &lab.root, &mut held).expect("시작된다");
    assert_eq!(older.before().standing(), EventStanding::Waiting { attempts: 0 });

    // 그 사이 다른 명시 재시도가 확정했다.
    let first = lab.retry(2, &desk).expect("시작된다");
    assert!(matches!(first.outcome, RetryOutcome::Confirmed { .. }), "{:#?}", first.outcome);
    let (calls, stored) = (desk.calls(), desk.records().facts);

    // 오래된 시도를 끝까지 실행한다 — 저장 경계는 결속이 지금 기록과 다른 저장 단위를 적지 않는다.
    while older.outcome().is_none() {
        step(&mut older);
    }
    let Some(RetryOutcome::Failed { cause, now }) = older.outcome() else {
        panic!("오래된 시도는 확정하지 못한다: {:#?}", older.outcome());
    };
    assert!(matches!(cause, FailureCause::StateNotStored { .. }), "{cause:#?}");
    // 「이 시도가 저장하지 못했다」와 「지금 사건이 대기다」는 다르다 — 지금 기록은 이미 확정이고, 그 지금의 읽기를 돌려준다.
    let AfterFailure::BindingGone { reading, changed } = now else {
        panic!("시작할 때의 대기가 그대로라고 말하지 않는다: {now:#?}");
    };
    assert!(changed.contains(&BindingPart::Standing) && !changed.contains(&BindingPart::Boundary), "{changed:?}");
    assert_eq!(
        reading.standing(),
        EventStanding::AlreadyConfirmed { snapshot: a(2), archived_at: c(1), archive_issuer: 2, attempts: 1 }
    );
    assert_eq!((reading.confirmed().world(), reading.confirmed().policy(), reading.applied().policy), (a(2), p(2), p(2)));
    // 중복 확정도, 실패 시도의 덧붙임도 없다 — 기준점과 시도 이력은 먼저 확정한 시도가 남긴 그대로다.
    assert_eq!(desk.calls(), (calls.0 + 1, calls.1 + 1, calls.2), "저장 요청 한 번(거절됨)과 다시 읽기 한 번뿐이다");
    assert_eq!(desk.records().facts, stored);
    assert_eq!(desk.attempts(2), [Archived]);
    assert_eq!(desk.records().registry.next_id(), 3);

    // 그 뒤의 조회와 새 요청은 이미 확정으로 읽는다 — 다시 읽기가 새 시도로 이어지지 않았다.
    assert!(matches!(lab.retry(2, &desk), Err(NotStarted::AlreadyConfirmed { .. })));
    assert_eq!(desk.calls(), (calls.0 + 1, calls.1 + 1, calls.2));
}

#[test]
fn an_older_attempt_that_fails_before_saving_does_not_append_to_a_record_another_attempt_confirmed() {
    let lab = Lab::new("stale-early", 0);
    let desk = lab.desk(Save::Store);
    let given = desk.records();
    let mut held = desk.clone();
    let mut older = RetryAttempt::begin(m(2), given.inputs(), &lab.root, &mut held).expect("시작된다");

    let first = lab.retry(2, &desk).expect("시작된다");
    assert!(matches!(first.outcome, RetryOutcome::Confirmed { .. }), "{:#?}", first.outcome);
    let calls = desk.calls();

    // 그 뒤 계속 추적한 파일이 바뀌었다 — 오래된 시도는 관측에서 실패한다.
    write(&lab.root, "src/a.txt", b"changed later");
    while older.outcome().is_none() {
        step(&mut older);
    }
    let Some(RetryOutcome::Failed { cause, now }) = older.outcome() else {
        panic!("오래된 시도는 실패한다: {:#?}", older.outcome());
    };
    assert!(matches!(cause, FailureCause::UnconfirmedChange { seen_at: SeenAt::Observation, .. }), "{cause:#?}");
    assert!(
        matches!(now, AfterFailure::BindingGone { reading, .. } if matches!(reading.standing(), EventStanding::AlreadyConfirmed { .. })),
        "{now:#?}"
    );
    assert_eq!(desk.calls(), (calls.0, calls.1 + 1, calls.2), "다시 읽기 한 번뿐 — 저장 요청도 실패 기록 요청도 없다");
    assert_eq!(desk.attempts(2), [Archived]);
}

// ── 같은 시도 수, 다른 결속 ────────────────────────────────────────────────

/// 대기 중 인터뷰 C2 가 열려 닫힌 뒤의 결속에서 달라지는 곳 — 사건·시도 수·적용 정책·확정 세계와 확정 정책은 그대로이고, 서 있는 경계와
/// 발급기, 그 경계의 유도 경로, `world_before_ref` 의 두 번째 근거(지금 경계)가 다르다.
const AFTER_ONE_MORE_INTERVIEW: [BindingPart; 4] =
    [BindingPart::Boundary, BindingPart::Issuer, BindingPart::ConfirmedBasis, BindingPart::ApprovalBasis];

#[test]
fn a_wait_with_the_same_attempt_count_at_a_later_boundary_is_not_the_binding_the_attempt_started_with() {
    let lab = Lab::new("moved-boundary", 0);
    let desk = lab.desk(Save::Store);
    let given = desk.records();
    let mut boundary = desk.clone();
    let mut attempt = RetryAttempt::begin(m(2), given.inputs(), &lab.root, &mut boundary).expect("시작된다");
    let started = attempt.before().clone();
    assert_eq!(
        (started.confirmed().boundary(), started.issuer(), started.standing()),
        (c(1), 2, EventStanding::Waiting { attempts: 0 })
    );

    // 정상적인 기록 진행: 대기 중 인터뷰 C2 가 열려 닫혔다(§6.3). 사건은 여전히 기준점 없이 같은 시도 수로 대기한다.
    desk.set(|held| held.records.chains = interviews(2));
    // 그 뒤 이 시도는 계속 추적한 파일의 변경으로 실패한다.
    write(&lab.root, "src/a.txt", b"changed outside");
    while attempt.outcome().is_none() {
        step(&mut attempt);
    }
    let Some(RetryOutcome::Failed { cause, now }) = attempt.outcome() else {
        panic!("실패로 끝난다: {:#?}", attempt.outcome());
    };
    assert!(matches!(cause, FailureCause::UnconfirmedChange { seen_at: SeenAt::Observation, .. }), "{cause:#?}");
    let AfterFailure::BindingGone { reading, changed } = now else {
        panic!("시도 수가 같다는 것만으로 같은 결속의 대기라고 하지 않는다: {now:#?}");
    };

    // 다시 읽은 기록은 읽히고 유도도 선다 — 같은 시도 수의 대기, 같은 적용 정책 P2, 같은 마지막 확정 세계 A1 과 확정 정책 P1.
    assert_eq!(reading.standing(), EventStanding::Waiting { attempts: 0 });
    assert_eq!((reading.applied().policy, reading.confirmed().world(), reading.confirmed().policy()), (p(2), a(1), p(1)));
    assert_eq!(reading.world_before().approval, started.world_before().approval, "승인 경계 C1 의 근거는 그대로다");
    // 그러나 서 있는 경계와 발급기가 다르고, 지금 경계의 유도는 서지만 시작할 때의 근거와 다른 값이다.
    assert_eq!((reading.confirmed().boundary(), reading.issuer()), (c(2), 3));
    assert_ne!(reading.confirmed().path(), started.confirmed().path());
    assert_eq!(changed, &AFTER_ONE_MORE_INTERVIEW);

    // 이전 결속의 실패 시도를 청하지 않았다 — 다시 읽기 한 번뿐이고 시도 이력은 그대로다.
    assert_eq!(desk.calls(), (0, 1, 0));
    assert_eq!(desk.attempts(2), []);
    assert_eq!(desk.records().registry.next_id(), 2);
}

#[test]
fn an_uncertain_save_reread_at_a_later_boundary_stays_unknown_and_records_no_failure() {
    let lab = Lab::new("uncertain-moved", 0);
    let desk = lab.desk(Save::UncertainWithoutStoring);
    let given = desk.records();
    let mut boundary = desk.clone();
    let mut attempt = RetryAttempt::begin(m(2), given.inputs(), &lab.root, &mut boundary).expect("시작된다");
    assert!(matches!(step(&mut attempt), RetryStep::Next(RetryRequest::Capture { .. })));
    assert!(matches!(step(&mut attempt), RetryStep::Next(RetryRequest::StoreManifest { .. })));
    assert!(matches!(step(&mut attempt), RetryStep::Next(RetryRequest::Save { .. })));

    // 저장 요청 전에 기록이 정상적으로 진행했다 — 대기 중 인터뷰 C2 가 닫혔다. 저장 경계는 불확실하다고 답한다.
    desk.set(|held| held.records.chains = interviews(2));
    assert!(matches!(step(&mut attempt), RetryStep::Next(RetryRequest::Reread { .. })), "불확실한 답 뒤에는 다시 읽는다");
    let RetryStep::Done(RetryOutcome::Unknown { answer, why, .. }) = step(&mut attempt) else {
        panic!("결속이 달라진 기록으로 저장 여부를 단정하지 않는다: {:#?}", attempt.outcome());
    };
    assert!(matches!(answer, SaveAnswer::Uncertain { .. }));
    let UnknownWhy::RereadDiffers { found, changed } = &why else {
        panic!("달라진 결속을 말해야 한다: {why:#?}");
    };
    // 사건에는 기준점이 없고 시도 수도 같다. 그래도 같은 결속의 대기가 아니므로 확인된 미저장으로 바꾸지 않고, 실패 시도도 청하지 않는다.
    assert_eq!(*found, EventStanding::Waiting { attempts: 0 });
    assert_eq!(changed, &AFTER_ONE_MORE_INTERVIEW);
    assert_eq!(desk.calls(), (1, 1, 0));
    assert_eq!(desk.attempts(2), []);
}

#[test]
fn the_state_boundary_refuses_a_unit_whose_binding_moved_and_the_attempt_reports_the_present_record() {
    let lab = Lab::new("refused-moved", 1);
    let desk = lab.desk(Save::Store);
    let given = desk.records();
    let mut boundary = desk.clone();
    let mut attempt = RetryAttempt::begin(m(2), given.inputs(), &lab.root, &mut boundary).expect("시작된다");
    assert!(matches!(step(&mut attempt), RetryStep::Next(RetryRequest::Capture { .. })));
    assert!(matches!(step(&mut attempt), RetryStep::Next(RetryRequest::StoreManifest { .. })));
    assert!(matches!(step(&mut attempt), RetryStep::Next(RetryRequest::Save { .. })));

    // 저장 요청 전에 인터뷰 C2 가 닫혔다. 저장 경계는 쓰는 시점에 결속 전체를 다시 보고, 시도 수가 같아도 이 저장 단위를 적지 않는다.
    desk.set(|held| held.records.chains = interviews(2));
    while attempt.outcome().is_none() {
        step(&mut attempt);
    }
    let Some(RetryOutcome::Failed { cause, now }) = attempt.outcome() else {
        panic!("저장하지 못한 시도다: {:#?}", attempt.outcome());
    };
    assert!(matches!(cause, FailureCause::StateNotStored { .. }), "{cause:#?}");
    let AfterFailure::BindingGone { reading, changed } = now else {
        panic!("지금 기록은 다른 결속의 대기다: {now:#?}");
    };
    // 단계 전 판정이 말하는 달라진 곳과 저장 경계가 쓰는 시점에 본 것이 같은 비교다.
    assert_eq!(changed, &AFTER_ONE_MORE_INTERVIEW);
    assert_eq!(
        attempt.binding().holds_in(&desk.records().inputs()),
        Err(BindingLost::Changed(AFTER_ONE_MORE_INTERVIEW.to_vec()))
    );
    assert_eq!(reading.standing(), EventStanding::Waiting { attempts: 1 });
    assert_eq!((reading.applied().policy, reading.confirmed().world(), reading.confirmed().policy()), (p(2), a(1), p(1)));
    // 저장 요청 한 번(거절됨)과 다시 읽기 한 번. 기준점도, 세계 이름도, 이전 결속의 실패 시도도 덧붙지 않았다.
    assert_eq!(desk.calls(), (1, 1, 0));
    assert_eq!(desk.attempts(2), [NotArchived]);
    assert_eq!(desk.records().registry.next_id(), 2);
}

#[test]
fn the_write_time_check_of_the_state_boundary_is_the_comparison_the_attempt_makes() {
    let lab = Lab::new("holds", 0);
    let desk = lab.desk(Save::Store);
    let given = desk.records();
    let mut boundary = desk.clone();
    let attempt = RetryAttempt::begin(m(2), given.inputs(), &lab.root, &mut boundary).expect("시작된다");
    let binding = attempt.binding().clone();
    drop(attempt);

    // 시작할 때의 기록 그대로 — 결속이 선다. 같은 기록을 새로 복사해 물어도(새 판정 인스턴스) 같고, 아무것도 실행되지 않는다.
    assert_eq!(binding.changes(binding.started()), []);
    assert_eq!(binding.holds_in(&given.inputs()), Ok(()));
    assert_eq!(binding.holds_in(&given.clone().inputs()), Ok(()));

    // 시도 수는 같고 경계만 나아간 기록 — 같은 결속이 아니다.
    let mut moved = given.clone();
    moved.chains = interviews(2);
    assert_eq!(binding.holds_in(&moved.inputs()), Err(BindingLost::Changed(AFTER_ONE_MORE_INTERVIEW.to_vec())));

    // 실패 시도 하나가 덧붙은 기록 — 시도 수가 다르다.
    let mut tried = given.clone();
    tried.facts[1] = waiting(2, 1, 2, 1);
    assert_eq!(binding.holds_in(&tried.inputs()), Err(BindingLost::Changed(vec![BindingPart::Standing])));

    // 실행 전제를 건네지 않은 기록과 보관 중단 표식이 있다고 건넨 기록 — 결속이 선다고 말하지 않는다.
    let mut unmarked = given.clone();
    unmarked.marks = ScopeMarks::not_provided();
    assert_eq!(
        binding.holds_in(&unmarked.inputs()),
        Err(BindingLost::PrerequisitesUnconfirmed(vec![Unjudged::ArchiveHaltNotProvided, Unjudged::UnfinishedRestoreNotProvided]))
    );
    let mut halted = given.clone();
    halted.marks = ScopeMarks { archive_halt: MarkSeen::Present, unfinished_restore: MarkSeen::Absent };
    assert_eq!(
        binding.holds_in(&halted.inputs()),
        Err(BindingLost::Unreadable(vec![ReadingProblem::OutOfScope(ScopeLimit::ArchiveHaltPresent)]))
    );

    assert_eq!(desk.calls(), (0, 0, 0));
    assert_eq!(desk.records().facts, lab.records.facts);
}

#[test]
fn a_failure_after_the_event_was_replaced_reports_the_replacement_and_appends_nothing() {
    let lab = Lab::new("replaced-meanwhile", 0);
    let desk = lab.desk(Save::Store);
    let given = desk.records();
    let mut boundary = desk.clone();
    let mut attempt = RetryAttempt::begin(m(2), given.inputs(), &lab.root, &mut boundary).expect("시작된다");

    // 정상적인 기록 진행: 기준점 없이 대기하던 M2 뒤에 새 범위 변경 M3(P2 → P3)이 승인·저장됐다 — M2 는 대체된 사건이다(§6.3). 앞선
    // 판·사건 칸·보충 칸·등록부는 그대로이고 M3 의 판과 칸만 덧붙었다.
    let later = Lab::with("replaced-later", &[P1_TEXT, P2_TEXT, b"notes/\n"], &[(1, 2, 0), (1, 2, 0)], 1);
    desk.set(|held| {
        let registry = held.records.registry.clone();
        held.records = OwnedRecords { registry, ..later.records.clone() };
    });
    write(&lab.root, "src/a.txt", b"changed outside");
    while attempt.outcome().is_none() {
        step(&mut attempt);
    }
    let Some(RetryOutcome::Failed { cause, now }) = attempt.outcome() else {
        panic!("실패로 끝난다: {:#?}", attempt.outcome());
    };
    assert!(matches!(cause, FailureCause::UnconfirmedChange { seen_at: SeenAt::Observation, .. }), "{cause:#?}");
    let AfterFailure::BindingGone { reading, changed } = now else {
        panic!("대체된 사건을 시작할 때의 대기라고 말하지 않는다: {now:#?}");
    };
    assert_eq!(reading.standing(), EventStanding::Replaced { by: m(3) });
    assert_eq!(reading.applied().policy, p(3));
    assert!(changed.contains(&BindingPart::Standing) && changed.contains(&BindingPart::AppliedPolicy), "{changed:?}");
    // 대체된 사건에도, 새 사건에도 이전 결속의 실패 시도를 덧붙이지 않았다.
    assert_eq!(desk.calls(), (0, 1, 0));
    assert_eq!(desk.attempts(2), []);
    assert_eq!(desk.attempts(3), []);
}

#[test]
fn a_reread_without_confirmed_prerequisites_after_a_failure_or_an_uncertain_save_is_left_unverified() {
    let withheld = [Unjudged::ArchiveHaltNotProvided, Unjudged::UnfinishedRestoreNotProvided];

    // 저장 경계가 저장하지 않았다고 답했고, 다시 읽기는 보관 중단 표식·미완 복원 흔적을 건네지 않는다.
    let lab = Lab::new("failed-no-marks", 0);
    let desk = lab.desk(Save::Refuse);
    desk.set(|held| held.reread_without_marks = true);
    let run = lab.retry(2, &desk).expect("시작된다");
    let (cause, now) = failed(&run);
    assert!(matches!(cause, FailureCause::StateNotStored { .. }), "{cause:#?}");
    let AfterFailure::NotVerified { why: UnknownWhy::PrerequisitesUnconfirmed { unconfirmed } } = now else {
        panic!("전제를 확인하지 못한 기록으로 지금의 대기를 말하지 않는다: {now:#?}");
    };
    assert_eq!(unconfirmed, &withheld);
    assert_eq!(desk.calls(), (1, 1, 0), "지금 기록을 확인하지 못했으므로 실패 시도를 청하지 않았다");
    assert_eq!(desk.attempts(2), []);

    // 저장 경계가 불확실하다고 답했고(적지 않았다), 다시 읽기는 같은 전제를 건네지 않는다 — 성공도, 확인된 미저장도 아니다.
    let lab = Lab::new("uncertain-no-marks", 0);
    let desk = lab.desk(Save::UncertainWithoutStoring);
    desk.set(|held| held.reread_without_marks = true);
    let run = lab.retry(2, &desk).expect("시작된다");
    let RetryOutcome::Unknown { answer, why: UnknownWhy::PrerequisitesUnconfirmed { unconfirmed }, .. } = &run.outcome else {
        panic!("전제를 확인하지 못한 기록으로 저장 여부를 단정하지 않는다: {:#?}", run.outcome);
    };
    assert!(matches!(answer, SaveAnswer::Uncertain { .. }));
    assert_eq!(unconfirmed, &withheld);
    assert_eq!(desk.calls(), (1, 1, 0));
    assert_eq!(desk.attempts(2), []);
}

// ── 시도를 만들지 않는 요청 ────────────────────────────────────────────────

#[test]
fn a_repeated_request_on_a_confirmed_event_makes_no_attempt_and_no_second_confirmation() {
    let lab = Lab::new("repeat", 0);
    let desk = lab.desk(Save::Store);
    let first = lab.retry(2, &desk).expect("시작된다");
    assert!(matches!(first.outcome, RetryOutcome::Confirmed { .. }));
    let (calls, observes, captures) = (desk.calls(), probe::observes(), probe::captures());
    let stored = desk.records().facts;

    for _ in 0..2 {
        let Err(NotStarted::AlreadyConfirmed { reading }) = lab.retry(2, &desk) else {
            panic!("이미 확정된 사건에는 시도를 만들지 않는다");
        };
        assert_eq!(
            reading.standing(),
            EventStanding::AlreadyConfirmed { snapshot: a(2), archived_at: c(1), archive_issuer: 2, attempts: 1 }
        );
    }
    assert_eq!((desk.calls(), probe::observes(), probe::captures()), (calls, observes, captures), "아무것도 다시 부르지 않았다");
    assert_eq!(desk.records().facts, stored);
    assert_eq!(desk.records().registry.next_id(), 3, "세계 이름이 다시 발급되지 않았다");
}

#[test]
fn queries_rebuilt_readings_and_out_of_scope_requests_call_no_observation_and_no_state() {
    let lab = Lab::new("queries", 1);
    let desk = lab.desk(Save::Store);
    let (observes, captures) = (probe::observes(), probe::captures());

    // 조회 — 몇 번을 읽어도, 같은 기록을 새로 복사해 읽어도(새 판정 인스턴스) 같은 읽기이고 아무것도 실행되지 않는다.
    let first = read_scope_change(m(2), &desk.records().inputs()).expect("읽힌다");
    for _ in 0..3 {
        let copied = desk.records();
        assert_eq!(read_scope_change(m(2), &copied.inputs()).expect("읽힌다"), first);
    }
    assert_eq!(first.standing(), EventStanding::Waiting { attempts: 1 });

    // 시작만 하고 실행하지 않은 시도 — 창고에서 원문과 기준 manifest 를 읽었을 뿐 관측도 저장 요청도 없다.
    let given = desk.records();
    let mut boundary = desk.clone();
    let idle = RetryAttempt::begin(m(2), given.inputs(), &lab.root, &mut boundary).expect("시작된다");
    assert!(matches!(idle.pending(), Some(RetryRequest::Observe { .. })));
    assert_eq!(idle.outcome(), None);
    drop(idle);

    // 보관 중단 표식이 있다고 건넨 기록, 시작 사건, 없는 사건 — 읽기가 돌려준 갈래 그대로이며 시도를 만들지 않는다.
    let mut halted = desk.records();
    halted.marks = ScopeMarks { archive_halt: MarkSeen::Present, unfinished_restore: MarkSeen::Absent };
    let mut boundary = desk.clone();
    assert_eq!(
        run_retry(m(2), halted.inputs(), &lab.root, &mut boundary).unwrap_err(),
        NotStarted::Reading(vec![ReadingProblem::OutOfScope(ScopeLimit::ArchiveHaltPresent)])
    );
    assert!(matches!(lab.retry(1, &desk), Err(NotStarted::Reading(_))));
    assert!(matches!(lab.retry(9, &desk), Err(NotStarted::Reading(_))));

    assert_eq!((probe::observes() - observes, probe::captures() - captures), (0, 0));
    assert_eq!(desk.calls(), (0, 0, 0));
    assert_eq!(desk.records().facts, lab.records.facts);
}

#[test]
fn a_retry_whose_policy_source_or_base_world_cannot_be_read_does_not_start() {
    // 측정 경계: `Lab::new` 는 마지막 확정 세계 A1 을 준비하려고 `capture_scoped` 를 한 번 부른다(fixture 준비이지 재시도가 아니다).
    // 그래서 경우마다 fixture 와 저장 경계를 **모두 준비한 뒤**, 검증하려는 `retry` 호출 바로 앞에서 기준값을 읽고 그 호출의 증가량만 본다.

    // 적용 정책 P2 의 원문이 창고에 없다 — 원장의 주소만 있고 객체가 없다.
    let mut lab = Lab::new("no-source", 0);
    let absent: &[u8] = b"never-stored/\n";
    lab.records.ledger =
        PolicyLedger::from_records(3, vec![record(1, P1_TEXT), record(2, absent)]).expect("시험 원장은 꼴이 맞다");
    let desk = lab.desk(Save::Store);
    // 여기까지가 준비다. 아래 한 호출이 측정 대상이다.
    let (observes, captures) = (probe::observes(), probe::captures());
    let Err(NotStarted::PolicySource { policy, .. }) = lab.retry(2, &desk) else {
        panic!("원문을 확인하지 못하면 시작하지 않는다");
    };
    assert_eq!(policy, p(2));
    assert_eq!((probe::observes() - observes, probe::captures() - captures), (0, 0), "원문 부재: 이 호출은 관측도 보관도 하지 않았다");
    assert_eq!(desk.calls(), (0, 0, 0), "원문 부재: 상태 저장·다시 읽기·실패 기록을 부르지 않았다");

    // 마지막 확정 세계의 이름은 등록부에 있지만 그 manifest 객체가 창고에 없다.
    let mut lab = Lab::new("no-base", 0);
    let nowhere = crate::ManifestAddress::from_parts("sha256", &[7u8; 32]).expect("꼴이 맞는 주소다");
    lab.records.registry = SnapshotRegistry::restore(2, vec![(1, nowhere)]).expect("시험 등록부는 꼴이 맞다");
    let other = lab.desk(Save::Store);
    // 여기까지가 준비다(위 `Lab::new` 의 fixture 보관 포함). 새 기준값을 읽고, 아래 한 호출이 측정 대상이다.
    let (observes, captures) = (probe::observes(), probe::captures());
    let Err(NotStarted::BaseWorld { world, .. }) = lab.retry(2, &other) else {
        panic!("기준 manifest 를 읽지 못하면 시작하지 않는다");
    };
    assert_eq!(world, a(1));
    assert_eq!((probe::observes() - observes, probe::captures() - captures), (0, 0), "기준 manifest 부재: 이 호출은 관측도 보관도 하지 않았다");
    assert_eq!(other.calls(), (0, 0, 0), "기준 manifest 부재: 상태 저장·다시 읽기·실패 기록을 부르지 않았다");
}

// ── 보고의 결속 ────────────────────────────────────────────────────────────

#[test]
fn reports_of_another_attempt_another_step_or_a_closed_attempt_are_refused_and_change_nothing() {
    let lab = Lab::new("tickets", 0);
    let desk = lab.desk(Save::Store);
    let given = desk.records();
    let (mut one, mut two) = (desk.clone(), desk.clone());
    let mut first = RetryAttempt::begin(m(2), given.inputs(), &lab.root, &mut one).expect("시작된다");
    let mut second = RetryAttempt::begin(m(2), given.inputs(), &lab.root, &mut two).expect("같은 입력으로 시작한 다른 시도다");

    let Some(RetryRequest::Observe { ticket: first_ticket }) = first.pending() else { panic!("첫 요청은 관측이다") };
    let Some(RetryRequest::Observe { ticket: second_ticket }) = second.pending() else { panic!("첫 요청은 관측이다") };
    assert_ne!(first_ticket, second_ticket, "시도마다 표가 다르다");

    // 첫 시도의 관측 보고 — 실제 관측의 결과다. 같은 걸음을 다시 실행하라는 요청은 거절되고 관측도 다시 일어나지 않는다.
    let observes = probe::observes();
    let observed = first.perform().expect("끝나지 않은 시도다");
    assert_eq!(observed.ticket(), first_ticket);
    assert_eq!(first.perform().err(), Some(RetryClosed::AlreadyPerformed { ticket: first_ticket }));
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
        let report = first.perform().expect("끝나지 않은 시도다");
        last = Some(report.clone());
        first.advance(report).expect("방금 낸 보고다");
    }
    let outcome = first.outcome().cloned().expect("끝났다");
    assert!(matches!(outcome, RetryOutcome::Confirmed { .. }), "{outcome:#?}");
    let last = last.expect("마지막 보고가 있다");
    assert_eq!(first.advance(last.clone()), Err(RetryOutOfOrder::AlreadyClosed { got: last.ticket() }));
    assert_eq!(first.advance(observed), Err(RetryOutOfOrder::AlreadyClosed { got: first_ticket }));
    assert_eq!(first.perform().err(), Some(RetryClosed::Finished));
    assert_eq!(first.outcome(), Some(&outcome), "끝난 시도의 결과는 바뀌지 않는다");
    assert_eq!(first.pending(), None);
    assert_eq!(second.pending(), Some(RetryRequest::Observe { ticket: second_ticket }), "다른 시도는 그동안 한 걸음도 가지 않았다");
}
