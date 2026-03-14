// #![windows_subsystem = "windows"]

use std::path::PathBuf;
use std::{env, sync::mpsc::channel};

use qrgen::QrGen;
use server::FileServer;
use util::get_local_ip;
use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::w;

use crate::ipc::{IpcClient, IpcServer};

mod ipc;
mod qrgen;
mod server;
mod util;

#[show_image::main]
#[actix_web::main]
async fn main() {
    // multiple files
    // Get files from the args
    // let n = env::args().len();
    // let mut file_names: Vec<PathBuf> = Vec::with_capacity(n);

    // for arg in env::args().skip(1) {
    //     let path = PathBuf::from(&arg);

    //     if !path.exists() {
    //         eprintln!("Warning: file does not exist: {}", arg);
    //         continue;
    //     }

    //     file_names.push(path);
    // }

    // if file_names.is_empty() {
    //     eprintln!("No files provided.");
    //     std::process::exit(1);
    // }

    let path = PathBuf::from(&env::args().skip(1).next().expect("No file provided!"));
    if !path.exists() {
        eprintln!("File does not exist: {:?}", path);
        std::process::exit(1);
    }

    // try to create mutex
    unsafe {
        let mutex = CreateMutexW(None, true, w!("quickshare_mutex"));
        println!("Created mutex: {:?}", mutex);

        // if exist pass to already running instance
        match mutex {
            Err(e) => {
                eprintln!("Failed to create mutex: {:?}", e);
                std::process::exit(1);
            }
            Ok(_handle) => {
                if GetLastError() == ERROR_ALREADY_EXISTS {
                    eprintln!("An instance is already running, exiting...");
                    // create an ipc client
                    let ipc_client = IpcClient::new();
                    ipc_client.send(path.clone());
                    // send file to running instance
                    std::process::exit(0);
                }
            }
        }
    }

    // setup channel
    let (s_should_stop, r_should_stop) = channel::<bool>();

    // else create an ipc server
    // let ipc_server = IpcServer::new(s_filename);

    // start listening
    // setup server config
    const PORT: u16 = 3000;
    let mut server = FileServer::new(vec![path], PORT); // creating a vec on the fly
    let root_address = format!("http://{}:{}", get_local_ip().unwrap(), PORT);

    // generate qr code
    let qrgen = match server.files.read().unwrap().len() {
        1 => {
            let files = server.files.read().unwrap();
            let (name, _) = files.iter().next().unwrap();
            let address = format!("{}/{}", root_address, name);
            println!("Address: {}", address);
            QrGen::create(address.as_ref()).unwrap()
        }
        _ => QrGen::create(root_address.as_ref()).unwrap(),
    };

    let (s_new_address, r_new_address) = std::sync::mpsc::channel::<String>();

    // start server
    println!("Starting server at: {}", root_address);
    server.start(r_should_stop, s_new_address).unwrap();

    // generate and show qr
    // qrgen.generate_image();
    qrgen.show(s_should_stop, r_new_address).unwrap();
}
