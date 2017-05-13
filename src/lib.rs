extern crate mailparse;
extern crate nix;

use std::error;
use std::fmt;
use std::fs;
use std::io::prelude::*;
use std::os::unix::fs::MetadataExt;
use std::ops::Deref;
use std::path::PathBuf;
use std::thread;
use std::time;

use mailparse::*;

#[derive(Debug)]
pub enum MailEntryError {
    IOError(std::io::Error),
    ParseError(MailParseError),
    DateError(&'static str),
}

impl fmt::Display for MailEntryError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            MailEntryError::IOError(ref err) => write!(f, "IO error: {}", err),
            MailEntryError::ParseError(ref err) => write!(f, "Parse error: {}", err),
            MailEntryError::DateError(ref msg) => write!(f, "Date error: {}", msg),
        }
    }
}

impl error::Error for MailEntryError {
    fn description(&self) -> &str {
        match *self {
            MailEntryError::IOError(ref err) => err.description(),
            MailEntryError::ParseError(ref err) => err.description(),
            MailEntryError::DateError(ref msg) => msg,
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            MailEntryError::IOError(ref err) => Some(err),
            MailEntryError::ParseError(ref err) => Some(err),
            MailEntryError::DateError(_) => None,
        }
    }
}

impl From<std::io::Error> for MailEntryError {
    fn from(err: std::io::Error) -> MailEntryError {
        MailEntryError::IOError(err)
    }
}

impl From<MailParseError> for MailEntryError {
    fn from(err: MailParseError) -> MailEntryError {
        MailEntryError::ParseError(err)
    }
}

impl From<&'static str> for MailEntryError {
    fn from(err: &'static str) -> MailEntryError {
        MailEntryError::DateError(err)
    }
}

/// This struct represents a single email message inside
/// the maildir. Creation of the struct does not automatically
/// load the content of the email file into memory - however,
/// that may happen upon calling functions that require parsing
/// the email.
pub struct MailEntry {
    id: String,
    flags: String,
    path: PathBuf,
    data: Option<Vec<u8>>,
}

impl MailEntry {
    pub fn id(&self) -> &str {
        &self.id
    }

    fn read_data(&mut self) -> std::io::Result<()> {
        if self.data.is_none() {
            let mut f = try!(fs::File::open(self.path.clone()));
            let mut d = Vec::<u8>::new();
            try!(f.read_to_end(&mut d));
            self.data = Some(d);
        }
        Ok(())
    }

    pub fn parsed(&mut self) -> Result<ParsedMail, MailEntryError> {
        try!(self.read_data());
        parse_mail(self.data.as_ref().unwrap()).map_err(|e| MailEntryError::ParseError(e))
    }

    pub fn headers(&mut self) -> Result<Vec<MailHeader>, MailEntryError> {
        try!(self.read_data());
        parse_headers(self.data.as_ref().unwrap())
            .map(|(v, _)| v)
            .map_err(|e| MailEntryError::ParseError(e))
    }

    pub fn received(&mut self) -> Result<i64, MailEntryError> {
        try!(self.read_data());
        let headers = try!(self.headers());
        let received = try!(headers.get_first_value("Received"));
        match received {
            Some(v) => {
                for ts in v.rsplit(';') {
                    return dateparse(ts).map_err(MailEntryError::from);
                }
                try!(Err("Unable to split Received header"))
            }
            None => try!(Err("No Received header found")),
        }
    }

    pub fn flags(&self) -> &str {
        &self.flags
    }

    pub fn is_draft(&self) -> bool {
        self.flags.contains('D')
    }

    pub fn is_flagged(&self) -> bool {
        self.flags.contains('F')
    }

    pub fn is_passed(&self) -> bool {
        self.flags.contains('P')
    }

    pub fn is_replied(&self) -> bool {
        self.flags.contains('R')
    }

    pub fn is_seen(&self) -> bool {
        self.flags.contains('S')
    }

    pub fn is_trashed(&self) -> bool {
        self.flags.contains('T')
    }
}

enum Subfolder {
    New,
    Cur,
}

