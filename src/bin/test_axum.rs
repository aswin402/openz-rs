use axum::Router;
use tower_http::services::ServeDir;
use std::fs;
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;

async fn force_utf8(
    req: axum::http::Request<axum::body::Body>,
    next: axum::middleware::Next,
) -> axum::response::Response {
    let mut response = next.run(req).await;
    if let Some(content_type) = response.headers().get(axum::http::header::CONTENT_TYPE) {
        if let Ok(content_type_str) = content_type.to_str() {
            if content_type_str.starts_with("text/html") && !content_type_str.contains("charset") {
                if let Ok(new_val) = axum::http::header::HeaderValue::from_str("text/html; charset=utf-8") {
                    response.headers_mut().insert(axum::http::header::CONTENT_TYPE, new_val);
                }
            }
        }
    }
    response
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let temp_dir = std::env::temp_dir().join("test_axum_serve");
    fs::create_dir_all(&temp_dir)?;
    fs::write(temp_dir.join("index.html"), "<h1>Hello from Axum!</h1>")?;

    println!("Temp dir: {:?}", temp_dir);

    // Bind listener
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let port = listener.local_addr()?.port();
    println!("Listening on port: {}", port);

    // Test with nest_service
    let app = Router::new()
        .nest_service("/", ServeDir::new(&temp_dir))
        .layer(axum::middleware::from_fn(force_utf8));

    let handle = tokio::spawn(async move {
        if let Err(e) = axum::serve(listener, app).await {
            eprintln!("Axum serve error: {:?}", e);
        }
    });

    sleep(Duration::from_millis(500)).await;

    // Test requesting via reqwest
    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{}/index.html", port);
    println!("Requesting URL: {}", url);
    match client.get(&url).send().await {
        Ok(resp) => {
            let status = resp.status();
            let text = resp.text().await?;
            println!("Response status: {}", status);
            println!("Response body: {}", text);
        }
        Err(e) => {
            eprintln!("Request failed: {:?}", e);
        }
    }

    handle.abort();
    let _ = fs::remove_dir_all(&temp_dir);
    Ok(())
}
