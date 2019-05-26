#[allow(unused_imports)]
#[macro_use]
extern crate failure;
use failure::Error;

extern crate systemstat;
use systemstat::{Platform, System};

extern crate config;
extern crate serde;
#[allow(unused_imports)]
#[macro_use]
extern crate serde_derive;

extern crate itertools;

use itertools::{Either, Itertools};

extern crate hostname;
extern crate regex;

mod settings;
pub use settings::{AppConfig, Settings};

use std::collections::VecDeque;
use std::ffi::OsStr;
use std::fs;
use std::path::{Component, Path, PathBuf};

fn get_current_drive() -> Option<String> {
    match std::env::current_dir() {
        Ok(path) => {
            let mut components = path.components();
            let mut path = PathBuf::new();

            while !path.ends_with(std::path::MAIN_SEPARATOR.to_string()) {
                match components.next() {
                    Some(component) => match component {
                        Component::RootDir => path.push(std::path::MAIN_SEPARATOR.to_string()),
                        Component::Prefix(prefix) => path.push(prefix.as_os_str()),
                        _ => {}
                    },
                    None => return None,
                }
            }

            return Some(path.to_string_lossy().into_owned());
        }
        _ => {}
    }
    return None;
}

fn get_drive_by_label(label: &str) -> Option<String> {
    let sys = System::new();
    match sys.mounts() {
        Ok(mounts) => {
            for mount in mounts {
                if mount.fs_mounted_from == label {
                    return Some(mount.fs_mounted_on);
                }
            }
            return None;
        }
        Err(_) => return None,
    }
}

fn get_drive(label: &str) -> Result<String, Error> {
    if label == "$CURRENTDRIVE" {
        match get_current_drive() {
            Some(path) => return Ok(path),
            None => return Err(format_err!("unable to detect current drive")),
        }
    } else {
        match get_drive_by_label(&label) {
            Some(path) => return Ok(path),
            None => return Err(format_err!("unable to find drive with label: {}", label)),
        }
    }
}

fn build_initial_dest<'a>(drive: &'a str, format: &'a str) -> Result<PathBuf, Error> {
    let mut dest = PathBuf::new();
    dest.push(drive);

    let components: Vec<Component> = Path::new(format).components().collect();
    for item in components {
        match item {
            Component::RootDir => {}
            Component::Normal(path) => {
                if path == "$HOSTNAME" {
                    dest.push(&hostname::get_hostname().unwrap_or_default());
                } else {
                    dest.push(path);
                }
            }
            _ => return Err(format_err!("dest.format is invalid: {}", format)),
        }
    }

    Ok(dest)
}

fn count(s: &str, c: char) -> usize {
    let mut s = s.to_owned();
    s.retain(|x| x == c);
    s.len()
}

#[derive(Debug)]
struct GlobMatch {
    path: PathBuf,
    matches: Vec<String>,
}

