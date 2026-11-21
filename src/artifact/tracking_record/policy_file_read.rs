//! 루트 `.gilignore` 한 파일의 **읽기 전용 실제 관측** — 3a 의 [`DiskReading`] 을 실제 디스크에서 만든다.
//!
//! [`DiskReading::from_read`] 는 호출자가 이미 읽은 결과를 옮길 뿐이다 — `fs::read` 의 `NotFound` 를 그대로 없음으로
//! 받으므로 「루트가 없다」「끊긴 연결 파일」「읽는 사이 이름이 바뀌었다」를 「유효한 루트에 정책 파일이 없다」와
//! 가르지 못한다. [`read_root_policy_file`] 은 아래 ①–⑥ 의 검사로 그 구분을 확인하고, 검사가 어긋남을 발견했거나
//! 검사를 마치지 못한 경우를 [`DiskReading::Unreadable`] 로 돌려준다. 검사 범위 밖의 변경까지 잡는다는 뜻은 아니다
//! (아래 「확인하는 범위와 한계」). 결과의 세 값과 그 뜻은 3a 그대로다.
//!
//! ```text
//! 호출자가 건넨 절대 경로의 루트        상대 경로면 읽지 않는다 — 현재 작업 폴더·환경 변수·부모 폴더로 추측하지 않는다
//!   ① 루트 자신을 연다                  Unix  open(O_DIRECTORY|O_NOFOLLOW) — 정책 엔진 트리 평가와 같은 문
//!                                       Windows 볼륨 뿌리부터 조상 고정(Chain), 모든 reparse point 거절
//!   ② `.gilignore` 의 종류를 따라가지 않고 본다
//!        없음        → ⑥ 으로(없음 후보)
//!        연결 파일·끊긴 연결·디렉터리·FIFO·소켓·장치 → 읽지 못함. 열지 않는다
//!        일반 파일    → ③
//!   ③ 그 이름을 연다                    Unix  openat(O_NOFOLLOW|O_NONBLOCK) 뒤 fstat — ② 에서 본 일반 파일과
//!                                             (장치, inode)가 같은가
//!                                       Windows reparse point 를 따라가지 않는 열기, 연 항목이 디렉터리·reparse 면 거절.
//!                                             ② 의 정체성을 보관하지 않으므로 ②→③ 사이 다른 일반 파일로 바뀐 것은 견주지 않는다
//!   ④ ③ 이 연 핸들로 처음부터 끝까지 한 번 흘려 읽는다(내용을 두 번 읽지 않는다). 그 핸들의 읽기 전·후 메타데이터
//!      (운영체제별 Stamp: 정체성·크기·수정/변경 시각·링크 수 등)가 같고 읽은 바이트 수가 크기와 같은가
//!      — 내용에 관한 마지막 검사다
//!   ⑤ 그 이름이 아직 ③ 에서 연 파일을 가리키는가 — 정체성만 견주고 내용·메타데이터는 다시 보지 않는다
//!   ⑥ 루트 경로가 아직 ① 에서 연 루트를 가리키는가 — 루트의 정체성만 견준다
//!   ⑦ 모든 검사가 맞으면 Present(④ 에서 읽은 바이트의 지문) 또는 Absent. 하나라도 어긋나거나 마치지 못하면 Unreadable
//! ```
//!
//! # 이 관측이 하지 않는 것
//!
//! - **쓰지 않는다.** `.gilignore`·`.gil`·`state.yaml`·객체 창고를 만들거나 고치지 않고, 잠금 파일·임시 파일·보관
//!   사본을 만들지 않는다. 프로젝트 잠금을 잡지 않는다 — 잠금과 엮는 일은 이것을 부를 후속 계층의 몫이다.
//!   운영체제가 읽기 때문에 접근 시각을 갱신할 수 있다. 그것까지 그대로라고 주장하지 않는다.
//! - **해석하지 않는다.** 바이트를 그대로 지문으로 만든다. 문자 인코딩·BOM·줄바꿈·공백·주석을 정규화하지 않고,
//!   빈 파일도 있는 파일이다. 원문 해석·정책 선택은 정책 엔진([`crate::artifact::gilignore`])의 일이다.
//! - **다른 파일을 읽지 않는다.** `.gitignore`, 하위 폴더의 `.gilignore`, 일반 작업 파일, 승인 원문 객체 창고를
//!   대신 읽지 않는다. 이름은 언제나 루트의 `.gilignore` 하나다.
//! - **판정·승인이 아니다.** 결과는 3a 의 [`DiskReading`] 값 그대로이며, 적용 정책·승인·`sync_base` 를 받지도 바꾸지도
//!   않는다. 이 결과를 들고 있다는 사실은 파일을 덮어써도 된다는 허가가 아니다 — §5.2 의 0단계(프로젝트 잠금 안의
//!   재확인)와 1–3단계는 여전히 후속 계층이 진다. 3a 판정표의 `UserChanged` 는 「적용 원문과도 기준과도 다르다」일 뿐
//!   작성자를 판별한 증거가 아니며, 이 관측도 작성자를 추정하지 않는다.
//!
//! # 확인하는 범위와 한계
//!
//! - **검사가 발견하는 것.** Unreadable 의 「바뀌었다」는 위 검사 하나가 어긋남을 **발견했다**는 뜻이다. 발견하는 것은
//!   다음뿐이다: (Unix) ② 와 ③ 사이 그 이름의 항목이 다른 항목·종류로 바뀌었거나 없어짐, (Windows) ③ 이 연 항목이
//!   디렉터리·reparse point 이거나 없어짐, ④ 의 읽기 전 메타데이터를 잰 때부터 읽기 후 메타데이터를 잴 때까지 연 파일의
//!   메타데이터가 달라졌거나 읽은 길이가 크기와 다름, ⑤ 의 순간 그 이름이 연 파일이 아닌 항목을 가리키거나 확인되지 않음,
//!   ⑥ 의 순간 루트 경로가 ① 의 루트가 아닌 폴더를 가리키거나 확인되지 않음. 이 밖의 변경은 탐지한다고 주장하지 않는다.
//! - **마지막 내용 검사 뒤의 틈.** 내용에 관한 검사는 ④ 의 읽기 후 메타데이터 비교가 마지막이다. 그 뒤 ⑤·⑥ 을 하는
//!   사이 같은 파일(같은 이름, 같은 정체성)의 내용이 바뀌면 ⑤·⑥ 은 그대로 맞고 결과는 Present(앞서 ④ 에서 읽은 바이트의
//!   지문)다. Present 는 함수가 끝나거나 값을 돌려받는 순간의 디스크 내용을 보증하지 않는다.
//! - **②→③ 구간의 운영체제 차이.** Unix 는 ② 에서 본 (장치, inode)를 ③ 이 연 항목과 견주므로 그사이 다른 일반 파일로
//!   바뀐 것을 잡는다(아래 번호 재사용은 제외). Windows 는 ② 에서 파일 ID 를 얻지 않으므로 그사이 다른 일반 파일로 바뀌어도
//!   견주지 않고 ③ 이 연 그 파일을 읽는다. Windows 의 정체성 비교는 ③ 이 연 핸들에 대한 ④ 의 전·후 비교와 ⑤ 의 「이름이
//!   그 핸들의 파일을 가리키는가」뿐이다. 그래서 Windows 의 Present 는 「② 에서 본 바로 그 파일」이 아니라 「③ 에서 안전하게
//!   연 일반 파일」의 바이트다.
//! - **없음의 시점.** Absent 는 ② 의 순간 그 이름의 항목이 없었고 ⑥ 의 순간 루트 경로가 아직 ① 의 루트를 가리켰다는
//!   뜻이다. ② 뒤에 생긴 `.gilignore` 는 다시 보지 않는다 — 반환 시점까지 계속 없었다고 보증하지 않는다.
//! - **메타데이터로 보는 내용 변경.** ④ 는 내용을 두 번 읽어 견주지 않고 크기·수정/변경 시각 등으로 본다 — 시각 해상도가
//!   거친 파일 시스템(예: FAT 계열)에서 같은 크기로 그 해상도 안에 바뀐 내용은 구별하지 못할 수 있다.
//! - **되돌림·관측 뒤 변경.** 두 검사 사이 또는 두 관측 사이에 바뀌었다가 같은 메타데이터·내용으로 되돌아온 변경, 그리고
//!   관측을 마친 뒤의 변경은 탐지한다고 주장하지 않는다. 그래서 이 값은 쓰기 직전의 재확인을 대신하지 않는다 — 후속 계층은
//!   프로젝트 잠금 안에서 다시 관측해야 한다(§5.2 0단계).
//! - **정체성.** Unix 는 (장치, inode), Windows 는 (볼륨 일련번호, 파일 ID)다. 지워진 항목의 번호가 같은 자리에서
//!   곧바로 재사용되는 경우는 구별하지 못한다.
//! - **루트 경로 계약은 기존 그대로다.** Unix 는 루트 자신이 연결 파일이면 열지 않고 조상의 연결은 운영체제가 푼다
//!   (정책 엔진 트리 평가와 같다). Windows 는 기존 `windows_fs::Chain` 대로 절대 드라이브 경로만 받고 모든 조상의
//!   reparse point 를 거절하며, 관측 동안 조상 폴더의 이름 바꾸기·삭제를 막는다. 두 운영체제에 같은 거절 정책을
//!   새로 만들지 않았다. 대소문자를 구분하지 않는 볼륨에서는 운영체제가 `.GILIGNORE` 같은 변형을 같은 이름으로
//!   열 수 있다 — 이름 변형의 판정은 정책 엔진 트리 평가가 한다.
//! - **하드 링크.** 링크 수가 1보다 큰 일반 파일도 일반 파일로 읽는다(정책 엔진 트리 평가와 같다). 다른 이름을 통한
//!   변경은 ④ 의 읽기 전·후 메타데이터 비교 구간에 메타데이터를 바꾼 것만 잡는다.