/// An iterator over the email messages in a particular
/// maildir subfolder (either `cur` or `new`). This iterator
/// produces a `std::io::Result<MailEntry>`, which can be an
/// `Err` if an error was encountered while trying to read
/// file system properties on a particular entry, or if an
/// invalid file was found in the maildir. Files starting with
/// a dot (.) character in the maildir folder are ignored.
pub struct MailEntries {
    path: PathBuf,
    subfolder: Subfolder,
    readdir: Option<fs::ReadDir>,
}

impl MailEntries {
    fn new(path: PathBuf, subfolder: Subfolder) -> MailEntries {
        MailEntries {
            path: path,
            subfolder: subfolder,
            readdir: None,
        }
    }
}

impl Iterator for MailEntries {
    type Item = std::io::Result<MailEntry>;

    fn next(&mut self) -> Option<std::io::Result<MailEntry>> {
        if self.readdir.is_none() {
            let mut dir_path = self.path.clone();
            dir_path.push(match self.subfolder {
                Subfolder::New => "new",
                Subfolder::Cur => "cur",
            });
            self.readdir = match fs::read_dir(dir_path) {
                Err(_) => return None,
                Ok(v) => Some(v),
            };
        }

        loop {
            // we need to skip over files starting with a '.'
            let dir_entry = self.readdir.iter_mut().next().unwrap().next();
            let result = dir_entry.map(|e| {
                let entry = try!(e);
                let filename = String::from(entry.file_name().to_string_lossy().deref());
                if filename.starts_with(".") {
                    return Ok(None);
                }
                let (id, flags) = match self.subfolder {
                    Subfolder::New => (Some(filename.as_str()), Some("")),
                    Subfolder::Cur => {
                        let mut iter = filename.split(":2,");
                        (iter.next(), iter.next())
                    }
                };
                if id.is_none() || flags.is_none() {
                    return Err(std::io::Error::new(std::io::ErrorKind::InvalidData,
                                                   "Non-maildir file found in maildir"));
                }
                Ok(Some(MailEntry {
                    id: String::from(id.unwrap()),
                    flags: String::from(flags.unwrap()),
                    path: entry.path(),
                    data: None,
                }))
            });
            return match result {
                None => None,
                Some(Err(e)) => Some(Err(e)),
                Some(Ok(None)) => continue,
                Some(Ok(Some(v))) => Some(Ok(v)),
            };
        }
    }
}

#[derive(Debug)]
pub enum MaildirError {
    Io(std::io::Error),
    Utf8(std::str::Utf8Error),
    Nix(nix::Error),
    Time(std::time::SystemTimeError),
}

impl fmt::Display for MaildirError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        use MaildirError::*;

        match *self {
            Io(ref e)   => write!(f, "IO Error: {}", e),
            Utf8(ref e) => write!(f, "UTF8 Encoding Error: {}", e),
            Nix(ref e)  => write!(f, "nix library Error: {}", e),
            Time(ref e) => write!(f, "Time Error: {}", e),
        }
    }
}

impl error::Error for MaildirError {
    fn description(&self) -> &str {
        use MaildirError::*;

        match *self {
            Io(ref e)   => e.description(),
            Utf8(ref e) => e.description(),
            Nix(ref e)  => e.description(),
            Time(ref e) => e.description(),
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        use MaildirError::*;

        match *self {
            Io(ref e)   => Some(e),
            Utf8(ref e) => Some(e),
            Nix(ref e)  => Some(e),
            Time(ref e) => Some(e),
        }
    }
}

impl From<std::io::Error> for MaildirError {
    fn from(e: std::io::Error) -> MaildirError {
        MaildirError::Io(e)
    }
}
impl From<std::str::Utf8Error> for MaildirError {
    fn from(e: std::str::Utf8Error) -> MaildirError {
        MaildirError::Utf8(e)
    }
}
impl From<nix::Error> for MaildirError {
    fn from(e: nix::Error) -> MaildirError {
        MaildirError::Nix(e)
    }
}
impl From<std::time::SystemTimeError> for MaildirError {
    fn from(e: std::time::SystemTimeError) -> MaildirError {
        MaildirError::Time(e)
    }
}

/// The main entry point for this library. This struct can be
/// instantiated from a path using the `from` implementations.
/// The path passed in to the `from` should be the root of the
/// maildir (the folder containing `cur`, `new`, and `tmp`).
pub struct Maildir {
    path: PathBuf,
}

impl Maildir {
    /// Returns the number of messages found inside the `new`
    /// maildir folder.
    pub fn count_new(&self) -> usize {
        self.list_new().count()
    }

