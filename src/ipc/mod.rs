use anyhow::Result;
use crossbeam::channel;
use interprocess::os::windows::named_pipe::{PipeListenerOptions, pipe_mode, tokio::*};

use std::path::{Path, PathBuf};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::task::JoinHandle;

const PIPE_NAME: &str = r"\\.\pipe\quickshare";
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

        let sender = self.sender.clone();

        let handle = tokio::spawn(async move {
            let result = async move {
                loop {
                    let conn = match listener.accept().await {
                        Ok(c) => c,
                        Err(e) => {
                            eprintln!("Accept failed: {e}");
                            continue;
                        }
                    };

                    let mut reader = BufReader::new(conn);
                    let mut line = String::new();

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
        tokio::spawn(async move {
            let conn = {
                let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(5);
                loop {
                    match SendPipeStream::<pipe_mode::Bytes>::connect_by_path(PIPE_NAME).await {
                        Ok(c) => break c,
                        Err(_) if tokio::time::Instant::now() < deadline => {
                            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        }
                        Err(e) => {
                            eprintln!("Failed to connect to IPC server after 5s: {e}");
                            return;
                        }
                    }
                }
            };

            let mut conn = conn;
            let mut message = path.as_os_str().as_encoded_bytes().to_vec();
            message.push(b'\n');
            conn.write_all(&message).await.unwrap();
            conn.shutdown().await.unwrap();
        })
    }
}
