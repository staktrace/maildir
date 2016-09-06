extern crate mailparse;

use std::fs;
use std::io::prelude::*;
use std::ops::Deref;
use std::path::PathBuf;

use mailparse::*;

pub struct MailEntry {
    id: String,
    data: Vec<u8>,
}

impl MailEntry {
    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn parsed(&self) -> Result<ParsedMail, MailParseError> {
        parse_mail(&self.data)
    }
}

pub struct MailEntries {
    readdir: fs::ReadDir,
}

impl Iterator for MailEntries {
    type Item = std::io::Result<MailEntry>;

    fn next(&mut self) -> Option<std::io::Result<MailEntry>> {
        let dir_entry = self.readdir.next();
        dir_entry.map(|e| {
            let entry = try!(e);
            let filename = String::from(entry.file_name().to_string_lossy().deref());
            let id = filename.split(":2,").next();
            if id.is_none() {
                return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "Non-maildir file found in maildir"));
            }
            let mut f = try!(fs::File::open(entry.path()));
            let mut d = Vec::<u8>::new();
            try!(f.read_to_end(&mut d));
            Ok(MailEntry{ id: String::from(id.unwrap()), data: d })
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
        Ok(MailEntries { readdir: dir })
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
        assert_eq!(first.parsed().unwrap().headers.get_first_value("Subject").unwrap(), Some(String::from("test")));
        let second = iter.next();
        assert!(second.is_none());
    }
}
