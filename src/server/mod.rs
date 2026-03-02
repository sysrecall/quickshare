use std::{collections::HashMap, path::PathBuf, sync::mpsc::Receiver};

use actix_files::NamedFile;
use actix_web::{
    App, HttpRequest, HttpResponse, HttpServer,
    dev::ServerHandle,
    error::HttpError,
    http::header::{ContentDisposition, DispositionParam, DispositionType},
    web,
};
use rand::{RngExt, distr::Alphanumeric};

struct AppState {}

pub struct FileServer {
    pub files: FileMap,
    pub port: u16,
    handle: Option<ServerHandle>,
}

pub type FileMap = HashMap<String, PathBuf>;

impl FileServer {
    pub fn new(file_names: Vec<PathBuf>, port: u16) -> FileServer {
        let mut files = FileMap::new();

        for file_name in file_names {
            let generated_name: String = rand::rng()
                .sample_iter(&Alphanumeric)
                .take(12)
                .map(char::from)
                .collect();

            files.insert(generated_name, file_name);
        }

        FileServer {
            files,
            port,
            handle: None,
        }
    }

    pub async fn start(&mut self, r_should_stop: Receiver<bool>) -> std::io::Result<()> {
        let app_state = web::Data::new(self.files.clone());

        let server = HttpServer::new(move || {
            App::new()
                .app_data(app_state.clone())
                .route("/", web::get().to(list_files))
                .route("/{id}", web::get().to(download_file))
        })
        .bind(("0.0.0.0", 3000))?
        .run();

        let handle = server.handle();
        self.handle = Some(handle.clone());

        actix_rt::spawn(async move {
            if let Ok(graceful) = r_should_stop.recv() {
                handle.stop(graceful).await;
            }
        });

        server.await
    }

    pub async fn stop(&self) {
        if let Some(handle) = &self.handle {
            handle.stop(true).await;
        }
    }
}

async fn list_files(files: web::Data<FileMap>) -> HttpResponse {
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

    for (id, file) in files.iter() {
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
    files: web::Data<FileMap>,
) -> Result<HttpResponse, HttpError> {
    let id = file_id.into_inner();

    let file = match files.get(&id) {
        Some(f) => f,
        None => return Ok(HttpResponse::NotFound().finish()),
    };

    let filename = file
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string();

    match NamedFile::open(&file) {
        Ok(named_file) => Ok(named_file
            .set_content_disposition(ContentDisposition {
                disposition: DispositionType::Attachment,
                parameters: vec![DispositionParam::Filename(filename)],
            })
            .use_last_modified(false)
            .into_response(&req)),
        _ => return Ok(HttpResponse::NotFound().finish()),
    }
}