use std::fmt;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::Path;

use super::sync::DiskReading;
use crate::artifact::ContentDigest;

/// 관측하는 이름 — 루트의 정책 파일 하나뿐이다.
const POLICY_FILE: &str = ".gilignore";

/// 호출자가 명시한 프로젝트 루트의 `.gilignore` 하나를 **읽기만** 해서 3a 의 관측 값으로 돌려준다.
///
/// - `Present(지문)`: ③ 에서 연결을 따라가지 않고 안전하게 연 일반 파일에서 ④ 가 실제로 읽은 바이트(빈 파일 포함)의
///   지문이고, 구현된 검사 — ④ 의 읽기 전·후 메타데이터 비교, ⑤ 의 이름 정체성 확인, ⑥ 의 루트 정체성 확인 — 를
///   통과했다는 뜻이다. 반환 시점까지 내용이 그대로였다는 보증이 아니다: ④ 의 마지막 메타데이터 검사 뒤 ⑤·⑥ 사이에
///   같은 파일의 내용이 바뀌어도 앞서 읽은 지문을 돌려준다. Windows 는 ②→③ 사이의 일반 파일 교체를 견주지 않는다.
/// - `Absent`: ② 의 순간 처음 연 루트에 그 이름의 항목이 없었고, ⑥ 의 순간 루트 경로가 아직 그 루트를 가리켰다.
///   ② 뒤에 생긴 항목은 다시 보지 않는다.
/// - `Unreadable`: 그 밖의 모든 경우 — 상대 경로, 루트 없음·확인 실패, 연결 파일·끊긴 연결·디렉터리·특수 항목,
///   접근·읽기 오류, 검사가 발견한 교체·삭제·변경, 지원하지 않는 운영체제. **없음으로 바꾸지 않는다.**
///
/// 세 값 모두 관측 값일 뿐 승인·정책 적용·`sync_base` 확정·쓰기 허가가 아니다. 범위와 한계는 모듈 설명에 있다.
pub(crate) fn read_root_policy_file(project_root: &Path) -> DiskReading {
    match observe(project_root, &mut |_| {}) {
        Ok(Some(digest)) => DiskReading::Present(digest),
        Ok(None) => DiskReading::Absent,
        Err(problem) => DiskReading::Unreadable { reason: problem.to_string() },
    }
}

