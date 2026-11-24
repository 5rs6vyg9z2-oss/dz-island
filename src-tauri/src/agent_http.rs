//! Bounded HTTP transport for native, authenticated localhost agent clients.
use std::io::Read;
use std::net::TcpStream;
use std::time::{Duration, Instant};

const MAX_HEAD: usize = 8192;
const MAX_BODY: usize = 8192;

pub struct Request {
    pub ask: bool,
    pub body: Vec<u8>,
}

fn equal_secret(a: &str, b: &str) -> bool {
    a.len() == b.len() && a.bytes().zip(b.bytes()).fold(0u8, |v, (x, y)| v | (x ^ y)) == 0
}

fn parse_head(bytes: &[u8], token: &str) -> Result<(bool, usize), &'static str> {
    if bytes.len() > MAX_HEAD { return Err("431 Request Header Fields Too Large"); }
    let head = std::str::from_utf8(bytes).map_err(|_| "400 Bad Request")?;
    let mut lines = head.split("\r\n");
    let ask = match lines.next().unwrap_or("") {
        "POST /activity HTTP/1.1" | "POST /activity HTTP/1.0" => false,
        "POST /ask HTTP/1.1" | "POST /ask HTTP/1.0" => true,
        _ => return Err("404 Not Found"),
    };
    let (mut auth, mut length) = (None, None);
    for line in lines {
        if line.is_empty() { break; }
        let (name, value) = line.split_once(':').ok_or("400 Bad Request")?;
        if name.is_empty() || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-') {
            return Err("400 Bad Request");
        }
        match name.to_ascii_lowercase().as_str() {
            "authorization" => {
                if auth.is_some() { return Err("400 Bad Request"); }
                auth = Some(value.trim());
            }
            "content-length" => {
                let value = value.trim();
                if length.is_some() || value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                    return Err("400 Bad Request");
                }
                length = Some(value.parse::<usize>().map_err(|_| "413 Payload Too Large")?);
            }
            "transfer-encoding" => return Err("400 Bad Request"),
            // Hooks use native clients. Browser-origin requests are not supported.
            "origin" => return Err("403 Forbidden"),
            _ => {}
        }
    }
    if !equal_secret(auth.unwrap_or(""), &format!("Bearer {token}")) {
        return Err("401 Unauthorized");
    }
    let length = length.ok_or("411 Length Required")?;
    if length > MAX_BODY { return Err("413 Payload Too Large"); }
    Ok((ask, length))
}

fn read_before(stream: &mut TcpStream, buffer: &mut [u8], deadline: Instant) -> Result<usize, &'static str> {
    let remaining = deadline.checked_duration_since(Instant::now()).ok_or("408 Request Timeout")?;
    if remaining.is_zero() { return Err("408 Request Timeout"); }
    stream.set_read_timeout(Some(remaining)).map_err(|_| "400 Bad Request")?;
    match stream.read(buffer) {
        Ok(0) => Err("400 Bad Request"),
        Ok(n) => Ok(n),
        Err(e) if matches!(e.kind(), std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock) => Err("408 Request Timeout"),
        Err(_) => Err("400 Bad Request"),
    }
}

pub fn read_request(stream: &mut TcpStream, token: &str, budget: Duration) -> Result<Request, &'static str> {
    let deadline = Instant::now() + budget;
    let mut bytes = Vec::new();
    let mut chunk = [0u8; 1024];
    let head_end = loop {
        let n = read_before(stream, &mut chunk, deadline)?;
        bytes.extend_from_slice(&chunk[..n]);
        if let Some(pos) = bytes.windows(4).position(|s| s == b"\r\n\r\n") {
            let end = pos + 4;
            if end > MAX_HEAD { return Err("431 Request Header Fields Too Large"); }
            break end;
        }
        if bytes.len() >= MAX_HEAD { return Err("431 Request Header Fields Too Large"); }
    };
    let (ask, length) = parse_head(&bytes[..head_end], token)?;
    let mut body = bytes.split_off(head_end);
    if body.len() > length { return Err("400 Bad Request"); }
    while body.len() < length {
        let wanted = (length - body.len()).min(chunk.len());
        let n = read_before(stream, &mut chunk[..wanted], deadline)?;
        body.extend_from_slice(&chunk[..n]);
    }
    Ok(Request { ask, body })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::net::TcpListener;

    fn head(extra: &str) -> String {
        format!("POST /activity HTTP/1.1\r\nAuthorization: Bearer secret\r\n{extra}\r\n")
    }
    #[test] fn authorized_native_request_is_accepted() {
        assert_eq!(parse_head(head("Content-Length: 2\r\n").as_bytes(), "secret"), Ok((false, 2)));
    }
    #[test] fn authentication_is_required() {
        assert_eq!(parse_head(head("Content-Length: 2\r\n").as_bytes(), "other"), Err("401 Unauthorized"));
    }
    #[test] fn ambiguous_or_unbounded_bodies_are_rejected() {
        for fields in ["Content-Length: 1\r\nContent-Length: 2\r\n", "Content-Length: -1\r\n", "Transfer-Encoding: chunked\r\nContent-Length: 2\r\n"] {
            assert!(parse_head(head(fields).as_bytes(), "secret").is_err());
        }
        assert_eq!(parse_head(head("Content-Length: 8193\r\n").as_bytes(), "secret"), Err("413 Payload Too Large"));
    }
    #[test] fn browser_origins_and_duplicate_auth_are_rejected() {
        assert_eq!(parse_head(head("Origin: https://example.invalid\r\nContent-Length: 2\r\n").as_bytes(), "secret"), Err("403 Forbidden"));
        assert!(parse_head(head("Authorization: Bearer secret\r\nContent-Length: 2\r\n").as_bytes(), "secret").is_err());
    }
    #[test] fn terminated_oversized_header_is_still_rejected() {
        let oversized = head(&format!("Content-Length: 2\r\nX-Padding: {}\r\n", "a".repeat(MAX_HEAD)));
        assert_eq!(parse_head(oversized.as_bytes(), "secret"), Err("431 Request Header Fields Too Large"));
    }
    #[test] fn body_is_received_across_multiple_packets() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (mut server, _) = listener.accept().unwrap();
        let sender = std::thread::spawn(move || {
            client.write_all(head("Content-Length: 2\r\n").as_bytes()).unwrap();
            client.write_all(b"{").unwrap();
            std::thread::sleep(Duration::from_millis(10));
            client.write_all(b"}").unwrap();
        });
        let request = read_request(&mut server, "secret", Duration::from_secs(1)).unwrap();
        assert!(!request.ask);
        assert_eq!(request.body, b"{}");
        sender.join().unwrap();
    }
    #[test] fn slow_trickle_cannot_extend_the_total_deadline() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let mut client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (mut server, _) = listener.accept().unwrap();
        let sender = std::thread::spawn(move || {
            for _ in 0..20 {
                if client.write_all(b"P").is_err() { break; }
                std::thread::sleep(Duration::from_millis(20));
            }
        });
        let started = Instant::now();
        let result = read_request(&mut server, "secret", Duration::from_millis(100));
        assert!(matches!(result, Err("408 Request Timeout")));
        assert!(started.elapsed() < Duration::from_secs(1));
        drop(server);
        sender.join().unwrap();
    }
}
