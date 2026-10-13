//! Loopback-only callback relay; the pinned proxy's default forwarder binds all interfaces.
use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};

pub(super) struct Callback {
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}
pub(super) fn bind() -> Result<TcpListener, String> {
    let socket = socket2::Socket::new(
        socket2::Domain::IPV4,
        socket2::Type::STREAM,
        Some(socket2::Protocol::TCP),
    )
    .map_err(|_| "无法创建授权监听器")?;
    socket
        .set_reuse_address(true)
        .map_err(|_| "无法初始化授权监听器")?;
    socket
        .bind(&SocketAddr::from(([127, 0, 0, 1], 1455)).into())
        .map_err(|_| "授权回调端口 1455 被占用，请关闭其他登录窗口后重试")?;
    socket.listen(8).map_err(|_| "无法启动授权监听器")?;
    socket
        .set_nonblocking(true)
        .map_err(|_| "无法设置授权监听器")?;
    Ok(socket.into())
}
fn target(request: &str, state: &str) -> Option<String> {
    let mut lines = request.split("\r\n");
    let mut first = lines.next()?.split_whitespace();
    if first.next()? != "GET" {
        return None;
    }
    let path = first.next()?;
    if !path.starts_with("/auth/callback?") || first.next()? != "HTTP/1.1" {
        return None;
    }
    let host: Vec<_> = lines
        .filter_map(|l| l.split_once(':'))
        .filter(|(k, _)| k.eq_ignore_ascii_case("host"))
        .map(|(_, v)| v.trim())
        .collect();
    if host.len() != 1 || !matches!(host[0], "localhost:1455" | "127.0.0.1:1455") {
        return None;
    }
    let url = reqwest::Url::parse(&format!("http://localhost:1455{path}")).ok()?;
    if url.fragment().is_some() {
        return None;
    }
    let states: Vec<_> = url.query_pairs().filter(|(k, _)| k == "state").collect();
    if states.len() != 1 || states[0].1 != state || state.is_empty() {
        return None;
    }
    if !url
        .query_pairs()
        .any(|(k, v)| (k == "code" || k == "error") && !v.is_empty())
    {
        return None;
    }
    Some(format!("{}/codex/callback?{}", super::BASE, url.query()?))
}
impl Callback {
    pub(super) fn start(listener: TcpListener, state: String) -> Result<Self, String> {
        let stop = Arc::new(AtomicBool::new(false));
        let signal = stop.clone();
        let worker = std::thread::Builder::new()
            .name("codex-callback".into())
            .spawn(move || {
                while !signal.load(Ordering::Relaxed) {
                    match listener.accept() {
                        Ok((mut stream, _)) => respond(&mut stream, &state, &signal),
                        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            std::thread::sleep(Duration::from_millis(30))
                        }
                        Err(_) => break,
                    }
                }
            })
            .map_err(|_| "无法启动授权回调线程")?;
        Ok(Self {
            stop,
            worker: Some(worker),
        })
    }
}
fn respond(stream: &mut TcpStream, state: &str, stop: &AtomicBool) {
    let _ = stream.set_read_timeout(Some(Duration::from_millis(200)));
    let _ = stream.set_write_timeout(Some(Duration::from_millis(500)));
    let mut data = Vec::new();
    let start = Instant::now();
    while data.len() < 8192
        && start.elapsed() < Duration::from_secs(2)
        && !stop.load(Ordering::Relaxed)
    {
        let mut buffer = [0u8; 1024];
        match stream.read(&mut buffer) {
            Ok(0) => break,
            Ok(n) => {
                data.extend_from_slice(&buffer[..n]);
                if data.windows(4).any(|s| s == b"\r\n\r\n") {
                    break;
                }
            }
            Err(e)
                if matches!(
                    e.kind(),
                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                ) => {}
            Err(_) => return,
        }
    }
    if stop.load(Ordering::Relaxed) {
        return;
    }
    let destination = std::str::from_utf8(&data)
        .ok()
        .and_then(|s| target(s, state));
    let response = match destination {
        Some(url) => format!("HTTP/1.1 302 Found\r\nLocation: {url}\r\nCache-Control: no-store\r\nReferrer-Policy: no-referrer\r\nConnection: close\r\nContent-Length: 0\r\n\r\n"),
        None => "HTTP/1.1 400 Bad Request\r\nCache-Control: no-store\r\nConnection: close\r\nContent-Length: 0\r\n\r\n".into(),
    };
    let _ = stream.write_all(response.as_bytes());
}
impl Drop for Callback {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn callback_checks_host_state_and_method() {
        let good = "GET /auth/callback?state=expected&code=one-time-code HTTP/1.1\r\nHost: localhost:1455\r\n\r\n";
        assert!(target(good, "expected")
            .unwrap()
            .starts_with("http://127.0.0.1:18317/codex/callback?"));
        for bad in [
            good.replace("localhost:1455", "evil.test"),
            good.replace("expected", "wrong"),
            good.replace("GET", "POST"),
            good.replace("&code=", "&state=expected&code="),
            good.replace("/auth/callback", "/other"),
        ] {
            assert!(target(&bad, "expected").is_none());
        }
    }
}
