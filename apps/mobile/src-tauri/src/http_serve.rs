//! Tiny localhost HTTP server for serving cached files with HTTP Range support.
//! Supports keep-alive for video streaming (multiple range requests per connection).

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use tauri::Manager;

use crate::stream_protocol;

fn parse_http_request(
    reader: &mut BufReader<std::net::TcpStream>,
) -> Option<(String, HashMap<String, String>)> {
    let mut first_line = String::new();
    reader.read_line(&mut first_line).ok()?;
    let parts: Vec<&str> = first_line.trim().split_whitespace().collect();
    if parts.len() < 2 {
        return None;
    }
    let path = parts[1].to_string();
    let mut headers = HashMap::new();
    loop {
        let mut line = String::new();
        reader.read_line(&mut line).ok()?;
        let line = line.trim().to_string();
        if line.is_empty() {
            break;
        }
        if let Some((key, val)) = line.split_once(':') {
            headers.insert(key.trim().to_lowercase(), val.trim().to_string());
        }
    }
    Some((path, headers))
}

fn handle_request(
    stream: &mut std::net::TcpStream,
    stream_id: u64,
    headers: &HashMap<String, String>,
    app: &tauri::AppHandle,
) -> bool {
    let slots = app.state::<stream_protocol::StreamSlots>();
    let entry = {
        let map = slots.0.lock().unwrap();
        match map.get(&stream_id) {
            Some(e) => stream_protocol::StreamEntry {
                source_uri: e.source_uri.clone(),
                cache_path: e.cache_path.clone(),
                ext_hint: e.ext_hint.clone(),
            },
            None => {
                let _ = stream.write_all(b"HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n");
                return false;
            }
        }
    };

    let mut entry_mut = entry.clone();
    let cached = match stream_protocol::ensure_cached(app, &mut entry_mut) {
        Ok(v) => v,
        Err(e) => {
            let body = format!("cache error: {}", e);
            let _ = stream.write_all(
                format!("HTTP/1.1 500\r\nContent-Length: {}\r\n\r\n{}", body.len(), body).as_bytes(),
            );
            return false;
        }
    };
    {
        let mut map = slots.0.lock().unwrap();
        if let Some(e) = map.get_mut(&stream_id) {
            e.cache_path = entry_mut.cache_path;
        }
    }

    let (cache_path, len) = cached;
    let mime = stream_protocol::guess_mime(&entry.source_uri, &entry.ext_hint);

    if let Some(range_val) = headers.get("range") {
        if let Some((start, end)) = stream_protocol::parse_range_header(range_val, len) {
            match stream_protocol::read_file_range(&cache_path, start, end) {
                Ok(slice) => {
                    let content_range = format!("bytes {}-{}/{}", start, end, len);
                    let header = format!(
                        "HTTP/1.1 206 Partial Content\r\n\
                         Content-Type: {}\r\n\
                         Content-Length: {}\r\n\
                         Content-Range: {}\r\n\
                         Accept-Ranges: bytes\r\n\
                         Connection: keep-alive\r\n\r\n",
                        mime, slice.len(), content_range
                    );
                    let _ = stream.write_all(header.as_bytes());
                    let _ = stream.write_all(&slice);
                    return true;
                }
                Err(e) => {
                    let body = format!("range error: {}", e);
                    let _ = stream.write_all(
                        format!("HTTP/1.1 500\r\nContent-Length: {}\r\n\r\n{}", body.len(), body).as_bytes(),
                    );
                    return false;
                }
            }
        }
    }

    match std::fs::read(&cache_path) {
        Ok(bytes) => {
            let header = format!(
                "HTTP/1.1 200 OK\r\n\
                 Content-Type: {}\r\n\
                 Content-Length: {}\r\n\
                 Accept-Ranges: bytes\r\n\
                 Connection: keep-alive\r\n\r\n",
                mime, len
            );
            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(&bytes);
        }
        Err(e) => {
            let body = format!("read error: {}", e);
            let _ = stream.write_all(
                format!("HTTP/1.1 500\r\nContent-Length: {}\r\n\r\n{}", body.len(), body).as_bytes(),
            );
            return false;
        }
    }
    true
}

fn handle_connection(
    mut stream: std::net::TcpStream,
    app: &tauri::AppHandle,
) {
    let stream_id = {
        let mut buf = BufReader::new(match stream.try_clone() {
            Ok(s) => s,
            Err(_) => return,
        });
        if let Some((path, _)) = parse_http_request(&mut buf) {
            let path_part = path.split('?').next().unwrap_or(&path);
            let id_str = path_part.trim_start_matches('/');
            id_str.parse::<u64>().ok()
        } else {
            None
        }
    };

    let stream_id = match stream_id {
        Some(id) => id,
        None => {
            let _ = stream.write_all(b"HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\n\r\n");
            return;
        }
    };

    let mut buf_reader = BufReader::new(stream);
    loop {
        let parsed = parse_http_request(&mut buf_reader);

        let (_path, headers) = match parsed {
            Some(v) => v,
            None => return,
        };

        let mut stream_ref = buf_reader.get_ref().try_clone().unwrap();
        if !handle_request(&mut stream_ref, stream_id, &headers, app) {
            return;
        }
    }
}

/// Start a localhost HTTP server on a random port. Returns the port number.
pub fn start(app: tauri::AppHandle) -> Result<u16, String> {
    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    let port = listener.local_addr().map_err(|e| e.to_string())?.port();
    eprintln!("[viewit] http server listening on 127.0.0.1:{}", port);

    std::thread::spawn(move || {
        for stream in listener.incoming().flatten() {
            let app_clone = app.clone();
            std::thread::spawn(move || {
                handle_connection(stream, &app_clone);
            });
        }
    });

    Ok(port)
}
