use clap::Parser;
use serde::Deserialize;
use std::fmt::Debug;
use std::path::PathBuf;

/// CLI structure that loads the commandline arguments. These arguments will be
/// serialized in this structure
#[derive(Parser, Debug, Default)]
#[clap(author, version, about, long_about = None)]
pub struct Cli {
    #[clap(short, long, default_value = "/opt/etc/ronaldo.cfg")]
    pub config: PathBuf,
}

macro_rules! config_definitions {
    ($($name:ident : $type:ty, $default:expr),+) => {
        #[derive(Deserialize, Debug, Default)]
        pub struct Config {
            $($name: Option<$type>,)*
        }

        impl Config {
            pub fn load() -> Self {
                let cli = Cli::parse();
                let mut cfg  = match std::fs::read_to_string(&cli.config){
                    Ok(raw) => {
                          serde_yaml::from_str::<Config>(&raw).unwrap()
                    }
                    _ => {
                        eprintln!("could not read config {}. defaulting config", cli.config.to_string_lossy());
                        Config::default()
                    }
                };

                $(if cfg.$name.is_none() {
                    cfg.$name = Some($default);
                })*

                cfg
            }

           $( pub fn $name(&self) -> &$type {
               self.$name.as_ref().unwrap()
           })*
        }
    };
}

config_definitions!(
    www_dir: PathBuf,
    PathBuf::from("/opt/share/www"),
    port: u16,
    80,
    host: String,
    "0.0.0.0".to_string(),
    private_key: PathBuf,
    PathBuf::from("../test_certificates/server.key"),
    certificates: PathBuf,
    PathBuf::from("../test_certificates/server.crt"),
    verbose: bool,
    false,
    api_key: String,
    String::new()
);