use std::fs;
use std::path::PathBuf;

pub struct Maildir {
    path: PathBuf,
}

impl Maildir {
    pub fn count_new(&self) -> std::io::Result<usize> {
        let mut new_path = self.path.clone();
        new_path.push("new");
        let dir = try!(fs::read_dir(new_path));
        Ok(dir.count())
    }

    pub fn count_cur(&self) -> std::io::Result<usize> {
        let mut new_path = self.path.clone();
        new_path.push("cur");
        let dir = try!(fs::read_dir(new_path));
        Ok(dir.count())
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
    #[test]
    fn it_works() {
    }
}
