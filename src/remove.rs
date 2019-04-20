// Copyright 2017-2019 Matthias Krüger. See the COPYRIGHT
// file at the top-level directory of this distribution.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::fs;
use std::path::PathBuf;

use crate::library::*;

use humansize::{file_size_opts, FileSize};

pub(crate) fn rm_old_crates(
    amount_to_keep: u64,
    dry_run: bool,
    registry_src_path: &PathBuf,
    size_changed: &mut bool,
) -> Result<(), (ErrorKind, PathBuf)> {
    println!();

    // remove crate sources from cache
    // src can be completely removed since we can always rebuilt it from cache (by extracting packages)
    let mut removed_size = 0;
    // walk registry repos
    for repo in fs::read_dir(&registry_src_path).unwrap() {
        let mut crate_list = fs::read_dir(&repo.unwrap().path())
            .unwrap()
            .map(|cratepath| cratepath.unwrap().path())
            .collect::<Vec<PathBuf>>();
        crate_list.sort();
        crate_list.reverse();

        let mut versions_of_this_package = 0;
        let mut last_pkgname = String::new();
        // iterate over all crates and extract name and version
        for pkgpath in &crate_list {
            let path_end = match pkgpath.iter().last() {
                Some(path_end) => path_end,
                None => return Err((ErrorKind::MalformedPackageName, (pkgpath.to_owned()))),
            };

            let mut vec = path_end.to_str().unwrap().split('-').collect::<Vec<&str>>();
            let pkgver = match vec.pop() {
                Some(pkgver) => pkgver,
                None => return Err((ErrorKind::MalformedPackageName, (pkgpath.to_owned()))),
            };
            let pkgname = vec.join("-");

            if amount_to_keep == 0 {
                removed_size += fs::metadata(pkgpath)
                    .unwrap_or_else(|_| {
                        panic!("Failed to get metadata of file '{}'", &pkgpath.display())
                    })
                    .len();

                let dryrun_msg = format!(
                    "dry run: not actually deleting {} {} at {}",
                    pkgname,
                    pkgver,
                    pkgpath.display()
                );
                remove_file(&pkgpath, dry_run, size_changed, None, Some(dryrun_msg));

                continue;
            }
            // println!("pkgname: {:?}, pkgver: {:?}", pkgname, pkgver);

            if last_pkgname == pkgname {
                versions_of_this_package += 1;
                if versions_of_this_package == amount_to_keep {
                    // we have seen this package too many times, queue for deletion
                    removed_size += fs::metadata(pkgpath)
                        .unwrap_or_else(|_| {
                            panic!("Failed to get metadata of file '{}'", &pkgpath.display())
                        })
                        .len();

                    let dryrun_msg = format!(
                        "dry run: not actually deleting {} {} at {}",
                        pkgname,
                        pkgver,
                        pkgpath.display()
                    );
                    remove_file(&pkgpath, dry_run, size_changed, None, Some(dryrun_msg));
                }
            } else {
                // last_pkgname != pkgname, we got to a new package, reset counter
                versions_of_this_package = 0;
                last_pkgname = pkgname;
            } // if last_pkgname == pkgname
        } // for pkgpath in &crate_list
    }
    println!(
        "Removed {} of compressed crate sources.",
        removed_size.file_size(file_size_opts::DECIMAL).unwrap()
    );
    Ok(())
}

