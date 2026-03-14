use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        Arc, RwLock,
        mpsc::{Receiver, Sender},
    },
};

use actix_files::NamedFile;
use actix_web::{
    App, HttpRequest, HttpResponse, HttpServer,
    dev::ServerHandle,
    error::HttpError,
    http::header::{ContentDisposition, DispositionParam, DispositionType},
    web,
};
use rand::{RngExt, distr::Alphanumeric};

use crate::{ipc::IpcServer, util::get_local_ip};

pub struct FileServer {
    pub files: Arc<RwLock<FileMap>>,
    pub port: u16,
    handle: Option<ServerHandle>,
    ipc_server: IpcServer,
    // receiver end of the channel whose sender lives inside ipc_server
    r_filename: Option<Receiver<String>>,
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
    pub fn new(file_names: Vec<PathBuf>, port: u16) -> FileServer {
        let files = Arc::new(RwLock::new(get_files_from_filenames(file_names)));

        // create the channel that connects IpcServer → FileServer
        let (s_filename, r_filename) = std::sync::mpsc::channel();
        let ipc_server = IpcServer::new(s_filename);

        FileServer {
            files,
            port,
            handle: None,
            ipc_server,
            r_filename: Some(r_filename),
        }
    }

    pub fn start(
        &mut self,
        r_should_stop: Receiver<bool>,
        s_new_address: Sender<String>,
    ) -> std::io::Result<()> {
        // start listening for filenames sent over the named pipe
        self.ipc_server
            .listen()
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        // take the receiver out so it can be moved into the actix thread
        let r_filename = self
            .r_filename
            .take()
            .expect("start() called more than once");

        let app_state = web::Data::new(self.files.clone());
        let port = self.port;

        // channel to get the server handle back from the thread
        let (tx, rx) = std::sync::mpsc::channel();
        let files = self.files.clone();

        std::thread::spawn(move || {
            let sys = actix_rt::System::new();
            sys.block_on(async move {
                let server = HttpServer::new(move || {
                    App::new()
                        .app_data(app_state.clone())
                        .route("/", web::get().to(list_files))
                        .route("/{id}", web::get().to(download_file))
                })
                .bind(("0.0.0.0", port))
                .unwrap()
                .run();

                let handle = server.handle();
                tx.send(handle.clone()).unwrap(); // send handle back before blocking

                actix_rt::spawn(async move {
                    if let Ok(graceful) = r_should_stop.recv() {
                        handle.stop(graceful).await;
                    }
                });

                // receive new filenames from IpcServer and register them
                std::thread::spawn(move || {
                    while let Ok(filename) = r_filename.recv() {
                        let generated_filename = generate_filename();
                        let filepath = PathBuf::from(filename.trim());
                        files
                            .write()
                            .unwrap()
                            .insert(generated_filename.clone(), filepath);
                        let full_url = format!(
                            "http://{:?}/{}",
                            get_local_ip().unwrap(),
                            generated_filename
                        );
                        let _ = s_new_address.send(full_url);
                    }
                });

                server.await.unwrap();
            });
        });

        // block until we get the handle, then return
        self.handle = Some(rx.recv().unwrap());
        Ok(())
    }

    pub async fn stop(&self) {
        if let Some(handle) = &self.handle {
            handle.stop(true).await;
        }
    }
}

async fn list_files(files: web::Data<Arc<RwLock<FileMap>>>) -> HttpResponse {
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

    HttpResponse::Ok()
        .content_type("text/html; charset=utf-8")
        .body(body)
}

async fn download_file(
    req: HttpRequest,
    file_id: web::Path<String>,
    files: web::Data<Arc<RwLock<FileMap>>>,
) -> Result<HttpResponse, HttpError> {
    let id = file_id.into_inner();

    let file_path = {
        let files = files.read().unwrap();
        match files.get(&id) {
            Some(f) => f.clone(),
            None => return Ok(HttpResponse::NotFound().finish()),
        }
    };

    let filename = file_path
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    match NamedFile::open(&file_path) {
        Ok(named_file) => Ok(named_file
            .set_content_disposition(ContentDisposition {
                disposition: DispositionType::Attachment,
                parameters: vec![DispositionParam::Filename(filename)],
            })
            .use_last_modified(false)
            .into_response(&req)),
        _ => Ok(HttpResponse::NotFound().finish()),
    }
}
