use super::screen::{render_health_json, render_metrics};
use super::snapshot::UiSnapshot;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, RwLock};
use std::thread;
use std::time::Duration;

pub fn spawn_http_server(
    bind: String,
    snapshot: Arc<RwLock<UiSnapshot>>,
    stop: Arc<AtomicBool>,
    live_mode: bool,
) -> std::io::Result<thread::JoinHandle<()>> {
    let listener = TcpListener::bind(&bind)?;
    listener.set_nonblocking(true)?;
    Ok(thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    let mut buffer = [0u8; 2048];
                    let read = stream.read(&mut buffer).unwrap_or(0);
                    let request = String::from_utf8_lossy(&buffer[..read]);
                    let path = request
                        .lines()
                        .next()
                        .and_then(|line| line.split_whitespace().nth(1))
                        .unwrap_or("/");
                    let snapshot = snapshot
                        .read()
                        .map(|guard| guard.clone())
                        .unwrap_or_default();
                    let (status, content_type, body) = match path {
                        "/healthz" => (
                            snapshot.health.http_status(),
                            "application/json",
                            render_health_json(&snapshot),
                        ),
                        "/readyz" => {
                            let ready =
                                snapshot.ready && (!live_mode || snapshot.feeds.user_ws.connected);
                            (
                                if ready { 200 } else { 503 },
                                "application/json",
                                format!("{{\"ready\":{},\"now_ms\":{}}}", ready, snapshot.now_ms),
                            )
                        }
                        "/metrics" => (200, "text/plain; version=0.0.4", render_metrics(&snapshot)),
                        _ => (404, "text/plain", "not found".to_string()),
                    };
                    let response = format!(
                        "HTTP/1.1 {} OK\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                        status,
                        content_type,
                        body.len(),
                        body,
                    );
                    let _ = stream.write_all(response.as_bytes());
                    let _ = stream.flush();
                }
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(50));
                }
                Err(_) => break,
            }
        }
    }))
}