pub(crate) fn remove_dir_via_cmdline(
    directory: Option<&str>,
    dry_run: bool,
    ccd: &CargoCachePaths,
    size_changed: &mut bool,
) -> Result<(), (ErrorKind, String)> {
    fn rm(
        dir: &PathBuf,
        dry_run: bool,
        size_changed: &mut bool,
    ) -> Result<(u64), (ErrorKind, String)> {
        // remove a specified subdirectory from cargo cache
        let msg = Some(format!("removing: '{}'", dir.display()));
        let size = remove_file(&dir, dry_run, size_changed, msg, None);
        // @TODO: use cache instead of recalculating

        Ok(size)
    }

    let input = if let Some(value) = directory {
        value
    } else {
        return Err((
            ErrorKind::RemoveDirNoArg,
            "No argument assigned to --remove-dir, example: 'git-repos,registry-sources'"
                .to_string(),
        ));
    };

    let inputs = input.split(',');
    let valid_dirs = vec![
        "git-db",
        "git-repos",
        "registry-sources",
        "registry-crate-cache",
        "registry-index",
        "registry",
        "all",
    ];

    // keep track of what we want to remove
    let mut rm_git_repos = false;
    let mut rm_git_checkouts = false;
    let mut rm_registry_sources = false;
    let mut rm_registry_crate_cache = false;
    let mut rm_registry_index = false;

    // validate input
    let mut invalid_dirs = String::new();
    let mut terminate: bool = false;

    for word in inputs {
        if valid_dirs.contains(&word) {
            // dir is recognized
            // dedupe
            match word {
                "all" => {
                    rm_git_repos = true;
                    rm_git_checkouts = true;
                    rm_registry_sources = true;
                    rm_registry_crate_cache = true;
                    rm_registry_index = true;
                    // we clean the entire cache anyway,
                    // no need to look further, break out of loop
                    break; // for word in inputs
                }
                "registry" | "registry-crate-cache" => {
                    rm_registry_sources = true;
                    rm_registry_crate_cache = true;
                }
                "registry-sources" => {
                    rm_registry_sources = true;
                }
                "registry-index" => {
                    rm_registry_index = true;
                }
                "git-repos" => {
                    rm_git_checkouts = true;
                }
                "git-db" => {
                    rm_git_repos = true;
                    rm_git_checkouts = true;
                }
                _ => unreachable!(),
            } // match *word
        } else {
            // collect all invalid dirs and print all of them as merged string later
            invalid_dirs.push_str(word);
            invalid_dirs.push_str(" ");
            terminate = true;
        }
    } // for word in inputs
    if terminate {
        // remove trailing whitespace
        let inv_dirs = invalid_dirs.trim();
        return Err((
            ErrorKind::InvalidDeletableDir,
            format!("Invalid deletable dir(s): {}", inv_dirs),
        ));
    }

    let mut size_removed: u64 = 0;

    if dry_run {
        println!(); // newline
    }

    // finally delete
    if rm_git_checkouts {
        size_removed += rm(&ccd.git_checkouts, dry_run, size_changed)?;
    }
    if rm_git_repos {
        size_removed += rm(&ccd.git_repos_bare, dry_run, size_changed)?
    }
    if rm_registry_sources {
        size_removed += rm(&ccd.registry_sources, dry_run, size_changed)?
    }
    if rm_registry_crate_cache {
        size_removed += rm(&ccd.registry_pkg_cache, dry_run, size_changed)?
    }
    if rm_registry_index {
        size_removed += rm(&ccd.registry_index, dry_run, size_changed)?
    }

    if dry_run {
        println!(
            "dry-run: would remove in total: {}",
            size_removed.file_size(file_size_opts::DECIMAL).unwrap()
        );
    }

    Ok(())
}

pub(crate) fn remove_file(
    path: &PathBuf,
    dry_run: bool,
    size_changed: &mut bool,
    deletion_msg: Option<String>,
    dry_run_msg: Option<String>,
) -> u64 {
    let mut size: u64 = 0;
    if dry_run {
        if let Some(dr_msg) = dry_run_msg {
            println!("{}", dr_msg)
        } else {
            size = cumulative_dir_size(path).dir_size;
            let size_hr = size.file_size(file_size_opts::DECIMAL).unwrap();
            println!("dry-run: would remove: '{}' ({})", path.display(), size_hr);
        }
        return size;
    }
    // print deletion message if we have one
    if let Some(msg) = deletion_msg {
        println!("{}", msg);
    }

    if path.is_file() && fs::remove_file(&path).is_err() {
        eprintln!("Warning: failed to remove file \"{}\".", path.display());
    } else {
        *size_changed = true;
    }

    if path.is_dir() && fs::remove_dir_all(&path).is_err() {
        eprintln!(
            "Warning: failed to recursively remove directory \"{}\".",
            path.display()
        );
    } else {
        *size_changed = true;
    }
    size
}
