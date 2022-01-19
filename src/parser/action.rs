use std::io;
use std::path::PathBuf;
use std::str::FromStr;

pub type Folder = String;

#[derive(Debug, PartialEq, Eq)]
pub struct NotebkPath {
    pub folders: Vec<Folder>,
    pub number: Option<usize>,
}

impl NotebkPath {
    pub fn to_dir_path(&self, directory: &str) -> io::Result<PathBuf> {
        if self.number.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Expected a directory"),
            ));
        }
        self.inner_to_dir_path(directory)
    }

    pub fn inner_to_dir_path(&self, directory: &str) -> io::Result<PathBuf> {
        let mut path_buf = PathBuf::new();
        path_buf.push(&directory);
        for ref folder in &self.folders {
            path_buf.push(folder);
        }
        Ok(path_buf)
    }
}

impl FromStr for NotebkPath {
    type Err = ();
    fn from_str(s: &str) -> Result<NotebkPath, ()> {
        let mut splits: Vec<String> = s
            .split("/")
            .filter(|s| s.len() > 0)
            .map(|s| s.to_owned())
            .collect();
        let last = splits.last().and_then(|s| usize::from_str(&s).ok());
        Ok(match last {
            Some(n) => {
                splits.pop();
                NotebkPath {
                    folders: splits,
                    number: Some(n),
                }
            }
            _ => NotebkPath {
                folders: splits,
                number: None,
            },
        })
    }
}

impl Default for NotebkPath {
    fn default() -> Self {
        Self {
            folders: Vec::new(),
            number: None,
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Delete(NotebkPath),
    Which(NotebkPath),
    List(NotebkPath, usize),
    Move(NotebkPath, NotebkPath),
    Open(NotebkPath),
    Sync,
}
