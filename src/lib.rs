extern crate mailparse;

use std::error;
use std::fmt;
use std::fs;
use std::io::prelude::*;
use std::ops::Deref;
use std::path::PathBuf;

use mailparse::*;

pub struct MailEntry {
    id: String,
    flags: String,
    path: PathBuf,
    data: Option<Vec<u8>>,
}

#[derive(Debug)]
pub enum MailEntryError {
    IOError(std::io::Error),
    ParseError(MailParseError),
}

impl fmt::Display for MailEntryError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            MailEntryError::IOError(ref err) => write!(f, "IO error: {}", err),
            MailEntryError::ParseError(ref err) => write!(f, "Parse error: {}", err),
        }
    }
}

impl error::Error for MailEntryError {
    fn description(&self) -> &str {
        match *self {
            MailEntryError::IOError(ref err) => err.description(),
            MailEntryError::ParseError(ref err) => err.description(),
        }
    }

    fn cause(&self) -> Option<&error::Error> {
        match *self {
            MailEntryError::IOError(ref err) => Some(err),
            MailEntryError::ParseError(ref err) => Some(err),
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
        parse_headers(self.data.as_ref().unwrap()).map(|(v, _)| v).map_err(|e| MailEntryError::ParseError(e))
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

pub struct MailEntries {
    path: PathBuf,
    subfolder: Subfolder,
    readdir: Option<fs::ReadDir>,
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

        loop { // we need to skip over files starting with a '.'
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

pub struct Maildir {
    path: PathBuf,
}

impl Maildir {
    pub fn count_new(&self) -> usize {
        self.list_new().count()
    }

    pub fn count_cur(&self) -> usize {
        self.list_cur().count()
    }

    pub fn list_new(&self) -> MailEntries {
        MailEntries {
            path: self.path.clone(),
            subfolder: Subfolder::New,
            readdir: None,
        }
    }

    pub fn list_cur(&self) -> MailEntries {
        MailEntries {
            path: self.path.clone(),
            subfolder: Subfolder::Cur,
            readdir: None,
        }
    }

    pub fn move_new_to_cur(&self, id: &str) -> std::io::Result<()> {
        let mut src = self.path.clone();
        src.push("new");
        src.push(id);
        let mut dst = self.path.clone();
        dst.push("cur");
        dst.push(String::from(id) + ":2,");
        fs::rename(src, dst)
    }

    pub fn find(&self, id: &str) -> Option<MailEntry> {
        let filter = |entry: &std::io::Result<MailEntry>| {
            match *entry {
                Err(_) => false,
                Ok(ref e) => e.id() == id,
            }
        };

        self.list_new().find(&filter).or_else(|| self.list_cur().find(&filter)).map(|e| e.unwrap())
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

impl <'a> From<&'a str> for Maildir {
    fn from(s: &str) -> Maildir {
        Maildir::from(PathBuf::from(s))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;

    use mailparse::MailHeaderMap;

    #[test]
    fn maildir_count() {
        let maildir = Maildir::from("testdata/maildir1");
        assert_eq!(maildir.count_cur(), 1);
        assert_eq!(maildir.count_new(), 1);
    }

    #[test]
    fn maildir_list() {
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
    }

    #[test]
    fn maildir_find() {
        let maildir = Maildir::from("testdata/maildir1");
        assert_eq!(maildir.find("bad_id").is_some(), false);
        assert_eq!(maildir.find("1463941010.5f7fa6dd4922c183dc457d033deee9d7").is_some(), true);
        assert_eq!(maildir.find("1463868505.38518452d49213cb409aa1db32f53184").is_some(), true);
    }

    #[test]
    fn mark_read() {
        let maildir = Maildir::from("testdata/maildir1");
        assert_eq!(maildir.move_new_to_cur("1463941010.5f7fa6dd4922c183dc457d033deee9d7").unwrap(), ());
        // Reset the filesystem
        fs::rename("testdata/maildir1/cur/1463941010.5f7fa6dd4922c183dc457d033deee9d7:2,",
                   "testdata/maildir1/new/1463941010.5f7fa6dd4922c183dc457d033deee9d7").unwrap();
    }
}
