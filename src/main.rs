extern crate chrono;
extern crate dirs;

use std::borrow::ToOwned;
use std::env;
use std::fs;
use std::io;
use std::io::BufRead;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;

fn read_file<P: AsRef<Path>>(path: P) -> io::Result<String> {
    fs::File::open(path.as_ref()).and_then(|mut f| {
        let mut contents = String::new();
        f.read_to_string(&mut contents).map(|_| {
            let l = contents.as_str().trim_end().len();
            contents.truncate(l);
            contents
        })
    })
}

fn list(dir_path: &Path, n: usize) -> io::Result<()> {
    if dir_path.exists() {
        for (i, entry) in entries(dir_path)?.into_iter().take(n).enumerate() {
            if entry.file_type()?.is_file() {
                let first_line = std::io::BufReader::new(fs::File::open(entry.path())?)
                    .lines()
                    .filter_map(Result::ok)
                    .filter(|r| r.len() > 0)
                    .next();
                println!(
                    "{:2}  {}",
                    i + 1,
                    first_line.as_ref().map(String::as_str).unwrap_or("<empty>")
                );
            } else {
                println!("{:2}  {}/", i + 1, entry.file_name().to_string_lossy());
            }
        }
        Ok(())
    } else {
        println!("That doesn't exist");
        Ok(())
    }
}

fn entries(dir_path: &Path) -> io::Result<Vec<fs::DirEntry>> {
    let mut listing = Vec::new();
    for entry_result in fs::read_dir(&dir_path)? {
        listing.push(entry_result?);
    }
    listing
        .as_mut_slice()
        .sort_unstable_by_key(|e| e.file_name());
    listing.as_mut_slice().reverse();
    Ok(listing)
}

fn get_directory() -> io::Result<String> {
    dirs::config_dir()
        .and_then(|d| read_file(d.join("notebk")).ok())
        .or_else(|| dirs::home_dir().and_then(|d| read_file(d.join(".notebk")).ok()))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "Please place a notebk file in your $XDG_CONFIG_DIR",
            )
        })
}

type Folder = String;

#[derive(Debug, PartialEq, Eq)]
struct NotebkPath {
    folders: Vec<Folder>,
    number: Option<usize>,
}

impl NotebkPath {
    pub fn to_file_path(&self, directory: &str) -> io::Result<PathBuf> {
        let mut path_buf = self.inner_to_dir_path(directory)?;
        match self.number {
            Some(n) => {
                let entry = entries(&path_buf)?
                    .into_iter()
                    .nth(n - 1)
                    .ok_or(io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("There is no entry number {}", n),
                    ))?;
                path_buf.push(entry.file_name())
            }
            None => {
                path_buf.push(format!("{}", chrono::Local::now().format("%Y-%m-%d.md")));
            }
        }
        Ok(path_buf)
    }

    pub fn to_dir_path(&self, directory: &str) -> io::Result<PathBuf> {
        if self.number.is_some() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("Expected a directory"),
            ));
        }
        self.inner_to_dir_path(directory)
    }

    fn inner_to_dir_path(&self, directory: &str) -> io::Result<PathBuf> {
        let mut path_buf = PathBuf::new();
        path_buf.push(&directory);
        for ref folder in &self.folders {
            path_buf.push(folder);
        }
        Ok(path_buf)
    }
}

fn cleanup(mut deleted_file: &Path) -> io::Result<()> {
    loop {
        deleted_file = match deleted_file.parent() {
            Some(ref p) => p,
            None => break,
        };
        let listing = fs::read_dir(deleted_file)?;
        if listing.count() > 0 {
            break;
        }
        fs::remove_dir(deleted_file)?;
    }
    Ok(())
}

fn make_writable(file: &Path) -> io::Result<()> {
    match file.parent() {
        Some(ref p) => fs::create_dir_all(p),
        None => Ok(()),
    }
}

