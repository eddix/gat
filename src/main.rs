mod config;

use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use colored::Colorize;
use git2::{AutotagOption, Cred, ErrorCode, FetchOptions, RemoteCallbacks, RemoteUpdateFlags, Repository, StatusOptions};
use std::io::Write;
use std::{env, io};

#[derive(Parser)]
struct Gat {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    List,
    Status,
    Fetch,
    Pull,
}

fn print_title(repository: &config::Repository) -> Result<()> {
    let repo = Repository::open(&repository.location)?;
    if repo.is_bare() {
        eprintln!(
            "{}: cannot use bare repository\n{} - {}\n",
            repository.name().red().bold(),
            repository
                .description
                .as_ref()
                .unwrap_or(&"No description".to_string())
                .yellow(),
            &repository.location.blue(),
        );
        return Err(anyhow!("Not a bare repository"));
    }
    let head = match repo.head() {
        Ok(head) => Some(head),
        Err(ref e) if e.code() == ErrorCode::UnbornBranch || e.code() == ErrorCode::NotFound => {
            None
        }
        Err(e) => {
            eprintln!("can't get HEAD: {}", e);
            return Err(e.into())
        }
    };

    println!(
        "{}({}): {}\n{}",
        repository.name().green().bold(),
        head.as_ref().and_then(|h| h.shorthand()).unwrap_or("no branch").cyan(),
        &repository.location.blue(),
        repository
            .description
            .as_ref()
            .unwrap_or(&"No description".to_string())
            .white().italic(),
    );
    Ok(())
}

fn status(repository: config::Repository) -> Result<()> {
    print_title(&repository)?;
    let repo = Repository::open(repository.location)?;
    if repo.is_bare() {
        return Err(anyhow::anyhow!("cannot use bare repository").into());
    }

    let mut opts = StatusOptions::new();
    opts.include_ignored(false);
    opts.include_untracked(true);
    let status = repo.statuses(Some(&mut opts))?;
    if status.iter().len() == 0 {
        println!("{}", "Nothing changed in this repository".green());
        return Ok(())
    }
    for status in repo.statuses(Some(&mut opts))?.iter() {
        let mut istatus = match status.status() {
            s if s.contains(git2::Status::INDEX_NEW) => 'A',
            s if s.contains(git2::Status::WT_MODIFIED) => 'M',
            s if s.contains(git2::Status::WT_DELETED) => 'D',
            s if s.contains(git2::Status::WT_RENAMED) => 'R',
            s if s.contains(git2::Status::WT_TYPECHANGE) => 'T',
            _ => ' '
        };
        let mut wstatus = match status.status() {
            s if s.contains(git2::Status::WT_NEW) => {
                if istatus == ' ' {
                    istatus = '?';
                }
                '?'
            },
            s if s.contains(git2::Status::WT_MODIFIED) => 'M',
            s if s.contains(git2::Status::WT_DELETED) => 'D',
            s if s.contains(git2::Status::WT_RENAMED) => 'R',
            s if s.contains(git2::Status::WT_TYPECHANGE) => 'T',
            _ => ' ',
        };
        if status.status().contains(git2::Status::IGNORED) {
            istatus = '!';
            wstatus = '!';
        }
        println!(
            "  - {}{}  {}",
            istatus,
            wstatus,
            status.path().unwrap_or("None"),
        );
    }
    Ok(())
}

fn fetch(repository: config::Repository) -> Result<()> {
    print_title(&repository)?;
    let repo = Repository::open(repository.location)?;
    if repo.is_bare() {
        return Err(anyhow!("cannot use bare repository").into());
    }
    let mut cb = RemoteCallbacks::new();
    cb.credentials(|_url, username_from_url, _allowed_types| {
        Cred::ssh_key(
            username_from_url.unwrap(),
            None,
            std::path::Path::new(&format!("{}/.ssh/id_rsa", env!("HOME"))),
            None,
        )
    });
    cb.sideband_progress(|data| {
        print!("{}", String::from_utf8_lossy(data));
        io::stdout().flush().unwrap();
        true
    });
    cb.update_tips(|refname, a, b| {
        if a.is_zero() {
            println!("[new]     {:20} {}", b, refname);
        } else {
            println!("[updated] {:10}..{:10} {}", a, b, refname);
        }
        true
    });
    cb.transfer_progress(|stats| {
        if stats.received_objects() == stats.total_objects() {
            print!(
                "Resolving deltas {}/{}\r",
                stats.indexed_deltas(),
                stats.total_deltas()
            );
        } else if stats.total_objects() > 0 {
            print!(
                "Received {}/{} objects ({}) in {} bytes\r",
                stats.received_objects(),
                stats.total_objects(),
                stats.indexed_objects(),
                stats.received_bytes()
            )
        }
        io::stdout().flush().unwrap();
        true
    });

    let mut fo = FetchOptions::new();
    fo.remote_callbacks(cb);

    let mut remote = repo.find_remote("origin")?;
    remote.download(&[] as &[&str], Some(&mut fo))?;

    {
        let stats = remote.stats();
        if stats.local_objects() > 0 {
            println!(
                "\rReceived {}/{} objects in {} bytes (used {} local \
             objects)",
                stats.indexed_objects(),
                stats.total_objects(),
                stats.received_bytes(),
                stats.local_objects()
            );
        } else {
            println!(
                "\rReceived {}/{} objects in {} bytes",
                stats.indexed_objects(),
                stats.total_objects(),
                stats.received_bytes()
            )
        }
    }

    remote.disconnect()?;
    remote.update_tips(
        None,
        RemoteUpdateFlags::UPDATE_FETCHHEAD,
        AutotagOption::Unspecified,
        None,
    )?;

    Ok(())
}

fn pull(repository: config::Repository) -> Result<()> {
    print_title(&repository)?;

    Ok(())
}

fn main() {
    let config = config::from_file(format!("{}/.gatconfig", env!("HOME")).as_str()).unwrap();
    match Gat::parse().command {
        Commands::List => {
            for repo in config.repository {
                let _ = print_title(&repo);
            }
        }
        Commands::Status => {
            for repo in config.repository {
                if let Err(err) = status(repo) {
                    eprintln!("{:?}", err);
                }
            }
        }
        Commands::Fetch => {
            for repo in config.repository {
                if let Err(err) = fetch(repo) {
                    eprintln!("{:?}", err)
                }
            }
        }
        Commands::Pull => {
            for repo in config.repository {
                if let Err(err) = pull(repo) {
                    eprintln!("{:?}", err)
                }
            }
        }
    }
}
