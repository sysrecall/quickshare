use anyhow::Result;
use interprocess::os::windows::named_pipe::{DuplexPipeStream, PipeListenerOptions, pipe_mode};
use std::{
    io::{BufReader, prelude::*},
    path::{Path, PathBuf},
    sync::mpsc::Sender,
    thread,
};

const PIPE_NAME: &str = r"\\.\pipe\quickshare";
const BUFFER_SIZE: usize = 512;
pub struct IpcServer {
    sender: Sender<String>,
}

impl IpcServer {
    pub fn new(sender: Sender<String>) -> Self {
        Self { sender: sender }
    }
    pub fn listen(&self) -> Result<()> {
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
                // Since our client example sends first, the server should receive a
                // line and only then send a response. Otherwise, because receiving
                // from and sending to a connection cannot be simultaneous without
                // threads or async, we can deadlock the two processes by having both
                // sides wait for the send buffer to be emptied by the other.
                if conn.read_line(&mut buffer).is_err() {
                    eprintln!("Unable to read message from client!");
                };

                // send message to a channel to update file list on the server and
                // regenerate qr code
                if sender.send(buffer.clone()).is_err() {
                    eprintln!("Unable to send message to the channel!");
                };
                // todo!();

                // Now that the receive has come through and the client is waiting
                // on the server's send, do it. (`.get_mut()` is to get the sender,
                // `BufReader` doesn't implement a pass-through `Write`.)
                // conn.get_mut().write_all(b"Hello from server!\n")?;

                // read_line keeps the line feed at the end.
                // print!("Client answered: {buffer}");

                // Clear the buffer so that the next iteration will display new data
                // instead of messages stacking on top of one another.
                buffer.clear();
            }
        });

        Ok(())
    }
}

pub struct IpcClient {}

impl IpcClient {
    pub fn new() -> Self {
        Self {}
    }

    pub fn send(&self, path: PathBuf) {
        let conn = DuplexPipeStream::<pipe_mode::Bytes>::connect_by_path(PIPE_NAME)
            .expect("Unable to create connection to the server!");
        let mut conn = BufReader::new(conn);

        // Append newline so the server's read_line() unblocks cleanly
        let mut message = path.as_os_str().as_encoded_bytes().to_vec();
        message.push(b'\n');

        // BufReader doesn't pass Write through, so we use get_mut()
        conn.get_mut()
            .write_all(&message)
            .expect("Unable to send message to the server!");
    }

    // pub fn send(&self, path: PathBuf) {
    //     let mut conn = DuplexPipeStream::<pipe_mode::Bytes>::connect_by_path(PIPE_NAME)
    //         .expect("Unable to create connection to the server!");

    //     // Append newline so the server's read_line() unblocks cleanly
    //     let mut message = path.as_os_str().as_encoded_bytes().to_vec();
    //     message.push(b'\n');

    //     conn.write_all(&message)
    //         .expect("Unable to send message to the server!");
    // }
    // pub fn send(&self, path: PathBuf) {
    //     // The right size depends on the specifics of the protocol in use.
    //     // let mut buffer = MsgBuf::from(Vec::with_capacity(BUFFER_SIZE));

    //     // Will fail immediately if the server hasn't started yet.
    //     let mut conn = DuplexPipeStream::<pipe_mode::Messages>::connect_by_path(PIPE_NAME)
    //         .expect("Unable to create connection to the server!");

    //     // Here's our message so that we can check the length of sent data.
    //     // const MESSAGE: &[u8] = b"Hello from client!";
    //     let MESSAGE: &[u8] = path.as_os_str().as_encoded_bytes();
    //     // Send the message, getting the amount of bytes that was actually sent in return.
    //     let sent = conn.send(MESSAGE);

    //     if sent.is_err() {
    //         eprintln!("Unable to send message to the server!");
    //         return;
    //     }

    //     // If the length doesn't match, something is seriously wrong.
    //     assert_eq!(sent.unwrap(), MESSAGE.len());

    //     // Use the reliable message receive API, which gets us a RecvResult
    //     // from the recvmsg crate.
    //     // conn.recv_msg(&mut buffer, None)?;

    //     // Avoid holding up resources.
    //     drop(conn);

    //     // Convert the data that's been received into a string. This checks for
    //     // UTF-8 validity, and if invalid characters are found, a new buffer
    //     // is allocated to house a modified version of the received data, where
    //     // decoding errors are replaced with those diamond-shaped question mark
    //     // U+FFFD REPLACEMENT CHARACTER thingies: �.
    //     // let received_string = String::from_utf8_lossy(buffer.filled_part());

    //     // println!("Server answered: {received_string}");
    //     //{
    //     // Ok(())
    // }
}
