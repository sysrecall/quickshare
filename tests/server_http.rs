use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, RwLock},
};

use axum::{
    body::{Body, to_bytes},
    http::{Request, StatusCode, header},
};
use quickshare::server::{FileMap, FileServer, router};
use tower::ServiceExt;

fn test_files(entries: &[(&str, &str)]) -> Arc<RwLock<FileMap>> {
    Arc::new(RwLock::new(
        entries
            .iter()
            .map(|(id, path)| (id.to_string(), PathBuf::from(path)))
            .collect::<HashMap<_, _>>(),
    ))
}

#[tokio::test]
async fn list_endpoint_contains_shared_files_and_ids() {
    let app = router(test_files(&[
        ("first", "photo.jpg"),
        ("second", "notes.txt"),
    ]));

    let response = app
        .oneshot(Request::get("/").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let body = String::from_utf8(body.to_vec()).unwrap();
    assert!(body.contains("/first"));
    assert!(body.contains("photo.jpg"));
    assert!(body.contains("/second"));
    assert!(body.contains("notes.txt"));
}

#[tokio::test]
async fn download_endpoint_streams_file_with_download_headers() {
    let path = std::env::temp_dir().join(format!("quickshare-test-{}", std::process::id()));
    tokio::fs::write(&path, b"test payload").await.unwrap();
    let app = router(test_files(&[("download", path.to_str().unwrap())]));

    let response = app
        .oneshot(Request::get("/download").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "application/octet-stream"
    );
    assert_eq!(
        response.headers()[header::CONTENT_DISPOSITION],
        format!(
            "attachment; filename=\"{}\"",
            path.file_name().unwrap().to_string_lossy()
        )
    );
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(body.as_ref(), b"test payload");
    tokio::fs::remove_file(path).await.unwrap();
}

#[tokio::test]
async fn download_endpoint_returns_not_found_for_unknown_id() {
    let app = router(test_files(&[]));

    let response = app
        .oneshot(Request::get("/missing").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn download_endpoint_returns_not_found_when_file_was_removed() {
    let path = std::env::temp_dir().join(format!("quickshare-missing-{}", std::process::id()));
    let app = router(test_files(&[("gone", path.to_str().unwrap())]));

    let response = app
        .oneshot(Request::get("/gone").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn bind_fails_when_configured_port_is_already_used() {
    let occupied = tokio::net::TcpListener::bind(("0.0.0.0", 0)).await.unwrap();
    let port = occupied.local_addr().unwrap().port();
    let server = FileServer::new(Vec::new(), None, port);

    assert!(server.bind().await.is_err());
}

#[tokio::test]
async fn server_stops_after_stop_signal() {
    let server = FileServer::new(Vec::new(), None, 0);
    let listener = server.bind().await.unwrap();
    let (stop_sender, stop_receiver) = crossbeam::channel::unbounded();
    let is_stopping = Arc::new(std::sync::atomic::AtomicBool::new(false));
    let server_stopping = Arc::clone(&is_stopping);

    let task = tokio::spawn(async move {
        let mut server = server;
        server
            .start(listener, stop_receiver, server_stopping)
            .await
            .unwrap();
    });

    stop_sender.send(true).unwrap();
    tokio::time::timeout(std::time::Duration::from_secs(1), task)
        .await
        .expect("server did not stop")
        .unwrap();
    assert!(is_stopping.load(std::sync::atomic::Ordering::Relaxed));
}