/// 관측 걸음 사이의 틈 — 실제 실행은 아무것도 하지 않고, **시험만** 이 틈으로 디스크를 흔든다.
/// 잠을 재우거나 스레드를 겹쳐 운에 맡기지 않으려는 자리다(`artifact::observe_twice` 와 같은 방식).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Pause {
    /// ① 루트를 연 뒤, ② 전.
    RootOpened,
    /// ② 가 일반 파일을 본 뒤, ③ 의 열기 전.
    EntrySeen,
    /// ③ 이 연 뒤(읽기 전 메타데이터를 잰 뒤), 읽기 전.
    FileOpened,
    /// ④ 의 읽기를 마친 뒤, 읽기 후 메타데이터를 재기 전.
    AfterRead,
    /// ④ 를 마친 뒤(내용에 관한 마지막 검사 뒤), ⑤ 전.
    BeforeNameRecheck,
    /// ⑥ 전(없음 후보에서는 ② 바로 뒤).
    BeforeRootRecheck,
}

/// 읽지 못한 까닭. 문장은 [`DiskReading::Unreadable`] 의 `reason` 이 된다.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ReadProblem {
    /// 루트가 절대 경로가 아니다. 현재 작업 폴더로 풀어 추측하지 않는다.
    RootNotAbsolute,
    /// 루트를 열지 못했다 — 없음, 디렉터리 아님, 루트 자신이 연결 파일, 권한, 경로 계약 거절 등.
    RootUnavailable { source: String },
    /// 정책 파일 자리의 항목이 일반 파일이 아니다. 따라가지도 열지도 않았다.
    NotRegular { kind: &'static str },
    /// 정책 파일 자리를 보거나 열거나 읽지 못했다.
    EntryUnavailable { source: String },
    /// 구현된 검사 하나가 관측 중의 어긋남을 발견했다 — 이 관측으로 파일을 확정하지 않는다. 검사 범위 밖의 변경은
    /// 이 값이 되지 않을 수 있다(모듈 설명 「확인하는 범위와 한계」).
    Changed { what: &'static str },
    /// 이 운영체제에는 핸들 기준 읽기 전용 관측이 없다.
    Unsupported,
}

impl fmt::Display for ReadProblem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ReadProblem::RootNotAbsolute => {
                f.write_str("프로젝트 루트가 절대 경로가 아니다 — 현재 작업 폴더로 추측하지 않는다")
            }
            ReadProblem::RootUnavailable { source } => write!(f, "프로젝트 루트를 확인하지 못했다: {source}"),
            ReadProblem::NotRegular { kind } => {
                write!(f, "루트 `.gilignore` 가 일반 파일이 아니다({kind}) — 따라가거나 열지 않았다")
            }
            ReadProblem::EntryUnavailable { source } => write!(f, "루트 `.gilignore` 를 읽지 못했다: {source}"),
            ReadProblem::Changed { what } => write!(f, "관측하는 동안 바뀌었다: {what}"),
            ReadProblem::Unsupported => f.write_str("이 운영체제에는 루트 `.gilignore` 의 안전한 읽기 전용 관측이 없다"),
        }
    }
}

