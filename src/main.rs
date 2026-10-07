extern crate ansi_term;
extern crate dirs;
extern crate time;

use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use time::{macros::format_description, Date, OffsetDateTime};

use ansi_term::Colour::{Blue, Green};

mod parser;

use parser::{
    action::{Action, NotebkPath},
    get_args_or_exit,
};

fn today_string() -> String {
    let fd = format_description!("[year]-[month]-[day].md");
    let date = OffsetDateTime::now_local().expect("local time").date();
    date.format(fd).expect("format date")
}

fn to_file_path(path: &NotebkPath, directory: &str) -> io::Result<PathBuf> {
    let mut path_buf = path.inner_to_dir_path(directory)?;
    match path.number {
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
        None => path_buf.push(today_string()),
    }
    Ok(path_buf)
}

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

/// An entry's title: its first non-blank line, or `<empty>`.
fn title_of(contents: &str) -> &str {
    contents
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or("<empty>")
}

fn list(dir_path: &Path, n: usize) -> io::Result<()> {
    if dir_path.exists() {
        for (i, entry) in entries(dir_path)?.into_iter().take(n).enumerate() {
            if entry.file_type()?.is_file() {
                let contents = read_file(entry.path())?;
                println!("{:2}  {}", i + 1, Green.paint(title_of(&contents)));
            } else {
                println!(
                    "{:2}  {}/",
                    i + 1,
                    Blue.paint(entry.file_name().to_string_lossy())
                );
            }
        }
        Ok(())
    } else {
        println!(
            "The path `{}` doesn't exist",
            dir_path.to_str().unwrap_or("INVALID UNICODE")
        );
        Ok(())
    }
}

fn most_recent(node: &fs::DirEntry) -> Option<Date> {
    let ft = node.file_type().unwrap();
    if ft.is_file() {
        let fd = format_description!("[year]-[month]-[day].md");
        Some(
            Date::parse(
                node.file_name().to_str().unwrap_or_else(|| {
                    eprintln!("Could not parse entry {} as a date", node.path().display());
                    std::process::exit(1)
                }),
                fd,
            )
            .unwrap_or_else(|e| {
                eprintln!(
                    "Could not parse entry {} as a date because {}",
                    node.path().display(),
                    e
                );
                std::process::exit(1)
            }),
        )
    } else if ft.is_dir() && node.file_name().to_string_lossy() != ".git" {
        fs::read_dir(&node.path())
            .unwrap_or_else(|e| {
                eprintln!(
                    "Could not read directory {} because {}",
                    node.path().display(),
                    e
                );
                std::process::exit(1)
            })
            .into_iter()
            .map(|e| {
                e.unwrap_or_else(|e| {
                    eprintln!(
                        "Could not list directory {} because {}",
                        node.path().display(),
                        e
                    );
                    std::process::exit(1)
                })
            })
            .filter_map(|e| most_recent(&e))
            .max()
    } else if ft.is_symlink() {
        panic!("Unexpected sym link at {}", node.path().display())
    } else {
        None
    }
}

fn entries(dir_path: &Path) -> io::Result<Vec<fs::DirEntry>> {
    let mut listing = Vec::new();
    for entry_result in fs::read_dir(&dir_path)? {
        listing.push(entry_result?);
    }
    listing
        .as_mut_slice()
        .sort_unstable_by_key(|e| most_recent(&e));
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

fn open_in_editor(file_path: &Path, line: Option<usize>) -> io::Result<()> {
    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_owned());
    let mut cmd = Command::new(&editor);
    if let Some(n) = line {
        // `+N` is the usual convention, but a GUI editor like `code` would
        // take it as a filename, so only pass it to editors known to accept it.
        if matches!(
            Path::new(&editor).file_name().and_then(|s| s.to_str()),
            Some("vim" | "nvim" | "vi" | "view" | "emacs" | "nano" | "micro" | "joe" | "kak")
        ) {
            cmd.arg(format!("+{}", n));
        }
    }
    cmd.arg(file_path);
    cmd.status()?;
    Ok(())
}

fn sanitize(s: &str) -> String {
    s.replace(['\t', '\n'], " ")
}

/// Display widths of the fixed-width columns in the fzf list.
const PATH_WIDTH: usize = 15;
const TITLE_WIDTH: usize = 35;

/// 0-based positions of the hidden trailing fields of each row, which fzf
/// does not display but the preview command and the selection parsing use.
const LINE_FIELD: usize = 3;
const ABS_PATH_FIELD: usize = 4;

/// Truncates `s` to `width` characters, padding it out to exactly that many,
/// so the fzf list keeps fixed-width columns.
fn fit(s: &str, width: usize) -> String {
    format!("{:<width$.width$}", s, width = width)
}

/// (line number, note date, the tab-separated fzf row)
type FzfCandidate = (usize, Date, String);