    /// Returns the number of messages found inside the `cur`
    /// maildir folder.
    pub fn count_cur(&self) -> usize {
        self.list_cur().count()
    }

    /// Returns an iterator over the messages inside the `new`
    /// maildir folder. The order of messages in the iterator
    /// is not specified, and is not guaranteed to be stable
    /// over multiple invocations of this method.
    pub fn list_new(&self) -> MailEntries {
        MailEntries::new(self.path.clone(), Subfolder::New)
    }

    /// Returns an iterator over the messages inside the `cur`
    /// maildir folder. The order of messages in the iterator
    /// is not specified, and is not guaranteed to be stable
    /// over multiple invocations of this method.
    pub fn list_cur(&self) -> MailEntries {
        MailEntries::new(self.path.clone(), Subfolder::Cur)
    }

    /// Moves a message from the `new` maildir folder to the
    /// `cur` maildir folder. The id passed in should be
    /// obtained from the iterator produced by `list_new`.
    pub fn move_new_to_cur(&self, id: &str) -> std::io::Result<()> {
        let mut src = self.path.clone();
        src.push("new");
        src.push(id);
        let mut dst = self.path.clone();
        dst.push("cur");
        dst.push(String::from(id) + ":2,");
        fs::rename(src, dst)
    }

    /// Tries to find the message with the given id in the
    /// maildir. This searches both the `new` and the `cur`
    /// folders.
    pub fn find(&self, id: &str) -> Option<MailEntry> {
        let filter = |entry: &std::io::Result<MailEntry>| {
            match *entry {
                Err(_) => false,
                Ok(ref e) => e.id() == id,
            }
        };

        self.list_new().find(&filter).or_else(|| self.list_cur().find(&filter)).map(|e| e.unwrap())
    }

    /// Creates all neccessary directories if they don't exist yet. It is the library user's
    /// responsibility to call this before using `store_new`.
    pub fn create_dirs(&self) -> std::io::Result<()> {
        let mut path = self.path.clone();
        for d in vec!["cur", "new", "tmp"] {
            path.push(d);
            fs::create_dir_all(path.as_path())?;
            path.pop();
        }
        Ok( () )
    }

    /// Stores the given message data as a new message file in the Maildir `new` folder. Does not
    /// create the neccessary directories, so if in doubt call `create_dirs` before using
    /// `store_new`.
    pub fn store_new(&self, data: &[u8]) -> std::result::Result<(), MaildirError> {
        self.store(Subfolder::New, data, "")
    }

    /// Stores the given message data as a new message file in the Maildir `cur` folder, adding the
    /// given `flags` to it. The possible flags are explained e.g. at
    /// <https://cr.yp.to/proto/maildir.html> or <http://www.courier-mta.org/maildir.html>.
    pub fn store_cur_with_flags(&self, data: &[u8], flags: &str) -> std::result::Result<(), MaildirError> {
        self.store(Subfolder::Cur, data, &format!(":2,{}", flags))
    }