/// 관측 한 번. `Ok(None)` 은 확인된 없음이다.
pub(super) fn observe(root: &Path, pause: &mut dyn FnMut(Pause)) -> Result<Option<ContentDigest>, ReadProblem> {
    if !root.is_absolute() {
        return Err(ReadProblem::RootNotAbsolute);
    }
    let held = os::Root::open(root).map_err(|error| match error.kind() {
        io::ErrorKind::Unsupported => ReadProblem::Unsupported,
        _ => ReadProblem::RootUnavailable { source: error.to_string() },
    })?;
    pause(Pause::RootOpened);

    let seen = match held.entry().map_err(unavailable)? {
        os::Entry::Missing => None,
        os::Entry::Other(kind) => return Err(ReadProblem::NotRegular { kind }),
        os::Entry::File(identity) => {
            pause(Pause::EntrySeen);
            let mut file = held.open_entry(&identity).map_err(|failure| match failure {
                os::OpenFailure::Changed => ReadProblem::Changed { what: "본 일반 파일과 연 항목이 다르다(교체·삭제·종류 변경)" },
                os::OpenFailure::Io(error) => unavailable(error),
            })?;
            let before = os::stamp(&file).map_err(unavailable)?;
            pause(Pause::FileOpened);
            let (digest, length) = read_from_start(&mut file).map_err(unavailable)?;
            pause(Pause::AfterRead);
            let after = os::stamp(&file).map_err(unavailable)?;
            if before != after || before.size() != length {
                return Err(ReadProblem::Changed { what: "읽는 동안 파일의 크기·시각·링크 수가 바뀌었다" });
            }
            pause(Pause::BeforeNameRecheck);
            if !held.still_names(&file).map_err(|_| ReadProblem::Changed { what: "읽은 뒤 그 이름을 다시 확인하지 못했다" })? {
                return Err(ReadProblem::Changed { what: "읽은 뒤 그 이름이 다른 항목을 가리킨다" });
            }
            Some(digest)
        }
    };
    pause(Pause::BeforeRootRecheck);
    match held.still_named(root) {
        Ok(true) => Ok(seen),
        Ok(false) => Err(ReadProblem::Changed { what: "루트 경로가 처음 연 루트를 가리키지 않는다" }),
        Err(_) => Err(ReadProblem::Changed { what: "관측을 마칠 때 루트를 다시 확인하지 못했다" }),
    }
}

