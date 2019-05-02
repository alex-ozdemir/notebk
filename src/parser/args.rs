use docopt::Docopt;

use serde::Deserialize;

pub const USAGE: &'static str = "
notebk

Usage:
  notebk ls [<count>]
  notebk <path> ls [<count>]
  notebk <path> which
  notebk <path> delete
  notebk <path>
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
";

#[derive(Debug, Deserialize)]
pub struct Args {
    pub cmd_delete: bool,
    pub cmd_ls: bool,
    pub cmd_which: bool,
    pub cmd_mv: bool,
    pub arg_path: Option<String>,
    pub arg_src: Option<String>,
    pub arg_dst: Option<String>,
    pub arg_count: Option<usize>,
}

pub fn get_args() -> Args {
    Docopt::new(USAGE)
        .and_then(|d| d.deserialize())
        .unwrap_or_else(|e| e.exit())
}