/// Writes one fzf candidate line per non-blank line of `entry`'s contents.
/// 
/// Each line carries, tab-separated:
/// 
/// * the *notebk path* (folders/number, as `to_file_path` uses)
/// * the *title* (first non-blank line of the entry)
/// * the line's text (sanitized)
/// * (hidden) the line number (1-based, as `bat` expects for its `--highlight-line` option)
/// * (hidden) the absolute path to the entry file, for the preview command and for
///   opening the selected entry in $EDITOR.
/// 
/// The first three are displayed in the fzf list, with the first two padded to
/// fixed widths so they align across rows. The last two fields are hidden, but
/// used by the preview command and for opening the selected entry in $EDITOR.
/// 
/// The list is ordered by line number (1 first), then by date (most recent
/// first).
fn emit_candidates(
    entry: &fs::DirEntry,
    folders: &[String],
    number: usize,
    out: &mut Vec<FzfCandidate>,
) {
    let notebk_path = if folders.is_empty() {
        number.to_string()
    } else {
        format!("{}/{}", folders.join("/"), number)
    };
    let notebk_path = fit(&sanitize(&notebk_path), PATH_WIDTH);

    let date = most_recent(entry).expect("dated file");

    let abs_path = entry.path();
    let abs_path_str = sanitize(&abs_path.to_string_lossy());

    let contents = read_file(&abs_path).unwrap_or_default();
    let mut lines: Vec<(usize, &str)> = contents
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty())
        .collect();
    // An entry with nothing in it still gets a row, so it stays findable.
    if lines.is_empty() {
        lines.push((1, title_of(&contents)));
    }
    let title = fit(&sanitize(lines[0].1), TITLE_WIDTH);

    for (n, text) in lines {
        out.push((
            n,
            date,
            format!(
                "{} \t{} \t{}\t{}\t{}",
                notebk_path,
                title,
                sanitize(text),
                n,
                abs_path_str
            ),
        ));
    }
}

/// Recursively walks `dir`, numbering entries the same way `entries()`
/// already does for `ls` and path resolution, and emits fzf candidates for
/// every file found (skipping `.git`).
fn collect_candidates(
    dir: &Path,
    folders: &[String],
    out: &mut Vec<FzfCandidate>,
) -> io::Result<()> {
    for (i, entry) in entries(dir)?.into_iter().enumerate() {
        let number = i + 1;
        let file_type = entry.file_type()?;
        if file_type.is_file() {
            emit_candidates(&entry, folders, number, out);
        } else if file_type.is_dir() && entry.file_name() != ".git" {
            let mut sub_folders = folders.to_vec();
            sub_folders.push(entry.file_name().to_string_lossy().into_owned());
            collect_candidates(&entry.path(), &sub_folders, out)?;
        }
    }
    Ok(())
}

/// Fuzzy-find a note by notebk path, title, or contents (via the `fzf`
/// binary) and open the selected one in $EDITOR.
fn find(base: &str) -> io::Result<()> {
    let base_path = Path::new(base);
    let mut rows = Vec::new();
    if base_path.exists() {
        collect_candidates(base_path, &[], &mut rows)?;
    }
    // Titles (line 1) first, then deeper lines; most recent notes first within
    // each line number.
    rows.sort_by_key(|(line, date, _)| (*line, std::cmp::Reverse(*date)));

    let header = format!(
        "--header={} \t{} \tline",
        fit("path", PATH_WIDTH),
        fit("title", TITLE_WIDTH)
    );
    // fzf's field placeholders are 1-based.
    let preview = format!(
        "--preview=bat --style=numbers --color=always --paging=never --highlight-line {{{line}}} -- {{{path}}} 2>/dev/null || cat -- {{{path}}}",
        line = LINE_FIELD + 1,
        path = ABS_PATH_FIELD + 1
    );
    let mut child = Command::new("fzf")
        .args([
            "--delimiter=\t",
            "--with-nth=1,2,3",
            // The columns are padded to fixed widths, so a tab must occupy a
            // single column or it would push them back out of alignment.
            "--tabstop=1",
            "--tiebreak=index",
            "--prompt=notebk> ",
            "--height=100%",
            "--layout=reverse",
            &header,
            &preview,
            "--preview-window=right:30%:nowrap",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .map_err(|e| {
            if e.kind() == io::ErrorKind::NotFound {
                io::Error::new(io::ErrorKind::NotFound, "fzf not found; please install it")
            } else {
                e
            }
        })?;

    {
        // The handle has to drop at the end of this scope, or fzf never sees
        // EOF on its stdin and waits forever.
        let mut stdin = io::BufWriter::new(child.stdin.take().expect("fzf stdin was piped"));
        for (_, _, row) in &rows {
            writeln!(stdin, "{}", row)?;
        }
    }

    let output = child.wait_with_output()?;
    let selection = String::from_utf8_lossy(&output.stdout);
    let selection = selection.trim();
    if selection.is_empty() {
        return Ok(());
    }

    let mut fields = selection.split('\t').skip(LINE_FIELD);
    let line = fields.next().and_then(|s| s.parse::<usize>().ok());
    let abs_path = fields.next().unwrap_or("");
    open_in_editor(Path::new(abs_path), line)
}

fn git(base_path: &Path, args: &[&str]) -> io::Result<()> {
    Command::new("git")
        .current_dir(base_path)
        .args(args)
        .status()?;
    Ok(())
}

fn sync<P: AsRef<Path>>(base_path: &P) -> io::Result<()> {
    let base_path: &Path = base_path.as_ref();
    git(base_path, &["pull"])?;
    git(base_path, &["add", "."])?;
    git(base_path, &["commit", "-a", "-m", "sync"])?;
    git(base_path, &["push"])
}

/// Runs git in `base_path` with its output captured rather than shown.
fn git_quiet(base_path: &Path, args: &[&str]) -> io::Result<Output> {
    Command::new("git").current_dir(base_path).args(args).output()
}

/// Like `git_quiet`, but a non-zero exit is an error carrying git's output.
fn git_checked(base_path: &Path, args: &[&str]) -> io::Result<Output> {
    let output = git_quiet(base_path, args)?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "`git {}` failed:\n{}{}",
                args.join(" "),
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr).trim_end()
            ),
        ))
    }
}

