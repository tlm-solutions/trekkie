extern crate clap;
//extern crate derive_builder;

use clap::Parser;
use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
pub struct Response {
    pub success: bool,
}

#[derive(Parser, Debug)]
#[clap(name = "dump-dvb telegram collection sink")]
#[clap(author = "dvb-dump@protonmail.com")]
#[clap(version = "0.1.0")]
#[clap(about = "data collection server", long_about = None)]
pub struct Args {
    #[arg(short, long, default_value_t = String::from("127.0.0.1"))]
    pub api_host: String,

    #[arg(short, long, default_value_t = 8080)]
    pub port: u16,

    #[arg(short, long, action)]
    pub swagger: bool,

}

#[derive(Deserialize, Serialize, Debug)]
pub struct StopConfig {
    pub name: String,
    pub lat: f64,
    pub lon: f64,
}
