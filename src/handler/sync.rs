use std::io::{BufRead, BufReader};
use std::mem;
use std::path::Path;
use std::process::{Child, ChildStdout, Command, Stdio};
use std::str::{self, FromStr};

use crate::config::Repo;
use crate::database::{Commit, Reference, SqlOid};

enum ParseLine {
    IsCommit,
    SkipMessage(usize),
    ParentMark(i64),
    CommitMark(i64),
    CommitHash(String),
    ChangeFile(String),
    PartDone,
    Continue,
}

pub struct CommitExportIter {
    command: Child,
    reader: BufReader<ChildStdout>,
    buffer: Vec<u8>,
    // debug
    line_number: usize,
    // fix field
    repo_id: i64,
    // internal state for parse
    is_commit: bool,
    skip_size: usize,
    parent_mark: i64,
    commit_mark: i64,
    commit_hash: String,
    files: Vec<String>,
}

impl CommitExportIter {
    pub fn new(repo: &Repo, mark: impl AsRef<Path>) -> anyhow::Result<Self> {
        if repo.repo_id == 0 {
            todo!()
        }
        let mark_file = mark.as_ref().join(&repo.name);
        let mark_file_str = mark_file.to_string_lossy();

        let mut command = Command::new("git")
            .args([
                "fast-export",
                "--signed-tags=strip",
                "--export-marks",
                &mark_file_str,
                "--import-marks",
                &mark_file_str,
                "--mark-tags",
                "--fake-missing-tagger",
                "--no-data",
                "--show-original-ids",
                "--reencode=yes",
                "--branches",
                "--tags",
            ])
            .current_dir(&repo.path)
            .stdout(Stdio::piped())
            .spawn()?;

        let stdout = command.stdout.take().unwrap();
        let reader = BufReader::new(stdout);
        let buffer: Vec<u8> = vec![];

        Ok(Self {
            command,
            reader,
            buffer,
            line_number: 0,
            repo_id: repo.repo_id,
            is_commit: false,
            commit_mark: 0,
            parent_mark: 0,
            skip_size: 0,
            commit_hash: "".into(),
            files: Vec::new(),
        })
    }

    fn parse_line(line: &str) -> ParseLine {
        let Some((token, value)) = line.split_once(' ') else {
            if !line.is_empty() {
                println!("{}", line);
                unreachable!();
            }
            // empty line indicates done
            return ParseLine::PartDone;
        };

        fn parse_num<T>(value: &str) -> T
        where
            T: FromStr + std::default::Default,
            <T as FromStr>::Err: std::fmt::Debug,
        {
            let mark = value.trim_start_matches(':').parse::<T>().unwrap();
            mark
        }

        fn parse_file(line: &str) -> String {
            let parts = line.splitn(3, ' ');
            let name = parts.last().unwrap();
            name.into()
        }

        match token {
            "commit" => ParseLine::IsCommit,
            "mark" => ParseLine::CommitMark(parse_num::<i64>(value)),
            "original-oid" => ParseLine::CommitHash(value.into()),
            "data" => ParseLine::SkipMessage(parse_num::<usize>(value)),
            "from" => ParseLine::ParentMark(parse_num::<i64>(value)),
            "M" => ParseLine::ChangeFile(parse_file(value)),
            _ => ParseLine::Continue,
        }
    }
}

impl Drop for CommitExportIter {
    fn drop(&mut self) {
        self.command
            .wait()
            .expect("child process encountered an error");
    }
}

impl Iterator for CommitExportIter {
    type Item = Commit;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            self.buffer.clear();

            // read stdout into buffer
            let line_size = self
                .reader
                .read_until(b'\n', &mut self.buffer)
                .expect("Failed to read from stdout");
            if line_size == 0 {
                return None; // EOF
            }

            self.line_number += 1;

            let mut index = 0;

            // skip commit message
            if self.skip_size != 0 {
                if line_size > self.skip_size {
                    index = self.skip_size;
                    // println!(
                    //     "line {}, skip {}, index {}",
                    //     line_size, self.skip_size, index
                    // );
                    self.skip_size = 0;
                } else {
                    self.skip_size -= line_size;
                    continue;
                }
            }

            // trim suffix
            if self.buffer.ends_with(b"\n") {
                self.buffer.pop();
                if self.buffer.ends_with(b"\r") {
                    self.buffer.pop();
                }
            }

            // get line from buffer
            let line = str::from_utf8(&self.buffer[index..]).expect("Failed to read utf-8");

            // parse result
            let result = Self::parse_line(&line);
            match result {
                ParseLine::IsCommit => self.is_commit = true,
                ParseLine::SkipMessage(n) => self.skip_size = n,
                ParseLine::ParentMark(u) => self.parent_mark = u,
                ParseLine::CommitMark(m) => self.commit_mark = m,
                ParseLine::CommitHash(h) => self.commit_hash = h,
                ParseLine::ChangeFile(f) => self.files.push(f),
                ParseLine::Continue => (),
                // ParseLine::Error(_) => todo!(),
                ParseLine::PartDone => {
                    // take and reset to default
                    let files = mem::take(&mut self.files);
                    let hash = mem::take(&mut self.commit_hash);
                    let c = Commit {
                        commit_id: 0,
                        commit_hash: SqlOid::from_str(&hash).expect("Failed to Parse Oid(hash)"),
                        commit_mark: mem::take(&mut self.commit_mark),
                        depth: 0,
                        repo_id: self.repo_id,
                        parent_id: 0,
                        parent_mark: mem::take(&mut self.parent_mark),
                        files: files.into_iter().map(|f| f.into()).collect(),
                    };
                    if self.is_commit {
                        self.is_commit = false;
                        return Some(c);
                    } else {
                        self.is_commit = false;
                    }
                }
            }
            // continue loop
        }
    }
}

pub struct RefsExportIter {
    repo_id: i64,
    grepo: git2::Repository,
    refs: Vec<String>,
}

impl RefsExportIter {
    pub fn new(repo: &Repo, mut refs: Vec<String>) -> anyhow::Result<Self> {
        let grepo = git2::Repository::open_bare(&repo.path)?;
        if refs.is_empty() {
            refs = grepo
                .references()?
                .names()
                .flatten()
                .map(|r| r.to_string())
                .collect();
        }
        let iter = Self {
            repo_id: repo.repo_id,
            grepo,
            refs,
        };
        Ok(iter)
    }
}

impl Iterator for RefsExportIter {
    type Item = Reference;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(name) = self.refs.pop() {
            let Ok(r) = self.grepo.find_reference(&name) else {
                continue;
            };
            let is_tag = if r.is_tag() {
                true
            } else if r.is_branch() {
                false
            } else {
                continue;
            };
            let Some(full_name) = r.name() else {
                continue;
            };
            let Some(short_name) = r.shorthand() else {
                continue;
            };
            if short_name == full_name {
                continue;
            }
            if let Ok(commit) = r.peel_to_commit() {
                let dr = Self::Item {
                    repo_id: self.repo_id,
                    full_name: full_name.to_string(),
                    short_name: short_name.replace("/", ":"),
                    is_tag,
                    commit_id: 0,
                    commit_hash: commit.id().into(),
                    time: commit.committer().when().seconds(),
                };
                return Some(dr);
            }
        }
        None
    }
}