fn unavailable(error: io::Error) -> ReadProblem {
    ReadProblem::EntryUnavailable { source: error.to_string() }
}

/// 같은 핸들의 처음부터 끝까지 흘려 읽은 지문과 바이트 수.
fn read_from_start(file: &mut std::fs::File) -> io::Result<(ContentDigest, u64)> {
    struct Counting<'a> {
        inner: &'a mut std::fs::File,
        count: u64,
    }
    impl Read for Counting<'_> {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            let read = self.inner.read(buf)?;
            self.count += read as u64;
            Ok(read)
        }
    }
    file.seek(SeekFrom::Start(0))?;
    let mut counting = Counting { inner: file, count: 0 };
    let digest = crate::artifact::digest_of(&mut counting)?;
    Ok((digest, counting.count))
}

// ── 운영체제별 ───────────────────────────────────────────────────────────

#[cfg(unix)]
mod os {
    use std::ffi::CString;
    use std::fs::File;
    use std::io;
    use std::os::fd::{AsRawFd, FromRawFd};
    use std::os::unix::fs::MetadataExt;
    use std::path::Path;

    /// (장치, inode).
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(super) struct Identity(u64, u64);

    pub(super) enum Entry {
        Missing,
        File(Identity),
        Other(&'static str),
    }

    pub(super) enum OpenFailure {
        Changed,
        Io(io::Error),
    }

    /// 읽기 전후로 같아야 하는 것: 정체성·종류·크기·수정 시각·상태 변경 시각·링크 수.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(super) struct Stamp {
        identity: Identity,
        mode: u32,
        size: u64,
        modified: (i64, i64),
        changed: (i64, i64),
        links: u64,
    }

    impl Stamp {
        pub(super) fn size(&self) -> u64 {
            self.size
        }
    }

    pub(super) fn stamp(file: &File) -> io::Result<Stamp> {
        let meta = file.metadata()?;
        Ok(Stamp {
            identity: Identity(meta.dev(), meta.ino()),
            mode: meta.mode(),
            size: meta.size(),
            modified: (meta.mtime(), meta.mtime_nsec()),
            changed: (meta.ctime(), meta.ctime_nsec()),
            links: meta.nlink(),
        })
    }

    fn name() -> CString {
        CString::new(super::POLICY_FILE).expect("고정 이름에 NUL 이 없다")
    }

    /// 열어 둔 루트 — 정책 파일은 **이 핸들 기준으로만** 보고 연다.
    pub(super) struct Root {
        dir: File,
    }

    impl Root {
        pub(super) fn open(path: &Path) -> io::Result<Root> {
            Ok(Root { dir: crate::internal_fs::dir_nofollow(path)? })
        }

        /// `fstatat(AT_SYMLINK_NOFOLLOW)` — 연결 파일을 연결 파일로 본다. 특수 항목은 열지 않는다.
        pub(super) fn entry(&self) -> io::Result<Entry> {
            // SAFETY: zeroed stat 은 유효한 출력 버퍼다.
            let mut st: libc::stat = unsafe { std::mem::zeroed() };
            if unsafe { libc::fstatat(self.dir.as_raw_fd(), name().as_ptr(), &mut st, libc::AT_SYMLINK_NOFOLLOW) } < 0 {
                let error = io::Error::last_os_error();
                return match error.kind() {
                    io::ErrorKind::NotFound => Ok(Entry::Missing),
                    _ => Err(error),
                };
            }
            let format = st.st_mode & libc::S_IFMT;
            Ok(if format == libc::S_IFREG {
                #[allow(clippy::unnecessary_cast)]
                Entry::File(Identity(st.st_dev as u64, st.st_ino as u64))
            } else if format == libc::S_IFLNK {
                Entry::Other("연결 파일")
            } else if format == libc::S_IFDIR {
                Entry::Other("디렉터리")
            } else if format == libc::S_IFIFO {
                Entry::Other("FIFO")
            } else if format == libc::S_IFSOCK {
                Entry::Other("소켓")
            } else if format == libc::S_IFBLK {
                Entry::Other("블록 장치")
            } else if format == libc::S_IFCHR {
                Entry::Other("문자 장치")
            } else {
                Entry::Other("지원하지 않는 종류")
            })
        }

