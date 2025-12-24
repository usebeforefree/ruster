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
    addr: &std::net::SocketAddr,
    authority: &Authority,
    path: &str,
    user_agent: &str,
) -> Result<Response<Incoming>, Box<dyn std::error::Error + Send + Sync>> {
    // TCP connection
    let stream = TcpStream::connect(addr).await?;
    let io = TokioIo::new(stream);

    // Hyper handshake
    let (mut sender, conn) = handshake::<_, Empty<Bytes>>(io).await?;

    // Spawn the connection task
    tokio::task::spawn(async move {
        if let Err(err) = conn.await {
            eprintln!("Connection failed: {:?}", err);
        }
    });

    // Build the request
    let req = Request::builder()
        .uri(path)
        .header(hyper::header::HOST, authority.as_str())
        .header(USER_AGENT, user_agent)
        .body(Empty::<Bytes>::new())?;

    // Send request and return response
    let res = sender.send_request(req).await?;
    Ok(res)
}

pub(crate) async fn make_request_with_timeout(
    addr: &std::net::SocketAddr,
    authority: &Authority,
    path: &str,
    user_agent: &str,
    timeout_duration: Duration,
) -> Result<Response<Incoming>, Box<dyn std::error::Error + Send + Sync>> {
    let res = timeout(
        timeout_duration,
        make_request(addr, authority, path, user_agent),
    )
    .await
    .map_err(|_| "request timed out")??;

    Ok(res)
}
