#![windows_subsystem = "windows"]

use std::path::PathBuf;
use std::{env, sync::mpsc::channel};

use qrgen::QrGen;
use server::FileServer;
use util::get_local_ip;

mod qrgen;
mod server;
mod util;

#[actix_web::main]
#[show_image::main]
async fn main() {
    // Get files from the args
    let n = env::args().len();
    let mut file_names: Vec<PathBuf> = Vec::with_capacity(n);

    for arg in env::args().skip(1) {
        let path = PathBuf::from(&arg);

        if !path.exists() {
            eprintln!("Warning: file does not exist: {}", arg);
            continue;
        }

        file_names.push(path);
    }

    if file_names.is_empty() {
        eprintln!("No files provided.");
        std::process::exit(1);
    }

    // setup channel
    let (s_should_stop, r_should_stop) = channel::<bool>();

    // setup server config
    const PORT: u16 = 3000;
    let mut server = FileServer::new(file_names, PORT);
    let root_address = get_local_ip().unwrap();

    // generate qr code
    let mut qrgen = match server.files.len() {
        1 => {
            let (name, _) = server.files.iter().next().unwrap();
            let address = format!("http://{}:{}/{}", root_address, PORT, name);
            println!("Address: {}", address);
            QrGen::create(address.as_ref()).unwrap()
        }
        _ => QrGen::create(root_address.as_ref()).unwrap(),
    };

    // start server
    println!("Starting server at: {}:{}", root_address, PORT);
    server.start(r_should_stop).unwrap();

    // generate and show qr
    qrgen.generate_image();
    qrgen.show(s_should_stop).unwrap();
}
