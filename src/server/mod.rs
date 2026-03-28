use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, RwLock, mpsc::Sender},
};

use axum::{
    Router,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::get,
};
use rand::{RngExt, distr::Alphanumeric};
use tokio::net::TcpListener;
use tokio_util::io::ReaderStream;

use crate::util::get_local_ip;

pub struct FileServer {
    pub files: Arc<RwLock<FileMap>>,
    pub port: u16,
    r_filename: Option<crossbeam::channel::Receiver<String>>,
}

pub type FileMap = HashMap<String, PathBuf>;

fn generate_filename() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(12)
        .map(char::from)
        .collect()
}

fn get_files_from_filenames(file_names: Vec<PathBuf>) -> FileMap {
    let mut files = FileMap::new();
    for file_name in file_names {
        files.insert(generate_filename(), file_name);
    }
    files
}

impl FileServer {
    pub fn new(
        file_names: Vec<PathBuf>,
        r_filename: Option<crossbeam::channel::Receiver<String>>,
        port: u16,
    ) -> FileServer {
        let files = Arc::new(RwLock::new(get_files_from_filenames(file_names)));
        FileServer {
            files,
            port,
            r_filename,
        }
    }

    pub fn listen_file_change(&mut self, s_qrgen_new_address: Sender<String>) {
        let r_filename = self
            .r_filename
            .take()
            .expect("listener called more than once");
        let files = self.files.clone();
        let port = self.port;

        tokio::task::spawn_blocking(move || {
            while let Ok(filename) = r_filename.recv() {
                let generated_filename = generate_filename();
                let filepath = PathBuf::from(filename.trim());
                files
                    .write()
                    .unwrap()
                    .insert(generated_filename.clone(), filepath);

                let full_url = match files.read().unwrap().len() {
                    1 => format!(
                        "http://{}:{}/{}",
                        get_local_ip().unwrap(),
                        port,
                        generated_filename
                    ),
                    _ => format!("http://{}:{}", get_local_ip().unwrap(), port),
                };

                let _ = s_qrgen_new_address.send(full_url);
            }
        });
    }

    pub async fn start(
        &mut self,
        r_stop: crossbeam::channel::Receiver<bool>,
    ) -> std::io::Result<()> {
        let files = self.files.clone();
        let port = self.port;

        let app = Router::new()
            .route("/", get(list_files))
            .route("/{id}", get(download_file))
            .with_state(files);

        let listener = TcpListener::bind(("0.0.0.0", port)).await?;
        println!("Listening on 0.0.0.0:{}", port);

        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                // r_stop is a sync crossbeam receiver; bridge it to async via spawn_blocking.
                tokio::task::spawn_blocking(move || {
                    let _ = r_stop.recv(); // blocks until QrGen sends the stop signal
                })
                .await
                .ok();
            })
            .await?;

        Ok(())
    }
}

async fn list_files(State(files): State<Arc<RwLock<FileMap>>>) -> Html<String> {
    let mut body = String::from(
        "<!doctype html>
        <html>
        <head>
            <meta charset='utf-8'>
            <title>QuickShare</title>
            <style>
                body { font-family: sans-serif; padding: 20px; }
                ul { list-style: none; padding: 0; }
                li { margin: 8px 0; }
            </style>
        </head>
        <body>
            <h1>Shared files</h1>
            <ul>",
    );

    for (id, file) in files.read().unwrap().iter() {
        let name = file.file_name().unwrap_or_default().to_string_lossy();
        body.push_str(&format!("<li><a href=\"/{}\">{}</a></li>", id, name));
    }

    body.push_str("</ul></body></html>");
    Html(body)
}

async fn download_file(
    Path(id): Path<String>,
    State(files): State<Arc<RwLock<FileMap>>>,
) -> Response {
    let file_path = {
        let files = files.read().unwrap();
        match files.get(&id) {
            Some(f) => f.clone(),
            None => return StatusCode::NOT_FOUND.into_response(),
        }
    };

    let filename = file_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    match tokio::fs::File::open(&file_path).await {
        Ok(file) => {
            let stream = ReaderStream::new(file);
            let body = axum::body::Body::from_stream(stream);

            Response::builder()
                .header(
                    header::CONTENT_DISPOSITION,
                    format!("attachment; filename=\"{}\"", filename),
                )
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .body(body)
                .unwrap()
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}
