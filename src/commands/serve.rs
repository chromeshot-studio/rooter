use crate::util::format;
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use tiny_http::{Header, Method, Response, Server, StatusCode};

const WORKERS: usize = 4;

pub fn run(port: Option<u16>, dir: Option<PathBuf>, open_browser: bool) -> Result<()> {
    let root = dir.unwrap_or(std::env::current_dir()?);
    if !root.is_dir() {
        bail!("{} is not a directory", root.display());
    }
    let root = fs::canonicalize(&root)?;

    let port = port.unwrap_or(8080);
    let server = Server::http(("0.0.0.0", port))
        .map_err(|e| anyhow::anyhow!("couldn't bind to port {port}: {e}"))
        .with_context(|| format!("try a different port, or `rooter kill {port}` if something else is using it"))?;
    let server = Arc::new(server);

    let url = format!("http://localhost:{port}");
    format::heading("Serving");
    format::row("Directory", format::clean_path(&root));
    format::row("URL", &url);
    format::info("  press Ctrl+C to stop\n");

    if open_browser {
        let _ = open::that(&url);
    }

    let mut handles = Vec::with_capacity(WORKERS);
    for _ in 0..WORKERS {
        let server = Arc::clone(&server);
        let root = root.clone();
        handles.push(thread::spawn(move || loop {
            match server.recv() {
                Ok(request) => handle(request, &root),
                Err(_) => break,
            }
        }));
    }
    for h in handles {
        let _ = h.join();
    }

    Ok(())
}

fn handle(request: tiny_http::Request, root: &Path) {
    let method = request.method().clone();
    let raw_path = request.url().split('?').next().unwrap_or("/").to_string();
    let decoded = urlencoding::decode(&raw_path).map(|s| s.into_owned()).unwrap_or(raw_path.clone());
    let rel = decoded.trim_start_matches('/');

    if !matches!(method, Method::Get | Method::Head) {
        respond_status(request, 405);
        return;
    }

    let requested = if rel.is_empty() { root.to_path_buf() } else { root.join(rel) };

    let resolved = match fs::canonicalize(&requested) {
        Ok(p) => p,
        Err(_) => {
            respond_status(request, 404);
            return;
        }
    };

    if !resolved.starts_with(root) {
        respond_status(request, 403);
        return;
    }

    if resolved.is_dir() {
        let index = resolved.join("index.html");
        if index.is_file() {
            serve_file(request, &index);
        } else {
            serve_listing(request, &resolved, &decoded);
        }
    } else if resolved.is_file() {
        serve_file(request, &resolved);
    } else {
        respond_status(request, 404);
    }
}

fn serve_file(request: tiny_http::Request, path: &Path) {
    let bytes = match fs::read(path) {
        Ok(b) => b,
        Err(_) => {
            respond_status(request, 500);
            return;
        }
    };
    let content_type = guess_mime(path);
    let header = Header::from_bytes(&b"Content-Type"[..], content_type.as_bytes()).unwrap();
    let response = Response::from_data(bytes).with_header(header);
    let _ = request.respond(response);
}

fn serve_listing(request: tiny_http::Request, dir: &Path, url_path: &str) {
    let mut entries: Vec<String> = fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| {
                    let name = e.file_name().to_string_lossy().to_string();
                    let is_dir = e.file_type().map(|t| t.is_dir()).unwrap_or(false);
                    let suffix = if is_dir { "/" } else { "" };
                    let display_name = format!("{name}{suffix}");
                    let safe = html_escape(&display_name);
                    let href = urlencoding::encode(&display_name).into_owned();
                    format!("<li><a href=\"{href}\">{safe}</a></li>")
                })
                .collect()
        })
        .unwrap_or_default();
    entries.sort();

    let title = html_escape(url_path);
    let body = format!(
        "<!doctype html><meta charset=\"utf-8\"><title>{title}</title><h1>{title}</h1><ul>{}</ul>",
        entries.join("")
    );
    let header = Header::from_bytes(&b"Content-Type"[..], &b"text/html; charset=utf-8"[..]).unwrap();
    let response = Response::from_string(body).with_header(header);
    let _ = request.respond(response);
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn respond_status(request: tiny_http::Request, code: u16) {
    let response = Response::from_string(format!("{code}")).with_status_code(StatusCode(code));
    let _ = request.respond(response);
}

fn guess_mime(path: &Path) -> &'static str {
    match path.extension().and_then(|e| e.to_str()).unwrap_or("").to_lowercase().as_str() {
        "html" | "htm" => "text/html; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "json" => "application/json; charset=utf-8",
        "txt" => "text/plain; charset=utf-8",
        "md" => "text/markdown; charset=utf-8",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "wasm" => "application/wasm",
        "pdf" => "application/pdf",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        _ => "application/octet-stream",
    }
}
