//! Bounded, one-shot HTTP transport; callers decide whether a paid call is permitted.
use reqwest::{
    header::{HeaderMap, HeaderName, HeaderValue},
    Client, Method, Url,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};

const MAX_RESPONSE: usize = 1024 * 1024;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpRequest {
    pub url: String,
    pub method: HttpMethod,
    #[serde(default)]
    pub headers: BTreeMap<String, String>,
    pub body: Option<serde_json::Value>,
    pub timeout_ms: u64,
}

#[derive(Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    Get,
    Post,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HttpResponse {
    pub status: u16,
    pub body: String,
    pub elapsed_ms: u64,
}

// No URL, request body, header or server error is included in diagnostics.
#[derive(Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum NetworkError {
    InvalidRequest,
    Timeout,
    Transport,
    ResponseTooLarge,
    InvalidResponse,
}

pub fn client() -> Result<Client, NetworkError> {
    Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .retry(reqwest::retry::never())
        .build()
        .map_err(|_| NetworkError::Transport)
}

pub async fn execute(client: &Client, request: HttpRequest) -> Result<HttpResponse, NetworkError> {
    let url = Url::parse(&request.url).map_err(|_| NetworkError::InvalidRequest)?;
    if !matches!(url.scheme(), "https" | "http")
        || url.host_str().is_none()
        || !url.username().is_empty()
        || url.password().is_some()
        || url.fragment().is_some()
        || !(1..=60_000).contains(&request.timeout_ms)
    {
        return Err(NetworkError::InvalidRequest);
    }
    let mut headers = HeaderMap::new();
    for (name, value) in request.headers {
        let name =
            HeaderName::from_bytes(name.as_bytes()).map_err(|_| NetworkError::InvalidRequest)?;
        let mut value = HeaderValue::from_str(&value).map_err(|_| NetworkError::InvalidRequest)?;
        value.set_sensitive(true);
        headers.insert(name, value);
    }
    let method = match request.method {
        HttpMethod::Get => Method::GET,
        HttpMethod::Post => Method::POST,
    };
    if method == Method::GET && request.body.is_some() {
        return Err(NetworkError::InvalidRequest);
    }
    let mut builder = client
        .request(method, url)
        .headers(headers)
        .timeout(Duration::from_millis(request.timeout_ms));
    if let Some(body) = request.body {
        if serde_json::to_vec(&body)
            .map_err(|_| NetworkError::InvalidRequest)?
            .len()
            > MAX_RESPONSE
        {
            return Err(NetworkError::InvalidRequest);
        }
        builder = builder.json(&body);
    }
    let start = Instant::now();
    let mut response = builder.send().await.map_err(map_error)?;
    let status = response.status().as_u16();
    if response
        .content_length()
        .is_some_and(|size| size > MAX_RESPONSE as u64)
    {
        return Err(NetworkError::ResponseTooLarge);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(map_error)? {
        if bytes.len() + chunk.len() > MAX_RESPONSE {
            return Err(NetworkError::ResponseTooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    let body = String::from_utf8(bytes).map_err(|_| NetworkError::InvalidResponse)?;
    Ok(HttpResponse {
        status,
        body,
        elapsed_ms: start.elapsed().as_millis() as u64,
    })
}

fn map_error(error: reqwest::Error) -> NetworkError {
    if error.is_timeout() {
        NetworkError::Timeout
    } else {
        NetworkError::Transport
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        io::{Read, Write},
        net::TcpListener,
        thread,
    };

    fn server(response: String) -> (String, thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/models", listener.local_addr().unwrap());
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut request = Vec::new();
            let mut buf = [0u8; 1024];
            while !request.windows(4).any(|b| b == b"\r\n\r\n") {
                let n = stream.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                request.extend_from_slice(&buf[..n]);
            }
            let _ = stream.write_all(response.as_bytes());
            String::from_utf8(request).unwrap()
        });
        (url, handle)
    }

    fn request(url: String) -> HttpRequest {
        HttpRequest {
            url,
            method: HttpMethod::Get,
            headers: BTreeMap::new(),
            body: None,
            timeout_ms: 1000,
        }
    }
    fn test_client() -> Client {
        Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .retry(reqwest::retry::never())
            .build()
            .unwrap()
    }

    #[tokio::test]
    async fn net_preserves_headers_and_http_error_status() {
        let (url, server) = server(
            "HTTP/1.1 401 Unauthorized\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}".into(),
        );
        let mut req = request(url);
        req.headers.insert("X-Test".into(), "sample".into());
        let result = execute(&test_client(), req).await.unwrap();
        assert_eq!(result.status, 401);
        assert_eq!(result.body, "{}");
        assert!(server
            .join()
            .unwrap()
            .to_lowercase()
            .contains("x-test: sample"));
    }

    #[tokio::test]
    async fn net_does_not_follow_redirects() {
        let (url, server) = server("HTTP/1.1 302 Found\r\nLocation: http://127.0.0.1:1/secret\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".into());
        assert_eq!(
            execute(&test_client(), request(url)).await.unwrap().status,
            302
        );
        server.join().unwrap();
    }

    #[tokio::test]
    async fn net_rejects_oversize_and_bad_urls_without_leaking_values() {
        let (url, server) = server(format!(
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            MAX_RESPONSE + 1
        ));
        assert!(matches!(
            execute(&test_client(), request(url)).await,
            Err(NetworkError::ResponseTooLarge)
        ));
        server.join().unwrap();
        for url in ["file:///private/key", "https://user:secret@example.com/"] {
            assert!(matches!(
                execute(&test_client(), request(url.into())).await,
                Err(NetworkError::InvalidRequest)
            ));
        }
    }

    #[tokio::test]
    async fn net_timeout_is_bounded_and_classified() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut req = request(format!("http://{}", listener.local_addr().unwrap()));
        req.timeout_ms = 30;
        assert!(matches!(
            execute(&test_client(), req).await,
            Err(NetworkError::Timeout)
        ));
    }
}