fn verify_is_file(file: &Path) -> io::Result<()> {
    if !file.is_file() {
        Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Expected an existing file"),
        ))
    } else {
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Action {
    Delete(NotebkPath),
    Which(NotebkPath),
    List(NotebkPath, usize),
    Move(NotebkPath, NotebkPath),
    Open(NotebkPath),
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

impl Action {
    fn execute(self) -> io::Result<()> {
        let base = get_directory()?;
        match self {
            Action::Delete(notebk_path) => {
                let file_path = notebk_path.to_file_path(&base)?;
                verify_is_file(&file_path)?;
                fs::remove_file(&file_path)?;
                cleanup(&file_path)?;
                Ok(())
            }
            Action::Which(notebk_path) => {
                let file_path = notebk_path.to_file_path(&base)?;
                println!("{}", file_path.to_string_lossy());
                Ok(())
            }
            Action::List(notebk_path, n) => {
                let dir_path = notebk_path.to_dir_path(&base)?;
                list(&dir_path, n)
            }
            Action::Open(notebk_path) => {
                let file_path = notebk_path.to_file_path(&base)?;
                make_writable(&file_path)?;
                std::process::Command::new("vim").arg(&file_path).status()?;
                cleanup(&file_path)
            }
            Action::Move(src_notebk_path, dst_notebk_path) => {
                let src_path = src_notebk_path.to_file_path(&base)?;
                let dst_dir = dst_notebk_path.to_dir_path(&base)?;
                let dst_path = dst_dir.join(src_path.file_name().unwrap());
                if dst_path.exists() {
                    Err(io::Error::new(
                        io::ErrorKind::NotFound,
                        format!("Destination {:?} exists", dst_path),
                    ))
                } else {
                    fs::create_dir_all(dst_dir)?;
                    fs::rename(src_path, dst_path)
                }
            }
        }
    }

    fn parse(arguments: impl Iterator<Item = String>) -> Result<Self, ()> {
        let args: Vec<_> = arguments.collect();
        match args.get(0).map(String::as_str) {
            None => Ok(Action::Open(NotebkPath::from_str("")?)),
            Some("delete") => {
                if args.len() != 2 {
                    Err(())
                } else {
                    Ok(Action::Delete(NotebkPath::from_str(&args[1])?))
                }
            }
            Some("which") => {
                if args.len() != 2 {
                    Err(())
                } else {
                    Ok(Action::Which(NotebkPath::from_str(&args[1])?))
                }
            }
            Some("ls") => {
                if args.len() == 1 {
                    Ok(Action::List(
                        NotebkPath {
                            folders: Vec::new(),
                            number: None,
                        },
                        10,
                    ))
                } else if args.len() == 2 {
                    match usize::from_str(&args[1]) {
                        Ok(n) => Ok(Action::List(
                            NotebkPath {
                                folders: Vec::new(),
                                number: None,
                            },
                            n,
                        )),
                        Err(_) => Ok(Action::List(NotebkPath::from_str(&args[1])?, 10)),
                    }
                } else if args.len() == 3 {
                    Ok(Action::List(
                        NotebkPath::from_str(&args[1])?,
                        usize::from_str(&args[2]).map_err(|_| ())?,
                    ))
                } else {
                    Err(())
                }
            }
            Some("mv") => {
                if args.len() != 3 {
                    Err(())
                } else {
                    Ok(Action::Move(
                        NotebkPath::from_str(&args[1])?,
                        NotebkPath::from_str(&args[2])?,
                    ))
                }
            }
            Some(p) => {
                if args.len() != 1 {
                    Err(())
                } else {
                    Ok(Action::Open(NotebkPath::from_str(p)?))
                }
            }
        }
    }
}

fn main() {
    let args = env::args().skip(1);
    let action = match Action::parse(args) {
        Ok(a) => a,
        Err(()) => {
            eprintln!("Could not parse");
            std::process::exit(2)
        }
    };
    std::process::exit(
        action
            .execute()
            .map_err(|e| println!("Error: {}", e))
            .map(|_| 0)
            .unwrap_or(1),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_which() {
        let input = vec!["which", "4"];
        let actual = Action::parse(input.into_iter().map(ToOwned::to_owned)).unwrap();
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
        let input = vec!["which", "food/dessert/4"];
        let actual = Action::parse(input.into_iter().map(ToOwned::to_owned)).unwrap();
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
        let input = vec!["puzzles/math/4"];
        let actual = Action::parse(input.into_iter().map(ToOwned::to_owned)).unwrap();
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
        let input = vec!["ls"];
        let actual = Action::parse(input.into_iter().map(ToOwned::to_owned)).unwrap();
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
