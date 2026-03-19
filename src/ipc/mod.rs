use anyhow::Result;
use crossbeam::channel;
use interprocess::os::windows::named_pipe::{PipeListenerOptions, pipe_mode, tokio::*};

use std::path::{Path, PathBuf};
use std::thread;
use tokio::io::{AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::task::JoinHandle;

const PIPE_NAME: &str = r"\\.\pipe\quickshare";
const BUFFER_SIZE: usize = 512;
pub struct IpcServer {
    // upon message recieve, this is where we forward them
    sender: channel::Sender<String>,
    pub receiver: channel::Receiver<String>,
}

impl IpcServer {
    pub fn new() -> Self {
        let (sender, receiver) = channel::unbounded();
        Self { sender, receiver }
    }

    pub async fn listen(&mut self) -> Result<JoinHandle<()>> {
        let listener = PipeListenerOptions::new()
            .path(Path::new(PIPE_NAME))
            .create_tokio_recv_only::<pipe_mode::Bytes>()?;
        // .create_tokio_duplex::<pipe_mode::Bytes>()?;

        let sender = self.sender.clone();

        let handle = tokio::spawn(async move {
            let result = async move {
                loop {
                    dbg!("Waiting for incoming conns");
                    let conn = match listener.accept().await {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("Accept failed: {e}");
                            continue;
                        }
                    };

                    dbg!("Connection Established");
                    let mut reader = BufReader::new(conn);
                    let mut line = String::new();

                    dbg!("Reading pipe");
                    match reader.read_line(&mut line).await {
                        Ok(0) => eprintln!("Client disconnected without sending"),
                        Ok(_) => {
                            let msg = line.trim_end_matches(['\n', '\r']).to_string();
                            eprintln!("[server] received: {msg}");
                            if let Err(e) = sender.send(msg) {
                                eprintln!("Channel send failed: {e}");
                            }
                        }
                        Err(e) => eprintln!("Read failed: {e}"),
                    }

                    dbg!("Reading complete");
                }
            };

            result.await;
        });

        Ok(handle)
    }
}

pub struct IpcClient {}

impl IpcClient {
    pub fn new() -> Self {
        Self {}
    }
    pub fn send(&self, path: PathBuf) -> JoinHandle<()> {
        let handle = tokio::spawn( async move {
            dbg!("Before establishing connection");
            let mut conn = SendPipeStream::<pipe_mode::Bytes>::connect_by_path(PIPE_NAME)
                .await
                .expect("Unable to connect to server!");

            dbg!("After establishing connection");
            let mut message = path.as_os_str().as_encoded_bytes().to_vec();
            message.push(b'\n');

            dbg!("Sending file from pipe: {}", &message);

            conn.write_all(&message).await.unwrap();

            dbg!("Sent file from pipe");
            conn.shutdown().await.unwrap();

        });

        handle

    }
}
