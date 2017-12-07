// https://github.com/LeopoldArkham/humansize
extern crate humansize; // convert bytes to whatever
// https://github.com/BurntSushi/walkdir
extern crate walkdir; // walk CARGO_DIR recursively
// https://github.com/kbknapp/clap-rs
extern crate clap; // cmdline arg parsing
// https://github.com/rust-lang/cargo
extern crate cargo; // obtain CARGO_DIR

use std::fs;
use std::path::Path;
use std::io;

use clap::{Arg, App};
use humansize::{FileSize, file_size_opts as options};
use walkdir::WalkDir;

struct CacheDir<'a> {
    path: &'a std::path::Path, // path object of the dir
    string: &'a str, // string that represents the dir path
}

struct CacheDirCollector<'a> {
    // an object containing all the relevant cache dirs
    // for easy pasing around to functions
    git_checkouts: &'a CacheDir<'a>,
    git_db: &'a CacheDir<'a>,
    registry: &'a CacheDir<'a>,
    //bin_dir: &'a CacheDir<'a>,
}

struct DirInfoObj {
    // make sure we do not accidentally confuse dir_size and file_number
    // since both are of the same type
    dir_size: u64,
    file_number: u64,
}


fn cumulative_dir_size(dir: &str) -> DirInfoObj {
    let dir_path = std::path::Path::new(dir);
    if !dir_path.is_dir() {
        return DirInfoObj {
            dir_size: 0,
            file_number: 0,
        };
    }
    //@TODO add some clever caching
    let mut cumulative_size = 0;
    let mut number_of_files = 0;
    // traverse recursively and sum filesizes
    for entry in WalkDir::new(dir) {
        let entry = entry.unwrap();
        let path = entry.path();
        //println!("{}", path.display());

        if path.is_file() {
            cumulative_size += fs::metadata(path).unwrap().len();
            number_of_files += 1;
        }
    } // walkdir

    DirInfoObj {
        dir_size: cumulative_size,
        file_number: number_of_files,
    }
}


fn rm_dir(cache: &CacheDirCollector) {
    // remove a directory from cargo cache
    let mut dirs_to_delete: Vec<&CacheDir> = Vec::new();

    println!("Possile directories to delete: 'git-checkouts', 'git', 'registry'.");
    println!("'abort' to abort.");

    'inputStrLoop: loop {
        let mut input = String::new();
        io::stdin().read_line(&mut input).expect(
            "Couldn't read input",
        );

        // check what dir we are supposed to delete now
        match input.trim() {
            "git-checkouts" => {
                dirs_to_delete.push(cache.git_checkouts);
                break;
            }
            "git" => {
                // @TODO make sure we print that we are rming bare repos AND checkouts
                dirs_to_delete.push(cache.git_db);
                dirs_to_delete.push(cache.git_checkouts);
                break;
            }
            "registry" => {
                dirs_to_delete.push(cache.registry);
                break;
            }
            "bin-dir" => println!("Please use 'cargo uninstall'."),
            _ => {
                println!("Invalid input.");
                println!("Possile directories to delete: 'git-checkouts', 'git-db', 'registry'.");
                continue 'inputStrLoop;
            } // _
        } // match input
    } // 'inputStrLoop



    println!(
        "Trying to delete {}",
        dirs_to_delete.first().unwrap().string
    );
    println!(
        "Really delete dir {} ? (yes/no)",
        dirs_to_delete.first().unwrap().string
    );

    loop {
        let mut input = String::new();
        io::stdin().read_line(&mut input).expect(
            "Couldn't read line",
        );
        if input.trim() == "yes" {
            println!("deleting {}", dirs_to_delete.first().unwrap().string);
            for dir in dirs_to_delete {
                if dir.path.is_file() {
                    println!("ERROR: {} is not a directory but a file??", dir.string);
                    println!("Doing nothing.");
                } else if dir.path.is_dir() {
                    fs::remove_dir_all(dir.string).unwrap();
                } else {
                    println!("Directory {} does not exist, skipping", dir.string);
                }
            }
            break;
        } else if input == "no" {
            println!("keeping {}", dirs_to_delete.first().unwrap().string);
            break;
        } else {
            println!("Invalid input: {}", input);
            println!("please use 'yes' or 'no'");
        }
    } // loop
} // fn rm_dir



