use std::net::UdpSocket;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, Sender};
use std::{env, sync::mpsc::channel};

use crate::{qrgen::QrGen, server::FileServer};
use show_image::{ImageInfo, ImageView, create_window};

mod qrgen;
mod server;

fn get_local_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    match socket.local_addr() {
        Ok(addr) => Some(addr.ip().to_string()),
        Err(_) => None,
    }
}

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
            let address = format!("{}:{}/{}", root_address, PORT, name);
            println!("Address: {}", address);
            QrGen::create(address.as_ref()).unwrap()
        }
        _ => QrGen::create(root_address.as_ref()).unwrap(),
    };

    qrgen.generate_image();
    qrgen.show(s_should_stop).unwrap();

    // start server
    println!("Starting server at: {}:{}", root_address, PORT);
    server.start(r_should_stop).await.unwrap();
}
