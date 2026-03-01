use actix_files::NamedFile;
use actix_web::cookie::time::error::Format;
use actix_web::http::header::{ContentDisposition, DispositionParam, DispositionType};
use actix_web::{App, HttpRequest, HttpResponse, HttpServer, Result, web};
use get_if_addrs::get_if_addrs;
use image::Luma;
use mdns_sd::{ServiceDaemon, ServiceInfo};
use qrcode::{EcLevel, QrCode, Version};
use rand::RngExt;
use rand::distr::Alphanumeric;
use std::collections::HashMap;
use std::net::{Ipv4Addr, UdpSocket};
use std::path::PathBuf;
use std::time::Duration;
use std::{env, thread};
use win_open;

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

fn get_local_ip() -> Option<String> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("8.8.8.8:80").ok()?;
    match socket.local_addr() {
        Ok(addr) => Some(addr.ip().to_string()),
        Err(_) => None,
    }
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

        let file_name: String = rand::rng()
            .sample_iter(&Alphanumeric)
            .take(12)
            .map(char::from)
            .collect();

        files.insert(file_name, path);
    }

    if files.is_empty() {
        eprintln!("No files provided.");
        std::process::exit(1);
    }

    const PORT: u16 = 3000;

    let local_ip = get_local_ip().unwrap();

    let root_address = format!("http://{}:{}", local_ip, PORT);

    println!("Root: {}", root_address);

    let code = match files.len() {
        1 => {
            let (name, _) = files.iter().next().unwrap();
            let address = format!("{}/{}", root_address, name);
            println!("Address: {}", address);
            QrCode::new(address.as_bytes()).unwrap()
        }
        _ => QrCode::new(root_address.as_bytes()).unwrap(),
    };

    let image = code.render::<Luma<u8>>().build();

    let mut qr_location = env::temp_dir();

    let file_name: String = rand::rng()
        .sample_iter(&Alphanumeric)
        .take(12)
        .map(char::from)
        .collect();

    let file_name = format!("{}.png", file_name);

    qr_location.push(file_name);

    println!("QR Code Location: {:?}", qr_location.display());

    image.save(&qr_location).unwrap();

    win_open::that(&qr_location).expect("Unable to open qr image!");

    let app_state = web::Data::new(files);

    println!("Starting server at: {}", root_address);

    HttpServer::new(move || {
        App::new()
            .app_data(app_state.clone())
            .route("/", web::get().to(list_files))
            .route("/{id}", web::get().to(download_file))
    })
    .bind(("0.0.0.0", 3000))?
    .run()
    .await
}
