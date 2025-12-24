use http::Method;
use http::{header::USER_AGENT, uri::Authority};
use http_body_util::Empty;
use hyper::Request;
use hyper::Response;
use hyper::body::Bytes;
use hyper::body::Incoming;
use hyper::client::conn::http1::handshake;
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;
use tokio::time::{Duration, timeout};

// This function makes a request and returns the response
pub(crate) async fn make_request(
    method: &Method,
    addr: &std::net::SocketAddr,
    authority: &Authority,
    path: &str,
    user_agent: &str,
    follow_redirects: bool,
    max_redirects: u8,
) -> Result<Response<Incoming>, Box<dyn std::error::Error + Send + Sync>> {
    let mut current_path = path.to_string();
    let mut redirects_followed: u8 = 0;

    loop {
        // TCP connection
        let stream = TcpStream::connect(addr).await?;
        let io = TokioIo::new(stream);

        // Hyper handshake
        let (mut sender, conn) = handshake::<_, Empty<Bytes>>(io).await?;

        tokio::task::spawn(async move {
            if let Err(err) = conn.await {
                eprintln!("Connection failed: {:?}", err);
            }
        });

        // Build request
        let req = Request::builder()
            .method(method)
            .uri(&current_path)
            .header(hyper::header::HOST, authority.as_str())
            .header(USER_AGENT, user_agent)
            .body(Empty::<Bytes>::new())?;

        let res = sender.send_request(req).await?;

        // Check for redirect
        if follow_redirects && res.status().is_redirection() && redirects_followed < max_redirects {
            if let Some(location) = res.headers().get(hyper::header::LOCATION) {
                let location = location.to_str()?;

                // Update path (assumes same authority)
                current_path = location.to_string();
                redirects_followed += 1;

                // Try again with new path
                continue;
            }
        }

        // Either not a redirect, redirects disabled, or limit reached
        return Ok(res);
    }
}

pub(crate) async fn make_request_with_timeout(
    method: &Method,
    addr: &std::net::SocketAddr,
    authority: &Authority,
    path: &str,
    user_agent: &str,
    follow_redirects: bool,
    max_redirects: u8,
    timeout_duration: Duration,
    retry: bool,
    retry_attempts: u32,
) -> Result<Response<Incoming>, Box<dyn std::error::Error + Send + Sync>> {
    let mut attempts = 0;

    loop {
        let result = timeout(
            timeout_duration,
            make_request(
                method,
                addr,
                authority,
                path,
                user_agent,
                follow_redirects,
                max_redirects,
            ),
        )
        .await;

        match result {
            // Completed within timeout
            Ok(inner) => {
                // Success or non-timeout error → return immediately
                return inner;
            }

            // Timeout
            Err(_) => {
                if !retry || attempts >= retry_attempts {
                    return Err("request timed out".into());
                }

                println!("Retry attempt");
                attempts += 1;
                // optional: add sleep/backoff later
                continue;
            }
        }
    }
}
