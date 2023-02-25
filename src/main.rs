use std::collections::HashMap;
use std::env;
use std::fs::{metadata, read_dir};
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;
use std::thread::sleep;
use std::time::SystemTime;

struct PrevFile {
    mtime: SystemTime,
    prev_size: u64,
}

fn get_file_mod_time(file: &PathBuf) -> SystemTime {
    metadata(file)
        .expect("Failed to get file metadata.")
        .modified()
        .expect("Failed to get modified time.")
}

fn is_file(path: &PathBuf) -> bool {
    path.is_file()
}

fn get_file_size(file: &PathBuf) -> u64 {
    metadata(file).expect("Failed to get file metadata.").len()
}

fn collect_mod_times(dir: &PathBuf, map: &mut HashMap<PathBuf, PrevFile>) {
    // save initial times
    for entry in read_dir(&dir).unwrap() {
        let path = entry.expect("Failed to read entry from cwd.").path();
        if is_file(&path) {
            let time = get_file_mod_time(&path);
            let size = get_file_size(&path);
            let pfile = PrevFile {
                mtime: time,
                prev_size: size,
            };

            map.insert(path, pfile);
        }
    }
}

fn main() -> std::io::Result<()> {
    // build command for file transfer
    // hostname is first arg, remote path is second arg
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 3 {
        println!("Wrong number of arguments. ./nvim-watcher <remote-host> <remote-path>");
        std::process::exit(-1);
    }
    let hostname = &args[1];
    let remote_path = &args[2];

    let mut child = Command::new("sftp")
        .args([hostname])
        .stdin(std::process::Stdio::piped())
        .spawn()?;

    let child_stdin = child.stdin.as_mut().unwrap();

    // collect mod times into map
    let mut last_mod: HashMap<PathBuf, PrevFile> = HashMap::new();
    let cwd = env::current_dir()?;
    collect_mod_times(&cwd, &mut last_mod);

    loop {
        // check all the files newest mod time
        for (file, pfile) in last_mod.iter_mut() {
            let mod_time = get_file_mod_time(&file);
            let curr_size = get_file_size(&file);

            // check if file changes (newer mod time)
            if !mod_time.duration_since(pfile.mtime).unwrap().is_zero() {
                println!("Detected file change for {} !", file.display());
                let rem_file = format!("{}{:?}", remote_path, file.file_name().unwrap());

                // actual string we send to SFTP to put the file on remote host
                let put_cmd = format!("put {:?} {}", file.file_name().unwrap(), rem_file);

                // write to child stdin, dont drop the stdin after,
                // since we will use it later
                child_stdin.write_all(format!("{}\n", put_cmd).as_bytes())?;

                // update last_mod_time in map
                (*pfile).mtime = mod_time;
                (*pfile).prev_size = curr_size;
            }
        }

        // sleep 300 millis before checking again
        sleep(std::time::Duration::new(0, 300000000));
    }
}
