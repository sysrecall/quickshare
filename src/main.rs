#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use crossbeam::channel::unbounded;
use std::env;
use std::path::PathBuf;
use std::sync::mpsc::channel;

use qrgen::QrGen;
use server::FileServer;
use windows::Win32::Foundation::{ERROR_ALREADY_EXISTS, GetLastError};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::w;

use crate::ipc::{IpcClient, IpcServer};
use crate::server::get_local_ip;

mod ipc;
mod qrgen;
mod server;
mod util;

fn main() {
    show_image::run_context(|| run());
}

#[tokio::main]
async fn run() {
    // opening the application while selecting multiple files opens the an instance for each file
    // this is the default behaviour of windows context menu, so we only parse single file for now

    // parse single file path from arguments, ignore rest if given
    let path = PathBuf::from(&env::args().skip(1).next().expect("No file provided!"));
    if !path.exists() {
        eprintln!("File does not exist: {:?}", path);
        std::process::exit(1);
    }

    // try to create a named mutex on windows
    // hold mutex guard
    let _mutex_guard = unsafe {
        let mutex = CreateMutexW(None, true, w!("quickshare_mutex")).unwrap_or_else(|e| {
            eprintln!("Failed to create mutex: {:?}", e);
            std::process::exit(1);
        });

        // an app instance already holds the mutex, send file path to the instance via IPC client
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let ipc_client = IpcClient::new();
            let handle = ipc_client.send(path.clone());
            handle
                .await
                .unwrap_or_else(|e| eprintln!("IPC send failed: {e}"));
            std::process::exit(0);
        }

        mutex
    };

    // this is the first instance of the app
    // create an IPC server and start listening for incoming filepaths
    let mut ipc_server = IpcServer::new();
    let ipc_receiver = ipc_server.receiver.clone();

    // init stop signal channel and new address channel
    let (s_stop, r_stop) = unbounded::<bool>();
    let (s_new_address, r_new_address) = channel::<String>();

    // start listening
    // setup server config
    const PORT: u16 = 3000;
    // create vec on the fly, since we only care about the first file
    let mut server = FileServer::new(vec![path], Some(ipc_receiver), PORT);
    let root_address = format!("http://{}:{}", get_local_ip().unwrap(), PORT);

    // generate qr code for the file/s
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

    // start listening on the IPC server
    tokio::spawn(async move {
        if ipc_server.listen().await.is_err() {
            eprintln!("error starting IPC server, exiting...");
            std::process::exit(1);
        }
    });

    // start listening to file changes on the server
    tokio::spawn(async move {
        println!("starting server at: {}", root_address);
        server.listen_file_change(s_new_address);
        server.start(r_stop).await.unwrap();
    });

    // show window
    // blocking main thread
    qrgen.show().unwrap();
}
