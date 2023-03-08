extern crate serde;

use std::path::PathBuf;

use clap::{Parser, ValueEnum};
use serde::{Deserialize, Serialize};

use anyhow::Result;
use std::fmt;
use url::Url;

#[derive(Debug, Serialize, Deserialize, Parser)]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Config {
    ///Source bitcoind node rpc url i.e. http://localhost, '.' for source_url defined in config
    ///file
    #[arg(group = "source")]
    pub source_ip_addr: String,
    ///Destination bitcoind node rpc url i.e. http://localhost, '.' for dest_url defined in config
    ///file
    #[arg(group = "dest")]
    pub dest_ip_addr: String,

    ///Source cookie auth path
    #[arg(short = 's', long)]
    pub source_cookie_auth_path: Option<PathBuf>,
    ///Destination cookie auth path
    #[arg(short = 'd', long)]
    pub dest_cookie_auth_path: Option<PathBuf>,
    ///User name for source bitcoin node
    #[arg(short = 'n', long, requires = "source")]
    pub source_user: Option<String>,
    ///Password for source bitcoin node
    #[arg(short = 'w', long, requires = "source")]
    pub source_passwd: Option<String>,
    ///Port for source bitcoin node rpc, use only to override --net network defaults
    #[arg(short = 'p', long, requires = "source")]
    pub source_port: Option<u16>,
    ///User name for destination bitcoin node
    #[arg(short = 'N', long, requires = "dest")]
    pub dest_user: Option<String>,
    ///Password for destination bitcoin node
    #[arg(short = 'W', long, requires = "dest")]
    pub dest_passwd: Option<String>,
    ///Port for destination bitcoin node rpc, use only to override --net network defaults
    #[arg(short = 'P', long, requires = "dest")]
    pub dest_port: Option<u16>,
    ///Bitcoin network type. Sets rpc port default.
    #[arg(short = 't',long,default_value_t=Net::MainNet, value_enum)]
    pub net: Net,
    ///ZMQ Interface to receive tx while working and send all at the end.
    #[arg(short = 'z', long, requires = "dest")]
    pub zmq_address: Option<Url>,
    ///Use get_raw_mempool_verbose rpc call which is faster but consumes a lot of mememory.
    #[arg(short, long, default_value_t = false)]
    pub fast_mode: bool,
    ///Show effective configuration
    #[arg(short, long)]
    pub verbose: bool,

    ///Use config in ~/.config/default-config.toml If file do not exists create it with current params
    #[arg(short = 'c', long, group = "file")]
    #[serde(skip)]
    use_config: bool,
    ///Use config in path i.e. ./config.toml If file do not exists create it with current params
    #[arg(short = 'C', long, group = "file")]
    #[serde(skip)]
    use_config_path: Option<PathBuf>,
}

impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Effective configuration:\n")?;
        write!(f, "  Source ip: {}\n", &self.source_ip_addr)?;
        write!(f, "  Source port: {:?}\n", &self.source_port.unwrap())?;
        write!(f, "  Source user name: ****\n")?;
        write!(f, "  Source password: ****\n")?;
        write!(f, "  Destination ip: {}\n", &self.dest_ip_addr)?;
        write!(f, "  Destination port: {:?}\n", &self.dest_port.unwrap())?;
        write!(f, "  Destination user name: ****\n")?;
        write!(f, "  Destination password: ****\n")?;
        write!(f, "  Source cookie auth path: ")?;
        print_pathbuffer(f, &self.source_cookie_auth_path)?;
        write!(f, "  Dest cookie auth path: ")?;
        print_pathbuffer(f, &self.dest_cookie_auth_path)?;
        write!(f, "\n  Network: {:?}\n", &self.net)?;
        write!(f, "  ZMQ Address: ")?;
        match &self.zmq_address {
            Some(address) => write!(f, "{:?}", address.as_ref().to_string())?,
            None => write!(f, "None")?,
        }
        write!(f, "\n  Fast Mode: {:?}\n", &self.fast_mode)?;
        write!(f, "  Verbose: {:?}\n", &self.verbose)?;
        write!(f, "  Config file used: {:?}\n", &self.config_file_used())?;
        Ok(())
    }
}

impl std::default::Default for Config {
    fn default() -> Self {
        Self {
            source_ip_addr: "127.0.0.1".to_string(),
            source_user: None,
            source_passwd: None,
            source_port: None,
            dest_ip_addr: "127.0.0.1".to_string(),
            dest_user: None,
            dest_passwd: None,
            dest_port: None,
            source_cookie_auth_path: None,
            dest_cookie_auth_path: None,
            net: Net::MainNet,
            zmq_address: Url::parse("tcp://127.0.0.1:29000").ok(),
            fast_mode: false,
            use_config: false,
            use_config_path: None,
            verbose: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, ValueEnum)]
pub enum Net {
    MainNet = 8332,
    TestNet = 18332,
    SigNet = 38332,
    RegTest = 18443,
}

impl Config {
    pub fn load() -> Result<Self> {
        let mut cfg = Config::parse();
        if cfg.use_config {
            cfg = confy::load("mempoolcp", None)?;
            cfg.use_config = true;
        }
        if cfg.use_config_path.is_some() {
            let mut config: Config =
                confy::load("mempoolcp", cfg.use_config_path.as_ref().unwrap().to_str())?;
            config.use_config_path = cfg.use_config_path;
            cfg = config;
        }
        if cfg.source_cookie_auth_path.is_none() {
            if cfg.source_user.is_none() {
                cfg.source_user = rpassword::prompt_password("Source bitcoind node user: ").ok();
            }
            if cfg.source_passwd.is_none() {
                cfg.source_passwd =
                    rpassword::prompt_password("Source bitcoind node password: ").ok();
            }
        }
        if cfg.dest_cookie_auth_path.is_none() {
            if cfg.dest_user.is_none() {
                cfg.dest_user = rpassword::prompt_password("Destination bitcoind node user: ").ok();
            }
            if cfg.dest_passwd.is_none() {
                cfg.dest_passwd =
                    rpassword::prompt_password("Destination bitcoind node password: ").ok();
            }
        }
        cfg.source_port = Some(cfg.source_port.unwrap_or(cfg.net as u16));
        cfg.dest_port = Some(cfg.dest_port.unwrap_or(cfg.net as u16));

        Ok(cfg)
    }
    fn config_file_used(&self) -> String {
        if self.use_config {
            "~/.config/default_config".to_string()
        } else if self.use_config_path.is_some() {
            self.use_config_path
                .as_ref()
                .unwrap()
                .to_str()
                .unwrap()
                .to_string()
        } else {
            "None".to_string()
        }
    }
}
fn print_pathbuffer(f: &mut fmt::Formatter, path_buff: &Option<PathBuf>) -> Result<(), fmt::Error> {
    match path_buff {
        Some(path) => match path.to_str() {
            Some(path_str) => write!(f, "{:?}\n", path_str)?,
            None => write!(f, "None\n")?,
        },
        None => write!(f, "None\n")?,
    };
    Ok(())
}
