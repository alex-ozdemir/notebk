use std::io;
use std::path::PathBuf;
use std::str::FromStr;

mod args;

use self::args::Args;

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

#[derive(Debug, PartialEq, Eq)]
pub enum Action {
    Delete(NotebkPath),
    Which(NotebkPath),
    List(NotebkPath, usize),
    Move(NotebkPath, NotebkPath),
    Open(NotebkPath),
}

impl Action {
    pub fn parse(args: Args) -> Result<Self, ()> {
        Ok(if args.cmd_ls {
            Action::List(
                NotebkPath::from_str(&args.arg_path.unwrap_or_else(String::new))?,
                args.arg_count.unwrap_or(10),
            )
        } else if args.cmd_which {
            Action::Which(NotebkPath::from_str(&args.arg_path.unwrap())?)
        } else if args.cmd_mv {
            Action::Move(
                NotebkPath::from_str(&args.arg_src.unwrap())?,
                NotebkPath::from_str(&args.arg_dst.unwrap())?,
            )
        } else if args.cmd_delete {
            Action::Delete(NotebkPath::from_str(&args.arg_path.unwrap())?)
        } else {
            Action::Open(NotebkPath::from_str(&args.arg_path.unwrap())?)
        })
    }

    pub fn from_args() -> Result<Self, ()> {
        Self::parse(args::get_args())
    }
}

#[cfg(test)]
mod tests {
    use docopt::Docopt;

    use super::*;

    use super::args::USAGE;

    #[test]
    fn parse_which() {
        let input = vec!["notebk", "4", "which"];
        let actual = Action::parse(
            Docopt::new(USAGE)
                .unwrap()
                .argv(input)
                .deserialize()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            Action::Which(NotebkPath {
                folders: Vec::new(),
                number: Some(4)
            }),
            actual
        );
    }

    #[test]
    fn parse_which_path() {
        let input = vec!["notebk", "food/dessert/4", "which"];
        let actual = Action::parse(
            Docopt::new(USAGE)
                .unwrap()
                .argv(input)
                .deserialize()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            Action::Which(NotebkPath {
                folders: vec!["food".to_owned(), "dessert".to_owned()],
                number: Some(4),
            }),
            actual
        );
    }

    #[test]
    fn parse_open_path() {
        let input = vec!["notebk", "puzzles/math/4"];
        let actual = Action::parse(
            Docopt::new(USAGE)
                .unwrap()
                .argv(input)
                .deserialize()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            Action::Open(NotebkPath {
                folders: vec!["puzzles".to_owned(), "math".to_owned()],
                number: Some(4),
            }),
            actual
        );
    }

    #[test]
    fn parse_null_list() {
        let input = vec!["notebk", "ls"];
        let actual = Action::parse(
            Docopt::new(USAGE)
                .unwrap()
                .argv(input)
                .deserialize()
                .unwrap(),
        )
        .unwrap();
        assert_eq!(
            Action::List(
                NotebkPath {
                    folders: vec![],
                    number: None,
                },
                10
            ),
            actual
        );
    }
}
