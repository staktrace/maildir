use std::fs;
use std::path::PathBuf;

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