        /// `openat(O_RDONLY | O_NOFOLLOW | O_NONBLOCK)` — 그사이 연결 파일로 바뀌었으면 열리지 않고, FIFO 로 바뀌었어도
        /// 쓰는 쪽을 기다리지 않는다. 연 뒤 fstat 이 ② 의 일반 파일과 같아야 한다.
        pub(super) fn open_entry(&self, seen: &Identity) -> Result<File, OpenFailure> {
            // SAFETY: NUL 로 끝나는 고정 이름, 성공한 fd 는 한 번만 소유한다.
            let fd = unsafe {
                libc::openat(
                    self.dir.as_raw_fd(),
                    name().as_ptr(),
                    libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
                )
            };
            if fd < 0 {
                let error = io::Error::last_os_error();
                return Err(match error.kind() {
                    io::ErrorKind::NotFound => OpenFailure::Changed,
                    _ if error.raw_os_error() == Some(libc::ELOOP) => OpenFailure::Changed,
                    _ => OpenFailure::Io(error),
                });
            }
            let file = unsafe { File::from_raw_fd(fd) };
            let meta = file.metadata().map_err(OpenFailure::Io)?;
            if !meta.file_type().is_file() || Identity(meta.dev(), meta.ino()) != *seen {
                return Err(OpenFailure::Changed);
            }
            Ok(file)
        }

        /// 그 이름이 아직 이 파일인가(같은 루트 핸들 기준, 따라가지 않고).
        pub(super) fn still_names(&self, file: &File) -> io::Result<bool> {
            let held = file.metadata()?;
            Ok(matches!(self.entry()?, Entry::File(now) if now == Identity(held.dev(), held.ino())))
        }

        /// 루트 경로를 같은 방식으로 다시 열어 처음 연 루트와 같은 디렉터리인가.
        pub(super) fn still_named(&self, path: &Path) -> io::Result<bool> {
            let again = crate::internal_fs::dir_nofollow(path)?;
            crate::internal_fs::same(&self.dir, &again)
        }
    }
}

#[cfg(windows)]
mod os {
    use std::ffi::OsStr;
    use std::fs::File;
    use std::io;
    use std::os::windows::fs::MetadataExt;
    use std::path::{Path, PathBuf};

