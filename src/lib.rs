extern crate mailparse;

use std::fs;
use std::io::prelude::*;
use std::ops::Deref;
use std::path::PathBuf;

use mailparse::*;

pub struct MailEntry {
    id: String,
    flags: String,
    data: Vec<u8>,
}

impl MailEntry {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn parsed(&self) -> Result<ParsedMail, MailParseError> {
        parse_mail(&self.data)
    }

    pub fn headers(&self) -> Result<Vec<MailHeader>, MailParseError> {
        parse_headers(&self.data).map(|(v, _)| v)
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

pub struct MailEntries {
    readdir: fs::ReadDir,
    is_new: bool,
}

impl Iterator for MailEntries {
    type Item = std::io::Result<MailEntry>;

    fn next(&mut self) -> Option<std::io::Result<MailEntry>> {
        let dir_entry = self.readdir.next();
        dir_entry.map(|e| {
            let entry = try!(e);
            let filename = String::from(entry.file_name().to_string_lossy().deref());
            let (id, flags) = match self.is_new {
                true => (Some(filename.as_str()), Some("")),
                false => {
                    let mut iter = filename.split(":2,");
                    (iter.next(), iter.next())
                }
            };
            if id.is_none() || flags.is_none() {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData,
                                               "Non-maildir file found in maildir"));
            }
            let mut f = try!(fs::File::open(entry.path()));
            let mut d = Vec::<u8>::new();
            try!(f.read_to_end(&mut d));
            Ok(MailEntry {
                id: String::from(id.unwrap()),
                flags: String::from(flags.unwrap()),
                data: d,
            })
        })
    }
}

pub struct Maildir {
    path: PathBuf,
}

impl Maildir {
    fn path_new(&self) -> std::io::Result<fs::ReadDir> {
        let mut new_path = self.path.clone();
        new_path.push("new");
        fs::read_dir(new_path)
    }

    fn path_cur(&self) -> std::io::Result<fs::ReadDir> {
        let mut cur_path = self.path.clone();
        cur_path.push("cur");
        fs::read_dir(cur_path)
    }

    pub fn count_new(&self) -> std::io::Result<usize> {
        let dir = try!(self.path_new());
        Ok(dir.count())
    }

    pub fn count_cur(&self) -> std::io::Result<usize> {
        let dir = try!(self.path_cur());
        Ok(dir.count())
    }

    pub fn list_new(&self) -> std::io::Result<MailEntries> {
        let dir = try!(self.path_new());
        Ok(MailEntries {
            readdir: dir,
            is_new: true,
        })
    }

    pub fn list_cur(&self) -> std::io::Result<MailEntries> {
        let dir = try!(self.path_cur());
        Ok(MailEntries {
            readdir: dir,
            is_new: false,
        })
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

#[cfg(test)]
mod tests {
    use super::*;

    use std::fs;

    use mailparse::MailHeaderMap;

    #[test]
    fn maildir_count() {
        let maildir = Maildir::from(String::from("testdata/maildir1"));
        assert_eq!(maildir.count_cur().unwrap(), 1);
        assert_eq!(maildir.count_new().unwrap(), 1);
    }

    #[test]
    fn maildir_list() {
        let maildir = Maildir::from(String::from("testdata/maildir1"));
        let mut iter = maildir.list_new().unwrap();
        let first = iter.next().unwrap().unwrap();
        assert_eq!(first.id(), "1463941010.5f7fa6dd4922c183dc457d033deee9d7");
        assert_eq!(first.headers().unwrap().get_first_value("Subject").unwrap(),
                   Some(String::from("test")));
        assert_eq!(first.is_seen(), false);
        let second = iter.next();
        assert!(second.is_none());

        let mut iter = maildir.list_cur().unwrap();
        let first = iter.next().unwrap().unwrap();
        assert_eq!(first.id(), "1463868505.38518452d49213cb409aa1db32f53184");
        assert_eq!(first.parsed().unwrap().headers.get_first_value("Subject").unwrap(),
                   Some(String::from("test")));
        assert_eq!(first.is_seen(), true);
        let second = iter.next();
        assert!(second.is_none());
    }

    #[test]
    fn mark_read() {
        let maildir = Maildir::from(String::from("testdata/maildir1"));
        assert_eq!(maildir.move_new_to_cur("1463941010.5f7fa6dd4922c183dc457d033deee9d7").unwrap(), ());
        // Reset the filesystem
        fs::rename("testdata/maildir1/cur/1463941010.5f7fa6dd4922c183dc457d033deee9d7:2,",
                   "testdata/maildir1/new/1463941010.5f7fa6dd4922c183dc457d033deee9d7").unwrap();
    }
}
