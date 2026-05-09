use serde_json::Value;
use std::io;
use time::{format_description::well_known::Rfc3339, Duration, OffsetDateTime};
use tiny_http::{Header, Method, Response, ResponseBox, Server, StatusCode};

fn main() {
    const PORT: u16 = 2015;
    const MIN_AGE_DAYS: i64 = 14;

    let server = Server::http(format!("127.0.0.1:{}", PORT)).unwrap();
    eprintln!("cargo-age-proxy listening on 127.0.0.1:{}", PORT);

    for req in server.incoming_requests() {
        let method = req.method().clone();
        let url = req.url().to_string();

        let res = if method != Method::Get {
            Response::empty(405).boxed()
        } else if let Some(path) = url.strip_prefix("/index/") {
            handle_index(path, MIN_AGE_DAYS)
        } else {
            Response::empty(404).boxed()
        };

        let status = res.status_code().0;
        req.respond(res).unwrap();
        eprintln!("{} {} {}", method, url, status);
    }
}

fn handle_index(path: &str, min_age_days: i64) -> ResponseBox {
    let upstream_url = format!("https://index.crates.io/{}", path);

    let mut upstream = match ureq::get(&upstream_url).call() {
        Ok(r) => r,
        Err(e) => {
            eprintln!("ERROR upstream {}: {}", path, e);
            return Response::empty(502).boxed();
        }
    };

    let status = upstream.status().as_u16();

    let mut headers = Vec::new();
    for (name, value) in upstream.headers().iter() {
        if let Ok(h) = Header::from_bytes(name.as_str().as_bytes(), value.as_bytes()) {
            headers.push(h);
        }
    }

    let body = match upstream.body_mut().read_to_string() {
        Ok(s) => s,
        Err(e) => {
            eprintln!("ERROR reading body for {}: {}", path, e);
            return Response::empty(500).boxed();
        }
    };

    if path == "config.json" {
        return Response::new(
            StatusCode(status),
            headers,
            io::Cursor::new(body.into_bytes()),
            None,
            None,
        )
        .boxed();
    }

    let cutoff = OffsetDateTime::now_utc() - Duration::days(min_age_days);
    let format = Rfc3339;

    let mut out = Vec::new();
    for line in body.lines() {
        if line.is_empty() {
            continue;
        }

        let mut entry: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("unparseable index line: {}", e);
                continue;
            }
        };

        if let Some(pubtime_str) = entry.get("pubtime").and_then(|v| v.as_str()) {
            if let Ok(pubtime) = OffsetDateTime::parse(pubtime_str, &format) {
                if pubtime > cutoff {
                    entry["yanked"] = Value::Bool(true);
                    if let (Some(name), Some(vers)) = (
                        entry.get("name").and_then(|v| v.as_str()),
                        entry.get("vers").and_then(|v| v.as_str()),
                    ) {
                        eprintln!("yanked {}@{} (published {})", name, vers, pubtime.date());
                    }
                }
            }
        }

        match serde_json::to_vec(&entry) {
            Ok(serialized) => {
                out.extend_from_slice(&serialized);
                out.push(b'\n');
            }
            Err(e) => {
                eprintln!("ERROR marshaling response: {}", e);
                return Response::empty(500).boxed();
            }
        }
    }

    let len = out.len();
    Response::new(
        StatusCode(status),
        headers,
        io::Cursor::new(out),
        Some(len),
        None,
    )
    .boxed()
}