fn main() {

    // parse args
    let cargo_show_cfg = App::new("cargo-show")
        .version("0.1")
        .about("Manage cargo cache")
        .author("matthiaskrgr")
        .arg(Arg::with_name("print-dirs").short("d").long("dirs").help(
            "Show found directory paths.",
        ))
        .arg(
            Arg::with_name("remove-dirs")
                .short("r")
                .long("remove")
                .help("Remove directories in the cache."),
        )
        .get_matches();

    // get the cargo home dir path from cargo
    let cargo_cfg = cargo::util::config::Config::default().unwrap();
    let cargo_home_str = format!("{}", cargo_cfg.home().display());
    let cargo_home_path = Path::new(&cargo_home_str);


    // make sure we actually have a cargo dir
    if !cargo_home_path.is_dir() {
        println!("Error, no '{} dir found", &cargo_home_str);
        std::process::exit(1);
    }


    let cumulative_size_cargo = cumulative_dir_size(&cargo_home_str).dir_size;

    let bin_dir = (cargo_home_path.clone()).join("bin/");
    let bin_dir_str = bin_dir.clone().into_os_string().into_string().unwrap();

    let tmp = cumulative_dir_size(&bin_dir_str);
    let (number_of_bins, cumulative_bin_size) = (tmp.dir_size, tmp.file_number);


    let registry_dir = (cargo_home_path.clone()).join("registry/");
    let registry_dir_str = (registry_dir.clone())
        .into_os_string()
        .into_string()
        .unwrap();

    let cumulative_registry_size = cumulative_dir_size(&registry_dir_str).dir_size;


    let git_db = (cargo_home_path.clone()).join("git/db/");
    let git_db_str = git_db.clone().into_os_string().into_string().unwrap();
    let git_db_size = cumulative_dir_size(&git_db_str).dir_size;


    let git_checkouts = (cargo_home_path.clone()).join("git/checkouts/");
    let git_checkouts_str = (git_checkouts.clone())
        .into_os_string()
        .into_string()
        .unwrap();

    let git_checkouts_size = cumulative_dir_size(&git_checkouts_str).dir_size;


    if cargo_show_cfg.is_present("print-dirs") {
        println!("cargo home: {}", cargo_home_str);
        println!("bin dir: {}", bin_dir_str);
        println!("registry dir: {}", registry_dir_str);
        println!("git db dir: {}", git_db_str);
        println!("checkouts dir: {}", git_checkouts_str);
    }

    //    let cargo_home_cache = CacheDir { path: &cargo_home_path, string:  &cargo_home_str };
    /*let bin_dir_cache = CacheDir {
        path: &bin_dir,
        string: &bin_dir_str,
    };*/
    let registry_dir_cache = CacheDir {
        path: &registry_dir,
        string: &registry_dir_str,
    };
    let git_db_cache = CacheDir {
        path: &git_db,
        string: &git_db_str,
    };
    let checkouts_cache = CacheDir {
        path: &git_checkouts,
        string: &git_checkouts_str,
    };
    // link everything into the CacheDirCollector
    let cargo_cache = CacheDirCollector {
        git_checkouts: &checkouts_cache,
        git_db: &git_db_cache,
        registry: &registry_dir_cache,
        //bin_dir: &bin_dir_cache,
    };


    println!("\nCargo cache:\n");
    println!(
        "Total size: {} ",
        cumulative_size_cargo.file_size(options::DECIMAL).unwrap()
    );
    println!(
        "Size of {} installed binaries {} ",
        number_of_bins,
        cumulative_bin_size.file_size(options::DECIMAL).unwrap()
    );
    println!(
        "Size of registry {} ",
        cumulative_registry_size
            .file_size(options::DECIMAL)
            .unwrap()
    );
    println!(
        "Size of git db  {} ",
        git_db_size.file_size(options::DECIMAL).unwrap()
    );
    println!(
        "Size of git repo checkouts {} ",
        git_checkouts_size.file_size(options::DECIMAL).unwrap()
    );


    if cargo_show_cfg.is_present("remove-dirs") {
        rm_dir(&cargo_cache);
    }

}
