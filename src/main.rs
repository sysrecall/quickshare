#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use crossbeam::channel::unbounded;
use std::io::Read;
use std::path::PathBuf;
use std::sync::mpsc::channel;
use std::{env, sync};
use std::{io, thread};
use tokio::join;

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

fn main() {
    show_image::run_context(|| run());
}

#[actix_web::main]
async fn run() {
    // multiple files
    // let n = env::args().len();
    // let mut paths: Vec<PathBuf> = Vec::with_capacity(n);

    // for arg in env::args().skip(1) {
    //     let path = PathBuf::from(&arg);

    //     if !path.exists() {
    //         eprintln!("Warning: file does not exist: {}", arg);
    //         continue;
    //     }

    //     paths.push(path);
    // }

    // if paths.is_empty() {
    //     eprintln!("No files provided.");
    //     std::process::exit(1);
    // }

    // single file
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
                    // create an ipc client
                    let ipc_client = IpcClient::new();

                    // send file to running instance
                    let handle = ipc_client.send(path.clone());
                    let res = handle.await;

                    std::process::exit(0);
                }
            }
        }
    }

    // create an ipc server
    let mut ipc_server = IpcServer::new();
    let ipc_receiver = ipc_server.receiver.clone();

    let (s_stop, r_stop) = unbounded::<bool>();
    let (s_new_address, r_new_address) = channel::<String>();

    // start listening
    // setup server config
    const PORT: u16 = 3000;
    let mut server = FileServer::new(vec![path], Some(ipc_receiver), PORT); // creating a vec on the fly
    let root_address = format!("http://{}:{}", get_local_ip().unwrap(), PORT);

    // generate qr code
    let mut qrgen = match server.files.read().unwrap().len() {
        1 => {
            let files = server.files.read().unwrap();
            let (name, _) = files.iter().next().unwrap();
            let address = format!("{}/{}", root_address, name);

            println!("Address: {}", address);

            QrGen::create(address.as_ref(), s_stop, Some(r_new_address)).unwrap()
        }
        _ => QrGen::create(root_address.as_ref(), s_stop, Some(r_new_address)).unwrap(),
    };

    // background thread
    thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();

        rt.block_on(async move {
            tokio::spawn(async move {
                if ipc_server.listen().await.is_err() {
                    eprintln!("error starting IPC server, exiting...");
                    std::process::exit(1);
                }
            });

            tokio::task::spawn_blocking(move || {
                println!("starting server at: {}", root_address);
                server.listen_file_change(s_new_address);
                server.start(r_stop).unwrap();
            });

            // waits forever
            futures::future::pending::<()>().await;
        });
    });

    // show window
    // blocking main thread
    qrgen.show().unwrap();
}
