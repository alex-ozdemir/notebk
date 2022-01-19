use std::process::exit;
use std::str::FromStr;

pub mod action;
use self::action::*;

const DEFAULT_LS_LENGTH: usize = 10;
pub const USAGE: &'static str = "
notebk

Usage:
  notebk sync
  notebk ls [<count>]
  notebk <path> ls [<count>]
  notebk <path> which
  notebk <path> delete
  notebk <path>
  notebk
  notebk mv <src> <dst>
  notebk -h | --help

Options:
  -h --help  Show this screen.

Actions:
  ls      list up to <count> (default 10) items from <path>

  which   identify the filesystem path to <path>

  delete  delete the entry at <path>

  mv      move the entry at <src> to <dst>

  <path>  open the entry at <path>

  sync    assuming that *notebook path* is a git repository:
          1. Pulls
          2. Commits
          3. Pushes

Configuration:

  *notebook path* The path of your notebook's root.
                  Fetched from $XDG_CONFIG_DIR/notebk.
";

pub enum Keyword {
    Ls,
    Sync,
    Which,
    Delete,
    Mv,
}

impl Keyword {
    fn parse(s: &str) -> Option<Keyword> {
        match s {
            "ls" => Some(Keyword::Ls),
            "sync" => Some(Keyword::Sync),
            "which" => Some(Keyword::Which),
            "delete" => Some(Keyword::Delete),
            "mv" => Some(Keyword::Mv),
            _ => None,
        }
    }
}

pub fn get_args_or_exit() -> Action {
    let args: Vec<String> = std::env::args().collect();
    if let Some(a) = get_args_opt(args) {
        a
    } else {
        println!("{}", USAGE);
        exit(2);
    }
}

fn get_args_opt(mut args: Vec<String>) -> Option<Action> {
    use self::Keyword::*;
    args.remove(0);
    let mut keywords: Vec<Keyword> = Vec::new();
    let ls_idx = args.iter().position(|a| a == "ls");
    args.retain(|a| {
        if let Some(k) = Keyword::parse(a) {
            keywords.push(k);
            false
        } else {
            true
        }
    });
    let shift_path = |args: &mut Vec<String>| -> Option<NotebkPath> {
        NotebkPath::from_str(&args.remove(0)).ok()
    };
    match keywords.as_slice() {
        [Sync] => (args.len() == 0).then(|| Action::Sync),
        [Mv] => {
            if args.len() == 2 {
                Some(Action::Move(shift_path(&mut args)?, shift_path(&mut args)?))
            } else {
                None
            }
        }
        [Which] => {
            if args.len() == 1 {
                Some(Action::Which(shift_path(&mut args)?))
            } else {
                None
            }
        }
        [Delete] => {
            if args.len() == 1 {
                Some(Action::Delete(shift_path(&mut args)?))
            } else {
                None
            }
        }
        [] => {
            if args.len() == 1 {
                Some(Action::Open(shift_path(&mut args)?))
            } else {
                Some(Action::Open(NotebkPath::default()))
            }
        }
        [Ls] => {
            let idx = ls_idx.unwrap();
            match args.len() {
                0 => Some(Action::List(NotebkPath::default(), DEFAULT_LS_LENGTH)),
                1 => {
                    if idx == 0 {
                        Some(Action::List(
                            NotebkPath::default(),
                            usize::from_str(&args[0]).ok()?,
                        ))
                    } else {
                        Some(Action::List(
                            NotebkPath::from_str(&args[0]).ok()?,
                            DEFAULT_LS_LENGTH,
                        ))
                    }
                }
                2 => Some(Action::List(
                    shift_path(&mut args)?,
                    usize::from_str(&args[0]).ok()?,
                )),
                _ => None,
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parse_which() {
        let input = vec!["notebk", "4", "which"];
        let actual = get_args_opt(input.into_iter().map(ToString::to_string).collect());
        assert_eq!(
            Some(Action::Which(NotebkPath {
                folders: Vec::new(),
                number: Some(4)
            })),
            actual
        );
    }

    #[test]
    fn parse_which_path() {
        let input = vec!["notebk", "food/dessert/4", "which"];
        let actual = get_args_opt(input.into_iter().map(ToString::to_string).collect());
        assert_eq!(
            Some(Action::Which(NotebkPath {
                folders: vec!["food".to_owned(), "dessert".to_owned()],
                number: Some(4),
            })),
            actual
        );
    }

    #[test]
    fn parse_open_path() {
        let input = vec!["notebk", "puzzles/math/4"];
        let actual = get_args_opt(input.into_iter().map(ToString::to_string).collect());
        assert_eq!(
            Some(Action::Open(NotebkPath {
                folders: vec!["puzzles".to_owned(), "math".to_owned()],
                number: Some(4),
            })),
            actual
        );
    }

    #[test]
    fn parse_null_list() {
        let input = vec!["notebk", "ls"];
        let actual = get_args_opt(input.into_iter().map(ToString::to_string).collect());
        assert_eq!(
            Some(Action::List(
                NotebkPath {
                    folders: vec![],
                    number: None,
                },
                10
            )),
            actual
        );
    }
}
