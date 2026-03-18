use anyhow::Result;
use interprocess::os::windows::named_pipe::{DuplexPipeStream, PipeListenerOptions, pipe_mode};
use std::{
    io::{BufReader, prelude::*},
    path::{Path, PathBuf},
    thread::{self, JoinHandle},
};

use crossbeam::channel;

const PIPE_NAME: &str = r"\\.\pipe\quickshare";
const BUFFER_SIZE: usize = 512;
pub struct IpcServer {
    // upon message recieve, this is where we forward them
    handle: Option<JoinHandle<()>>,
    sender: channel::Sender<String>,
    pub receiver: channel::Receiver<String>,
}

impl IpcServer {
    pub fn new() -> Self {
        let (sender, receiver) = channel::unbounded();
        Self {
            handle: None,
            sender,
            receiver,
        }
    }
    pub fn listen(&mut self) -> Result<()> {
        let listener = PipeListenerOptions::new()
            .path(Path::new(PIPE_NAME))
            .create_duplex::<pipe_mode::Bytes>()?;

        // This buffer will be reused between clients.
        let mut buffer = String::with_capacity(BUFFER_SIZE);
        let sender = self.sender.clone();

        let handle = thread::spawn(move || {
            for mut conn in listener
                .incoming()
                .filter_map(|conn| {
                    conn.map_err(|e| eprintln!("Incoming connection failed: {e}"))
                        .ok()
                })
                .map(BufReader::new)
            {
                if conn.read_line(&mut buffer).is_err() {
                    eprintln!("Unable to read message from client!");
                };

                if sender.send(buffer.clone()).is_err() {
                    eprintln!("Unable to send message to the channel!");
                };

                buffer.clear();
            }
        });

        self.handle = Some(handle);

        Ok(())
    }
}

pub struct IpcClient {}

impl IpcClient {
    pub fn new() -> Self {
        Self {}
    }
    pub fn send(&self, path: PathBuf) {
        let mut conn = DuplexPipeStream::<pipe_mode::Bytes>::connect_by_path(PIPE_NAME)
            .expect("Unable to connect to server!");
        let mut message = path.as_os_str().as_encoded_bytes().to_vec();
        message.push(b'\n');
        conn.write_all(&message)
            .expect("Unable to send message to server!");
    }
}