    use crate::windows_fs::{self, Chain, SHARE_ALL};
    use windows_sys::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL, OPEN_EXISTING};

    /// WinNT.h `FILE_ATTRIBUTE_REPARSE_POINT`.
    const REPARSE_POINT: u32 = 0x400;

    /// ② 에서 종류를 볼 때는 파일 ID 를 얻지 않는다 — 그래서 ② 와 ③ 사이 다른 일반 파일로 바뀐 것은 견주지 않는다.
    /// 정체성은 ③ 이 연 핸들에서 재고, ④ 의 전·후 비교와 ⑤ 의 이름 확인에만 쓴다.
    pub(super) struct Identity;

    pub(super) enum Entry {
        Missing,
        File(Identity),
        Other(&'static str),
    }

    pub(super) enum OpenFailure {
        Changed,
        Io(io::Error),
    }

    /// 기존 Windows 저장 계약의 `Stamp` 그대로: 파일 ID·크기·링크 수·속성·생성/쓰기/변경 시각.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(super) struct Stamp(windows_fs::Stamp);

    impl Stamp {
        pub(super) fn size(&self) -> u64 {
            self.0.size
        }
    }

    pub(super) fn stamp(file: &File) -> io::Result<Stamp> {
        windows_fs::Stamp::of(file).map(Stamp)
    }

    /// 볼륨 뿌리부터 조상을 고정한 루트. 고정한 폴더는 관측 동안 이름 바꾸기·삭제가 막힌다.
    pub(super) struct Root {
        chain: Chain,
    }

    impl Root {
        pub(super) fn open(path: &Path) -> io::Result<Root> {
            Ok(Root { chain: Chain::open(path)? })
        }

        fn at(&self) -> io::Result<PathBuf> {
            self.chain.child(OsStr::new(super::POLICY_FILE))
        }

        /// reparse point 자신의 속성을 본다(정책 엔진 트리 평가와 같다). 모든 reparse point 를 연결로 본다.
        pub(super) fn entry(&self) -> io::Result<Entry> {
            let data = match std::fs::symlink_metadata(self.at()?) {
                Ok(data) => data,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Entry::Missing),
                Err(error) => return Err(error),
            };
            Ok(if data.file_attributes() & REPARSE_POINT != 0 {
                Entry::Other("연결 파일(reparse point)")
            } else if data.is_dir() {
                Entry::Other("디렉터리")
            } else if data.is_file() {
                Entry::File(Identity)
            } else {
                Entry::Other("지원하지 않는 종류")
            })
        }

        /// `FILE_FLAG_OPEN_REPARSE_POINT` 로 항목 자신을 연다(따라가지 않는다). 연 항목이 디렉터리·reparse point 면
        /// ② 뒤에 바뀐 것이다. 그사이 디렉터리로 바뀐 항목은 대개 열기 자체가 접근 거부로 실패한다(읽지 못함).
        /// 연 일반 파일이 ② 에서 본 그 파일인지는 견주지 않는다(위 `Identity`).
        pub(super) fn open_entry(&self, _: &Identity) -> Result<File, OpenFailure> {
            let at = self.at().map_err(OpenFailure::Io)?;
            let file = match windows_fs::open_raw(&at, windows_fs::READ, SHARE_ALL, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL) {
                Ok(file) => file,
                Err(error) if error.kind() == io::ErrorKind::NotFound => return Err(OpenFailure::Changed),
                Err(error) => return Err(OpenFailure::Io(error)),
            };
            let attributes = windows_fs::info(&file).map_err(OpenFailure::Io)?.dwFileAttributes;
            if attributes & (REPARSE_POINT | FILE_ATTRIBUTE_DIRECTORY) != 0 {
                return Err(OpenFailure::Changed);
            }
            Ok(file)
        }

        /// 그 이름을 같은 방식으로 다시 열어 같은 파일 ID 인가.
        pub(super) fn still_names(&self, file: &File) -> io::Result<bool> {
            let again = windows_fs::open_raw(&self.at()?, windows_fs::READ, SHARE_ALL, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL)?;
            Ok(windows_fs::id(&again)? == windows_fs::id(file)?)
        }

        /// 고정한 루트를 이름으로 다시 열어 같은 폴더인가(`Chain::still_named`). 거절은 오류로 돌아온다.
        pub(super) fn still_named(&self, _: &Path) -> io::Result<bool> {
            self.chain.still_named().map(|()| true)
        }
    }
}

#[cfg(not(any(unix, windows)))]
mod os {
    use std::fs::File;
    use std::io;
    use std::path::Path;

    pub(super) struct Identity;

    pub(super) enum Entry {
        Missing,
        File(Identity),
        Other(&'static str),
    }

    pub(super) enum OpenFailure {
        Changed,
        Io(io::Error),
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(super) struct Stamp;

    impl Stamp {
        pub(super) fn size(&self) -> u64 {
            0
        }
    }

    pub(super) fn stamp(_: &File) -> io::Result<Stamp> {
        Err(io::ErrorKind::Unsupported.into())
    }

    pub(super) struct Root;

    impl Root {
        pub(super) fn open(_: &Path) -> io::Result<Root> {
            Err(io::ErrorKind::Unsupported.into())
        }
        pub(super) fn entry(&self) -> io::Result<Entry> {
            Err(io::ErrorKind::Unsupported.into())
        }
        pub(super) fn open_entry(&self, _: &Identity) -> Result<File, OpenFailure> {
            Err(OpenFailure::Io(io::ErrorKind::Unsupported.into()))
        }
        pub(super) fn still_names(&self, _: &File) -> io::Result<bool> {
            Err(io::ErrorKind::Unsupported.into())
        }
        pub(super) fn still_named(&self, _: &Path) -> io::Result<bool> {
            Err(io::ErrorKind::Unsupported.into())
        }
    }
}