fn glob(
    current_path: &mut PathBuf,
    parts_remaining: &mut VecDeque<Component>,
    filters: &mut VecDeque<settings::Match>,
    matches: &mut VecDeque<String>,
) -> Result<Vec<Result<GlobMatch, Error>>, Error> {
    if parts_remaining.len() == 0 {
        return Ok(vec![Ok(GlobMatch {
            path: current_path.to_owned(),
            matches: matches.to_owned().into(),
        })]);
    }

    match parts_remaining.pop_front().unwrap() {
        Component::Prefix(prefix) => current_path.push(prefix.as_os_str()),
        Component::RootDir => current_path.push(std::path::MAIN_SEPARATOR.to_string()),
        Component::Normal(path) => {
            let path: String = path.to_string_lossy().into_owned();
            if path.contains("*") {
                let filter_count = count(&path, '*');
                let filters = filters.clone();

                let re = regex::Regex::new(&format!(
                    r"(?i)^{}$",
                    regex::escape(&path).replace("\\*", "(.*)")
                ))
                .unwrap();

                match fs::read_dir(&current_path) {
                    Ok(v) => {
                        let (successes, failures): (Vec<_>, Vec<Result<GlobMatch, Error>>) = v
                            .partition_map(|x| match x {
                                Ok(v) => Either::Left(v),
                                Err(v) => Either::Right(Err(v.into())),
                            });

                        let (successes, more_failures): (
                            Vec<Vec<Result<GlobMatch, Error>>>,
                            Vec<Result<GlobMatch, Error>>,
                        ) = successes
                            .into_iter()
                            .filter(|x| {
                                let x: &Path = &x.path();

                                if x.is_file() && parts_remaining.len() > 0 {
                                    return false;
                                }

                                let file_name: String = x
                                    .file_name()
                                    .unwrap()
                                    .to_string_lossy()
                                    .into_owned()
                                    .to_lowercase();

                                match re.captures(&file_name) {
                                    Some(cap) => {
                                        let mut filters = filters.clone();

                                        for (i, mat) in cap.iter().enumerate() {
                                            if i > 0 {
                                                if let Some(mat) = mat {
                                                    let mat = mat.as_str().to_owned();
                                                    let filter =
                                                        filters.pop_front().unwrap_or_default();

                                                    if filter.exclude.contains(&mat) {
                                                        return false;
                                                    }

                                                    if filter.only.len() > 0
                                                        && !filter.only.contains(&mat)
                                                    {
                                                        return false;
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    None => return false,
                                }

                                true
                            })
                            .map(|x| {
                                let mut new_matches = matches.clone();
                                new_matches.push_back(
                                    x.path().file_name().unwrap().to_string_lossy().into_owned(),
                                );

                                let mut filters = filters.clone();
                                for _ in 0..filter_count {
                                    filters.pop_front();
                                }

                                glob(
                                    &mut x.path(),
                                    &mut parts_remaining.clone(),
                                    &mut filters,
                                    &mut new_matches,
                                )
                            })
                            .partition_map(|x| match x {
                                Ok(v) => Either::Left(v),
                                Err(v) => Either::Right(Err(v)),
                            });
                        let successes: Vec<Result<GlobMatch, Error>> =
                            successes.into_iter().flatten().collect();

                        return Ok(vec![successes, failures, more_failures]
                            .into_iter()
                            .flatten()
                            .collect());
                    }
                    Err(v) => return Err(format_err!("{}: {}", current_path.to_string_lossy(), v)),
                }
            } else if path.starts_with('{') && path.ends_with('}') {
                let filter: Vec<String> = path[1..path.len() - 1]
                    .split(',')
                    .map(|x| x.trim().to_lowercase().to_owned())
                    .collect();

                let mut exclude: Vec<String> = vec![];
                let mut only: Vec<String> = vec![];

                for item in filter {
                    if item.starts_with('-') {
                        exclude.push(item[1..].to_owned());
                    } else {
                        only.push(item);
                    }
                }

                filters.push_front(settings::Match { exclude, only });
                parts_remaining.push_front(Component::Normal(OsStr::new("*")));
            } else {
                current_path.push(path);
            }
        }
        _ => return Err(format_err!("source must be absolute")),
    }

    if !current_path.exists() {
        return Ok(vec![]);
    }

    glob(current_path, parts_remaining, filters, matches)
}

fn path_from_matches(format: VecDeque<Component>, matches: Vec<String>) -> Result<PathBuf, Error> {
    let mut format = format.clone();
    let mut ret = PathBuf::new();

    while !format.is_empty() {
        match format.pop_front().unwrap() {
            Component::RootDir => {}
            Component::Normal(path) => {
                let path: String = path.to_string_lossy().into_owned();
                if path.starts_with('$') {
                    let i = path[1..].parse::<usize>()?;

                    match i {
                        _ => ret.push(&matches[i - 1]),
                    }
                } else {
                    ret.push(path);
                }
            }
            _ => return Err(format_err!("path is invalid")),
        }
    }

    Ok(ret)
}

fn rcopy(src: PathBuf, dest: PathBuf, config: &AppConfig, ret: &mut [u32; 4]) -> Result<(), Error> {
    let mut internal_copy = |src: PathBuf, dest: PathBuf| {
        if !config.dryrun {
            match fs::copy(&src, &dest) {
                Ok(_) => {
                    ret[0] += 1;
                    ret[2] += 1;

                    if !config.quiet {
                        println!("{}: Copied.", src.to_string_lossy());
                    }
                }
                Err(e) => {
                    ret[1] += 1;
                    eprintln!("{}: {}", src.to_string_lossy(), e);
                }
            }
        } else {
            ret[0] += 1;
            ret[2] += 1;

            if !config.quiet {
                println!("{}: Would be copied.", src.to_string_lossy());
            }
        }
    };

    let src_md = src.metadata()?;
    if src_md.is_file() {
        if dest.exists() {
            let dest_md = dest.metadata()?;

            if dest_md.modified()? >= src_md.modified()? {
                ret[0] += 1;
                ret[3] += 1;
                if !config.quiet {
                    if !config.dryrun {
                        println!("{}: Skipped.", src.to_string_lossy());
                    } else {
                        println!("{}: Would be skipped.", src.to_string_lossy());
                    }
                }
            } else {
                internal_copy(src, dest);
            }
        } else {
            if !config.dryrun {
                fs::create_dir_all(dest.parent().unwrap())?;
            }
            internal_copy(src, dest);
        }
    } else {
        for entry in fs::read_dir(&src)? {
            match entry {
                Ok(entry) => {
                    let entry = entry.path();
                    let mut dest = dest.clone();
                    dest.push(entry.file_name().unwrap());

                    if let Err(e) = rcopy(entry.clone(), dest, &config, ret) {
                        ret[1] += 1;
                        eprintln!("{}: {}", entry.to_string_lossy(), e);
                    }
                }
                Err(e) => {
                    ret[1] += 1;
                    eprintln!("{}", e);
                }
            }
        }
    }

    Ok(())
}

pub fn backup(settings: Settings) -> Result<[u32; 4], Error> {
    let dest: PathBuf =
        build_initial_dest(&get_drive(&settings.dest.label)?, &settings.dest.format)?;

    let mut ret: [u32; 4] = [0, 0, 0, 0];

    for entry in settings.files {
        let to: VecDeque<Component> = Path::new(&entry.to).components().collect();
        let mut source: VecDeque<Component> = Path::new(&entry.from).components().collect();

        for file in glob(
            &mut PathBuf::new(),
            &mut source,
            &mut entry.filters.into(),
            &mut VecDeque::new(),
        )? {
            match file {
                Ok(file) => {
                    let mut dest: PathBuf = dest.clone();
                    match path_from_matches(to.clone(), file.matches) {
                        Ok(path) => dest.push(path),
                        _ => return Err(format_err!("to field is invalid: {}", entry.to)),
                    }

                    if let Err(e) = rcopy(file.path.clone(), dest, &settings.config, &mut ret) {
                        ret[1] += 1;
                        eprintln!("{}: {}", file.path.to_string_lossy(), e);
                    }
                }
                Err(e) => {
                    ret[1] += 1;
                    eprintln!("{}", e);
                }
            }
        }
    }

    Ok(ret)
}
