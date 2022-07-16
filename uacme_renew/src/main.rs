mod log;

use ::log::{error, info};
use clap::Parser;
use daemonize::Daemonize;
use ronaldos_config::get_webserver_pid;
use std::{
    fs::File,
    path::{Path, PathBuf},
    process::Command,
    thread,
    time::Duration,
};

#[derive(Parser)]
struct Args {
    #[clap(short, long, default_value = ronaldos_config::CFG_PATH)]
    pub config: PathBuf,
    #[clap(short)]
    pub now: bool,
}

fn main() {
    log::init().unwrap();
    let cli = Args::parse();
    let config = ronaldos_config::get_application_config(&cli.config);
    let duration = Duration::from_secs(config.interval_days() * 24 * 60 * 60);

    let script_path: PathBuf = PathBuf::from("/opt/share/uacme/uacme.sh");
    if !script_path.exists() {
        error!(
            "{} does not exist. did you install the uacme package?",
            script_path.to_string_lossy()
        );
        return;
    }

    let host = format!("www.{}", config.hostname());

    if cli.now {
        execute(&script_path, &host, config.www_dir());
        return;
    }

    daemonize();

    loop {
        info!("waking up in {:?}", duration);
        thread::sleep(duration);
        execute(&script_path, &host, config.www_dir());
    }
}

fn execute(script_path: &Path, host: &String, www: &Path) {
    let uacme_challenge_path = format!("UACME_CHALLENGE_PATH={}", www.to_string_lossy());
    let uacme_args = [
        &uacme_challenge_path,
        "uacme",
        "-h",
        &script_path.to_string_lossy(),
        "issue",
        host,
    ];

    if get_webserver_pid().is_none() {
        if !webserver_command(false) {
            error!("ronaldos_webserver must be running");
        }
    }

    match Command::new("/bin/bash").args(uacme_args).output() {
        Ok(o) => {
            if o.status.success() {
                info!("renew certificates succeeded");
                if !webserver_command(true) {
                    error!("restart of ronaldos_webserver might not be successful");
                }
            } else {
                error!("renew of certificates returned statuscode {}", o.status);
            }
        }
        Err(e) => {
            error!("error executing uacme process {}", e);
        }
    }
}

fn daemonize() {
    const STDOUT: &str = concat!("/opt/var/", env!("CARGO_PKG_NAME"));
    const PID: &str = "/opt/var/run/ronaldo_uacme.pid";

    let stdout = create_if_not_exists(format!("{}/daemon.out", STDOUT));
    let stderr = create_if_not_exists(format!("{}/daemon.err", STDOUT));
    Daemonize::new()
        .pid_file(PID)
        //.chown_pid(true)
        .stdout(stdout)
        .stderr(stderr)
        .start()
        .ok();
}

fn webserver_command(restart: bool) -> bool {
    let start = match restart {
        true => "restart",
        false => "start",
    };

    Command::new("/bin/bash")
        .arg("ronaldos_webserver")
        .arg("-d")
        .arg(start)
        .output()
        .map(|o| o.status.success())
        .unwrap_or_default()
}

fn create_if_not_exists<P: AsRef<Path>>(path: P) -> File {
    let path: &Path = path.as_ref();
    let parent = path.parent().unwrap();
    if !parent.exists() {
        std::fs::create_dir_all(parent).unwrap();
    }
    std::fs::File::create(path).unwrap()
}
