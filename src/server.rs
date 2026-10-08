use crate::{
    Error, Result, canonical,
    model::Artifact,
    query::{self, Query},
    store::Cache,
};
use std::{io::Read, net::SocketAddr};
use tiny_http::{Header, Method, Request, Response, Server};

pub fn bind(address: SocketAddr) -> Result<Server> {
    bind_with_network(address, false)
}

pub fn bind_with_network(address: SocketAddr, allow_network: bool) -> Result<Server> {
    if !address.ip().is_loopback() && !allow_network {
        return Err(Error::new(
            "unsupported",
            "non-loopback listeners require explicit --allow-network",
        ));
    }
    Server::http(address).map_err(|e| Error::new("io", e.to_string()))
}

fn body(request: &mut Request) -> Result<Vec<u8>> {
    if request
        .body_length()
        .is_some_and(|n| n > canonical::MAX_BYTES)
    {
        return Err(Error::new("too_large", "request body exceeds 4 MiB"));
    }
    let mut bytes = Vec::new();
    request
        .as_reader()
        .take((canonical::MAX_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn route(request: &mut Request, cache: &Cache, publishers: &[String]) -> Result<Vec<u8>> {
    let method = request.method().clone();
    let path = request.url().to_owned();
    match (method, path.as_str()) {
        (Method::Get, "/health") => Ok(br#"{"status":"ok","mode":"public_cache"}"#.to_vec()),
        (Method::Get, path) if path.starts_with("/v1/artifacts/") => {
            canonical::encode(&cache.get(&path[14..])?)
        }
        (Method::Post, "/v1/search") => {
            let query: Query = canonical::parse(&body(request)?)?;
            canonical::encode(&query::search(cache, &query)?)
        }
        (Method::Post, "/v1/artifacts") => {
            if publishers.is_empty() {
                return Err(Error::new("forbidden", "HTTP publication is disabled"));
            }
            let artifact: Artifact = canonical::parse(&body(request)?)?;
            if !publishers.contains(&artifact.payload.publisher) {
                return Err(Error::new("forbidden", "publisher is not allowed"));
            }
            canonical::encode(&serde_json::json!({"address":cache.put(&artifact)?}))
        }
        _ => Err(Error::new("not_found", "unknown route")),
    }
}

pub fn handle(mut request: Request, cache: &Cache, publishers: &[String]) -> Result<()> {
    let (status, bytes) = match route(&mut request, cache, publishers) {
        Ok(bytes) => (200, bytes),
        Err(error) => {
            let status = match error.code {
                "forbidden" => 403,
                "not_found" => 404,
                "too_large" => 413,
                "io" => 500,
                _ => 400,
            };
            (status, serde_json::to_vec(&error)?)
        }
    };
    let header = Header::from_bytes("Content-Type", "application/json")
        .map_err(|_| Error::new("io", "invalid header"))?;
    request.respond(
        Response::from_data(bytes)
            .with_status_code(status)
            .with_header(header),
    )?;
    Ok(())
}

pub fn serve(server: Server, cache: &Cache, publishers: &[String]) -> Result<()> {
    for request in server.incoming_requests() {
        if let Err(error) = handle(request, cache, publishers) {
            eprintln!("request failed: {error}");
        }
    }
    Ok(())
}
