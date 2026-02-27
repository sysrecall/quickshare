use actix_files::NamedFile;
use actix_web::http::header::{self, ContentDisposition, DispositionParam, DispositionType};
use actix_web::{App, HttpRequest, HttpResponse, HttpServer, Result, web};
use std::collections::HashMap;
use std::env;
use std::path::PathBuf;

type FileMap = HashMap<String, PathBuf>;

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

        body.push_str(&format!("<li><a href=\"/files/{}\">{}</a></li>", id, name));
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
) -> Result<HttpResponse> {
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

    let named = NamedFile::open(&file)?
        .set_content_disposition(ContentDisposition {
            disposition: DispositionType::Attachment,
            parameters: vec![DispositionParam::Filename(filename)],
        })
        .use_last_modified(false);

    Ok(named.into_response(&req))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let mut files = FileMap::new();

    for (idx, arg) in env::args().skip(1).enumerate() {
        let path = PathBuf::from(&arg);

        if !path.exists() {
            eprintln!("Warning: file does not exist: {}", arg);
            continue;
        }

        let id = format!("file{}", idx);
        files.insert(id, path);
    }

    if files.is_empty() {
        eprintln!("No files provided.");
        std::process::exit(1);
    }

    // files.insert(
    //     "dep".into(),
    //     PathBuf::from(r#"E:\projects\rust\quickshare\Cargo.toml"#),
    // );

    let app_state = web::Data::new(files);

    println!("Starting server at: 127.0.0.1:3000");

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .route("/files", web::get().to(list_files))
            .route("/files/{id}", web::get().to(download_file))
    })
    .bind(("0.0.0.0", 3000))?
    .run()
    .await
}