    fn store(&self, subfolder: Subfolder, data: &[u8], flags: &str) -> std::result::Result<(), MaildirError> {
        // try to get some uniquenes, as described at http://cr.yp.to/proto/maildir.html
        // dovecot and courier IMAP use <timestamp>.M<usec>P<pid>.<hostname> for tmp-files and then
        // move to <timestamp>.M<usec>P<pid>V<dev>I<ino>.<hostname>,S=<size_in_bytes> when moving
        // to new dir. see for example http://www.courier-mta.org/maildir.html
        let pid = nix::unistd::getpid();

        // note: gethostname(2) says that 64 bytes is the de-facto limit on linux, SUSv2 says limit
        // is 255.
        let mut hostname_buf = [0u8; 255];
        let hostname_cstr = nix::unistd::gethostname(&mut hostname_buf)?;
        let hostname = hostname_cstr.to_str()?;

        // loop when conflicting filenames occur, as described at
        // http://www.courier-mta.org/maildir.html
        // this assumes that pid and hostname don't change.
        let mut ts;
        let mut tmppath = self.path.clone();
        tmppath.push("tmp");
        loop {
            ts = time::SystemTime::now().duration_since(time::UNIX_EPOCH)?;
            tmppath.push(format!("{}.M{}P{}.{}", ts.as_secs(), ts.subsec_nanos(), pid, hostname));
            if !tmppath.exists() {
                break;
            }
            tmppath.pop();
            thread::sleep(time::Duration::from_millis(10));
        }

        let mut file = std::fs::File::create(tmppath.to_owned())?;
        file.write_all(data)?;
        file.sync_all()?;

        let meta = file.metadata()?;
        let mut newpath = self.path.clone();
        newpath.push(match subfolder {
            Subfolder::New => "new",
            Subfolder::Cur => "cur",
        });
        newpath.push(format!("{}.M{}P{}V{}I{}.{},S={}{}", ts.as_secs(), ts.subsec_nanos(), pid, meta.dev(), meta.ino(), hostname, meta.size(), flags));
        std::fs::rename(tmppath, newpath)?;

        Ok( () )
    }
}

impl From<PathBuf> for Maildir {
    fn from(p: PathBuf) -> Maildir {
        Maildir { path: p }
    }
}

impl From<String> for Maildir {
    fn from(s: String) -> Maildir {
        Maildir::from(PathBuf::from(s))
    }
}

impl<'a> From<&'a str> for Maildir {
    fn from(s: &str) -> Maildir {
        Maildir::from(PathBuf::from(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;
    use std::path::*;

    use mailparse::MailHeaderMap;

    // `cargo package` doesn't package files with colons in the
    // name, so we have to resort to naming it something else
    // and renaming for the tests. Talk about ugly!
    fn setup() {
        fs::rename("testdata/maildir1/cur/1463868505.38518452d49213cb409aa1db32f53184_2_S",
                   "testdata/maildir1/cur/1463868505.38518452d49213cb409aa1db32f53184:2,S")
            .unwrap();
    }

    fn teardown() {
        fs::rename("testdata/maildir1/cur/1463868505.38518452d49213cb409aa1db32f53184:2,S",
                   "testdata/maildir1/cur/1463868505.38518452d49213cb409aa1db32f53184_2_S")
            .unwrap();
    }

    #[test]
    fn maildir_count() {
        setup();
        let maildir = Maildir::from("testdata/maildir1");
        assert_eq!(maildir.count_cur(), 1);
        assert_eq!(maildir.count_new(), 1);
        teardown();
    }

    #[test]
    fn maildir_list() {
        setup();
        let maildir = Maildir::from("testdata/maildir1");
        let mut iter = maildir.list_new();
        let mut first = iter.next().unwrap().unwrap();
        assert_eq!(first.id(), "1463941010.5f7fa6dd4922c183dc457d033deee9d7");
        assert_eq!(first.headers().unwrap().get_first_value("Subject").unwrap(),
                   Some(String::from("test")));
        assert_eq!(first.is_seen(), false);
        let second = iter.next();
        assert!(second.is_none());

        let mut iter = maildir.list_cur();
        let mut first = iter.next().unwrap().unwrap();
        assert_eq!(first.id(), "1463868505.38518452d49213cb409aa1db32f53184");
        assert_eq!(first.parsed().unwrap().headers.get_first_value("Subject").unwrap(),
                   Some(String::from("test")));
        assert_eq!(first.is_seen(), true);
        let second = iter.next();
        assert!(second.is_none());
        teardown();
    }

    #[test]
    fn maildir_find() {
        setup();
        let maildir = Maildir::from("testdata/maildir1");
        assert_eq!(maildir.find("bad_id").is_some(), false);
        assert_eq!(maildir.find("1463941010.5f7fa6dd4922c183dc457d033deee9d7").is_some(),
                   true);
        assert_eq!(maildir.find("1463868505.38518452d49213cb409aa1db32f53184").is_some(),
                   true);
        teardown();
    }

    #[test]
    fn mark_read() {
        setup();
        let maildir = Maildir::from("testdata/maildir1");
        assert_eq!(maildir.move_new_to_cur("1463941010.5f7fa6dd4922c183dc457d033deee9d7").unwrap(),
                   ());
        // Reset the filesystem
        fs::rename("testdata/maildir1/cur/1463941010.5f7fa6dd4922c183dc457d033deee9d7:2,",
                   "testdata/maildir1/new/1463941010.5f7fa6dd4922c183dc457d033deee9d7")
            .unwrap();
        teardown();
    }

    #[test]
    fn check_received() {
        setup();
        let maildir = Maildir::from("testdata/maildir1");
        let mut iter = maildir.list_cur();
        let mut first = iter.next().unwrap().unwrap();
        assert_eq!(first.received().unwrap(), 1463868507);
        teardown();
    }

    #[test]
    fn check_create_dirs() {
        let maildir = Maildir::from("testdata/maildir2");
        assert!(!Path::new("testdata/maildir2").exists());
        assert!(!Path::new("testdata/maildir2/cur").exists());
        assert!(!Path::new("testdata/maildir2/new").exists());
        assert!(!Path::new("testdata/maildir2/tmp").exists());

        maildir.create_dirs().unwrap();
        assert!(Path::new("testdata/maildir2").exists());
        assert!(Path::new("testdata/maildir2/cur").exists());
        assert!(Path::new("testdata/maildir2/new").exists());
        assert!(Path::new("testdata/maildir2/tmp").exists());

        fs::remove_dir_all("testdata/maildir2").unwrap();
    }


    const TEST_MAIL_BODY : &'static [u8] = b"Return-Path: <of82ecuq@cip.cs.fau.de>
X-Original-To: of82ecuq@cip.cs.fau.de
Delivered-To: of82ecuq@cip.cs.fau.de
Received: from faui0fl.informatik.uni-erlangen.de (unknown [IPv6:2001:638:a000:4160:131:188:60:117])
        by faui03.informatik.uni-erlangen.de (Postfix) with ESMTP id 466C1240A3D
        for <of82ecuq@cip.cs.fau.de>; Fri, 12 May 2017 10:09:45 +0000 (UTC)
Received: by faui0fl.informatik.uni-erlangen.de (Postfix, from userid 303135)
        id 389CC10E1A32; Fri, 12 May 2017 12:09:45 +0200 (CEST)
To: of82ecuq@cip.cs.fau.de
MIME-Version: 1.0
Content-Type: text/plain; charset=\"UTF-8\"
Content-Transfer-Encoding: 8bit
Message-Id: <20170512100945.389CC10E1A32@faui0fl.informatik.uni-erlangen.de>
Date: Fri, 12 May 2017 12:09:45 +0200 (CEST)
From: of82ecuq@cip.cs.fau.de (Johannes Schilling)
Subject: maildir delivery test mail

Today is Boomtime, the 59th day of Discord in the YOLD 3183";

    #[test]
    fn check_store_new() {
        let maildir = Maildir::from("testdata/maildir2");
        maildir.create_dirs().unwrap();

        assert_eq!(maildir.count_new(), 0);
        maildir.store_new(TEST_MAIL_BODY).unwrap();
        assert_eq!(maildir.count_new(), 1);

        fs::remove_dir_all("testdata/maildir2").unwrap();
    }

    #[test]
    fn check_store_cur() {
        let maildir = Maildir::from("testdata/maildir2");
        maildir.create_dirs().unwrap();
        let testflags = "FRS";

        assert_eq!(maildir.count_cur(), 0);
        maildir.store_cur_with_flags(TEST_MAIL_BODY, testflags).unwrap();
        assert_eq!(maildir.count_cur(), 1);

        let mut iter = maildir.list_cur();
        let first = iter.next().unwrap().unwrap();
        assert_eq!(first.flags(), testflags);

        fs::remove_dir_all("testdata/maildir2").unwrap();
    }
}
