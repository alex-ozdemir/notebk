extern crate chrono;

use std::env;
use std::fs;
use std::io::BufRead;
use std::io;

const DIRECTORY: &str = "/Users/aozdemir/personal/notebook";

fn entries() -> Result<Vec<fs::DirEntry>, std::io::Error> {
    let mut listing: Vec<_> = Vec::new();
    for entry_result in fs::read_dir(DIRECTORY)? {
        listing.push(entry_result?);
    }
    listing.as_mut_slice().sort_unstable_by_key(
        |e| e.file_name(),
    );
    listing.as_mut_slice().reverse();
    Ok(listing)
}

fn entry(n: usize) -> io::Result<fs::DirEntry> {
    entries()?.into_iter().nth(n - 1).ok_or(io::Error::new(
        io::ErrorKind::NotFound,
        format!("There is no entry number {}", n),
    ))
}

fn list(n: usize) -> Result<(), std::io::Error> {
    for (i, entry) in entries()?.iter().take(n).enumerate() {
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
    }
    Ok(())
}

fn open_entry(n: usize) -> Result<(), std::io::Error> {
    let path = entry(n)?.path();
    std::process::Command::new("vim").arg(path).status()?;
    Ok(())
}

fn open_todays_entry() -> io::Result<()> {
    let mut path_buf = std::path::PathBuf::from(DIRECTORY);
    path_buf.push(format!("{}", chrono::Local::now().format("%Y-%m-%d.md")));
    std::process::Command::new("vim").arg(path_buf).status()?;
    Ok(())
}

fn delete_entry(n: usize) -> io::Result<()> {
    fs::remove_file(entry(n)?.path())
}

fn number_arg(n: usize) -> io::Result<usize> {
    env::args()
        .nth(n)
        .and_then(|s| s.parse::<usize>().ok())
        .ok_or(io::Error::new(
            io::ErrorKind::Other,
            "expected a number as the second argument",
        ))
}

fn application() -> io::Result<()> {
    match env::args().nth(1).as_ref().map(String::as_str) {
        None => open_todays_entry(),
        Some("delete") => delete_entry(number_arg(2)?),
        Some("ls") => list(number_arg(2).unwrap_or(10)),
        Some(_) => open_entry(number_arg(1)?),
    }
}

fn main() {
    match application() {
        Err(e) => {
            println!("Error: {}", e);
            std::process::exit(1);
        }
        Ok(()) => std::process::exit(0),
    }
}