fn is_git_repo(base_path: &Path) -> bool {
    git_quiet(base_path, &["rev-parse", "--git-dir"]).map_or(false, |o| o.status.success())
}

/// Commits local changes, pulls, and pushes.
///
/// If the pull conflicts, the repository is rolled back to how it was before
/// the attempt and an error is returned. If the pull or push fails for any
/// other reason (e.g., no network), only a warning is printed.
fn try_sync(base_path: &Path) -> io::Result<()> {
    // Unset only before the first commit, when a pull can't conflict.
    let orig_head = git_checked(base_path, &["rev-parse", "--verify", "-q", "HEAD"])
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned());

    git_checked(base_path, &["add", "-A"])?;
    let nothing_staged = git_quiet(base_path, &["diff", "--cached", "--quiet"])?
        .status
        .success();
    if !nothing_staged {
        git_checked(base_path, &["commit", "-q", "-m", "sync"])?;
    }

    let result = git_checked(base_path, &["pull", "-q", "--no-rebase", "--no-edit"])
        .and_then(|_| git_checked(base_path, &["push", "-q"]).map(|_| ()));
    if let Err(e) = result {
        let conflicted = !git_checked(base_path, &["ls-files", "-u"])?.stdout.is_empty();
        if !conflicted {
            eprintln!("Warning: could not sync with remote: {}", e);
            return Ok(());
        }
        git_checked(base_path, &["merge", "--abort"])?;
        if let Some(h) = &orig_head {
            git_checked(base_path, &["reset", "-q", h])?;
        }
        return Err(io::Error::new(
            io::ErrorKind::Other,
            format!(
                "{}\nRolled back the sync attempt; your changes are uncommitted in {}",
                e,
                base_path.display()
            ),
        ));
    }
    Ok(())
}

fn execute(action: Action) -> io::Result<()> {
    let base = get_directory()?;
    let modifies = matches!(
        action,
        Action::Delete(_) | Action::Move(..) | Action::Open(_) | Action::Find
    );
    perform(action, &base)?;
    let base_path = Path::new(&base);
    if modifies && is_git_repo(base_path) {
        try_sync(base_path)?;
    }
    Ok(())
}

fn perform(action: Action, base: &str) -> io::Result<()> {
    match action {
        Action::Delete(notebk_path) => {
            let file_path = to_file_path(&notebk_path, &base)?;
            verify_is_file(&file_path)?;
            fs::remove_file(&file_path)?;
            cleanup(&file_path)?;
            Ok(())
        }
        Action::Which(notebk_path) => {
            let file_path = to_file_path(&notebk_path, &base)?;
            println!("{}", file_path.to_string_lossy());
            Ok(())
        }
        Action::List(notebk_path, n) => {
            let dir_path = notebk_path.to_dir_path(&base)?;
            list(&dir_path, n)
        }
        Action::Sync => sync(&base),
        Action::Find => find(&base),
        Action::Open(notebk_path) => {
            let file_path = to_file_path(&notebk_path, &base)?;
            make_writable(&file_path)?;
            open_in_editor(&file_path, None)?;
            cleanup(&file_path)
        }
        Action::Move(src_notebk_path, dst_notebk_path) => {
            let src_path = to_file_path(&src_notebk_path, &base)?;
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

fn main() {
    let action = get_args_or_exit();
    std::process::exit(
        execute(action)
            .map_err(|e| println!("Error: {}", e))
            .map(|_| 0)
            .unwrap_or(1),
    )
}
