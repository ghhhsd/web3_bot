use dotenv::dotenv;
use log::info;
use std::path::Path;

mod core;
mod dex;
mod engine;
mod service;
mod utils;

#[tokio::main]
async fn main() {
    if let Err(err) = dotenv() {
        println!("env init failed,error is {:?}", err);
        return;
    }

    if let Err(err) = log4rs::init_file(Path::new("./config/log/log4rs.yaml"), Default::default()) {
        println!("log init failed,error is {:?}", err);
        return;
    }

    // utils::jjj::import_env_var();

    // info!("{}","kkkk");
}
