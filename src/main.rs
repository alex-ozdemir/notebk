extern crate ansi_term;
extern crate dirs;
extern crate time;

use std::fs;
use std::io;
use std::io::BufRead;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use time::{macros::format_description, Date, OffsetDateTime};

use ansi_term::Colour::{Blue, Green};

mod parser;

use parser::{
    action::{Action, NotebkPath},
    args::get_args_or_exit,
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
                    Green.paint(first_line.as_ref().map(String::as_str).unwrap_or("<empty>"))
                );
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

fn sync<P: AsRef<Path>>(base_path: &P) -> io::Result<()> {
    let base_path: &Path = base_path.as_ref();
    Command::new("git")
        .current_dir(base_path)
        .arg("pull")
        .status()?;
    Command::new("git")
        .current_dir(base_path)
        .args(["commit", "-a", "-m", "sync"])
        .status()?;
    Command::new("git")
        .current_dir(base_path)
        .arg("push")
        .status()?;
    Ok(())
}

fn execute(action: Action) -> io::Result<()> {
    let base = get_directory()?;
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
        Action::Open(notebk_path) => {
            let file_path = to_file_path(&notebk_path, &base)?;
            make_writable(&file_path)?;
            let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vim".to_owned());
            Command::new(&editor).arg(&file_path).status()?;
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
