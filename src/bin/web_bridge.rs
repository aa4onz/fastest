// src/bin/web_bridge.rs
use std::io::{Read, Write};
use std::os::fd::AsRawFd;
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

// Shared memory buffer to hold terminal text
struct SharedBuffer {
    data: Vec<u8>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let buffer = Arc::new(Mutex::new(SharedBuffer { data: Vec::new() }));
    let buffer_clone = Arc::clone(&buffer);

    // 1. Launch your UNCHANGED main rust app using a structural PTY/Pipe wrapper
    let mut child = Command::new("cargo")
        .args(["run", "--bin", "discord_tui_app"]) // Change this to your exact binary name if different
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("Failed to start your main Rust application");

    let mut child_stdout = child.stdout.take().unwrap();
    let mut child_stdin = Arc::new(Mutex::new(child.stdin.take().unwrap()));
    let child_stdin_clone = Arc::clone(&child_stdin);

    // Thread 2: Continuously read text out of your application's layout buffer
    thread::spawn(move || {
        let mut read_buf = [0u8; 1024];
        loop {
            if let Ok(n) = child_stdout.read(&mut read_buf) {
                if n == 0 { break; }
                if let Ok(mut buf) = buffer_clone.lock() {
                    buf.data.extend_from_slice(&read_buf[..n]);
                }
            } else { break; }
        }
    });

    // 3. Bind a native Rust TCP routing listener onto port 8080
    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await?;
    println!("Rust Web Bridge actively running on port 8080...");

    loop {
        let (mut socket, _) = listener.accept().await?;
        let buffer_ref = Arc::clone(&buffer);
        let stdin_ref = Arc::clone(&child_stdin_clone);

        tokio::spawn(async move {
            let mut req_buf = [0u8; 2048];
            let mut raw_socket = socket;
            use tokio::io::{AsyncReadExt, AsyncWriteExt};

            if let Ok(n) = raw_socket.read(&mut req_buf).await {
                let request = String::from_utf8_lossy(&req_buf[..n]);
                
                // Route A: Streaming interface frame adjustments
                if request.contains("GET /stream") {
                    let output_bytes = {
                        let mut buf = buffer_ref.lock().unwrap();
                        std::mem::take(&mut buf.data)
                    };

                    let response = format!(
                        "HTTP/1.1 200 OK\r\nContent-Type: text/plain\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: {}\r\n\r\n",
                        output_bytes.len()
                    );
                    let _ = raw_socket.write_all(response.as_bytes()).await;
                    let _ = raw_socket.write_all(&output_bytes).await;
                
                // Route B: Processing 0ms latency input data from browser
                } else if request.contains("GET /input?msg=") {
                    if let Some(start) = request.find("msg=") {
                        let line = request[start + 4..].split(' ').next().unwrap_or("");
                        let decoded_msg = urlencoding::decode(line).unwrap_or_default().into_owned();
                        
                        if let Ok(mut stdin) = stdin_ref.lock() {
                            let _ = writeln!(stdin, "{}", decoded_msg);
                            let _ = stdin.flush();
                        }
                    }

                    let response = "HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: 0\r\n\r\n";
                    let _ = raw_socket.write_all(response.as_bytes()).await;
                }
            }
        });
    }
}
