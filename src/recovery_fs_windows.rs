//! Recovery I/O: pin every ancestor; open individual entries without reparse traversal.
use crate::windows_fs as win;
use std::{
    ffi::OsStr,
    fs::{self, File},
    io,
    path::Path,
};
use windows_sys::Win32::Storage::FileSystem::{DELETE, FILE_SHARE_READ};

pub(crate) struct Dir(win::Chain);
impl Dir {
    pub(crate) fn open(path: &Path) -> io::Result<Self> {
        Ok(Self(win::Chain::open(path)?))
    }
    pub(crate) fn path(&self) -> &Path {
        self.0.path()
    }
    pub(crate) fn volume(&self) -> io::Result<u64> {
        Ok(win::id(self.0.dir())?.volume)
    }
    pub(crate) fn child(&self, name: &str, create: bool) -> io::Result<Self> {
        let name = OsStr::new(name);
        let mut chain = win::Chain::open(self.path())?;
        if !win::same(chain.dir(), self.0.dir())? {
            return Err(win::refused());
        }
        if create {
            match win::create_dir_at(chain.dir(), name) {
                Ok(_) => win::flush_dir(chain.path(), chain.dir())?,
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e),
            }
        }
        chain.push(name, false)?;
        Ok(Self(chain))
    }
    pub(crate) fn sync(&self) -> io::Result<()> {
        win::flush_dir(self.path(), self.0.dir())
    }
    pub(crate) fn private_child(&self, name: &str) -> io::Result<Self> {
        match win::create_private_dir_at(self.0.dir(), OsStr::new(name)) {
            Ok(_) => self.sync()?,
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e),
        }
        let dir = self.child(name, false)?;
        win::private_dir(dir.0.dir())?;
        Ok(dir)
    }
    pub(crate) fn rename_file(&self, from: &str, to: &str) -> io::Result<()> {
        let f = win::open_entry_at(
            self.0.dir(),
            OsStr::new(from),
            win::READ | DELETE,
            win::SHARE_ALL,
            Some(false),
        )?;
        if !win::Stamp::of(&f)?.is_single_regular() {
            return Err(win::refused());
        }
        self.rename(&f, to, false)
    }
    pub(crate) fn read(&self, name: &str) -> io::Result<File> {
        let f = win::open_entry_at(
            self.0.dir(),
            OsStr::new(name),
            win::READ,
            win::SHARE_ALL,
            Some(false),
        )?;
        if !win::Stamp::of(&f)?.is_single_regular() {
            return Err(win::refused());
        }
        Ok(f)
    }
    pub(crate) fn create(&self, name: &str) -> io::Result<File> {
        win::create_file_at(
            self.0.dir(),
            OsStr::new(name),
            win::READ_WRITE_DELETE,
            win::SHARE_ALL,
        )
    }
    pub(crate) fn names(&self) -> io::Result<Vec<String>> {
        self.0.still_named()?;
        fs::read_dir(self.path())?
            .map(|e| e?.file_name().into_string().map_err(|_| win::refused()))
            .collect()
    }
    pub(crate) fn rename(&self, file: &File, name: &str, replace: bool) -> io::Result<()> {
        win::rename(file, self.0.dir(), self.path(), OsStr::new(name), replace)?;
        self.sync()
    }
    pub(crate) fn rename_dir(&self, from: &str, to: &str) -> io::Result<()> {
        let file = win::open_entry_at(
            self.0.dir(),
            OsStr::new(from),
            win::READ | DELETE,
            win::SHARE_ALL,
            Some(true),
        )?;
        self.rename(&file, to, false)
    }
    pub(crate) fn remove_dir(&self, name: &str) -> io::Result<()> {
        let f = win::open_entry_at(
            self.0.dir(),
            OsStr::new(name),
            win::READ | DELETE,
            win::SHARE_ALL,
            Some(true),
        )?;
        win::delete(&f)?;
        drop(f);
        self.sync()
    }
    pub(crate) fn delete_file(
        &self,
        name: &str,
        expected: &crate::artifact::ContentDigest,
    ) -> io::Result<()> {
        let f = win::open_entry_at(
            self.0.dir(),
            OsStr::new(name),
            win::READ | DELETE,
            FILE_SHARE_READ,
            Some(false),
        )?;
        if !win::Stamp::of(&f)?.is_single_regular()
            || crate::artifact::digest_of(f.try_clone()?)? != *expected
        {
            return Err(win::refused());
        }
        win::delete(&f)?;
        drop(f);
        self.sync()
    }
}
