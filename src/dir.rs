use std::net::SocketAddr;

use crate::{
    Args,
    status_code_range::{self, StatusCodeRange},
    subcommand::Subcommand,
};

use async_trait::async_trait;
use clap::Parser;
use http::uri::Authority;
use http_body_util::Empty;
use hyper::Request;
use hyper::body::Bytes;
use hyper::client::conn::http1::handshake;
use hyper_util::rt::TokioIo;
use tokio::net::TcpStream;
use tokio::sync::OnceCell;

#[derive(Clone, Parser)]
#[command(about, long_about = None)]
pub(crate) struct DirArgs {
    /// HTTP status codes (e.g. 200-299,404,500)
    #[arg(long, value_delimiter = ',', value_parser = clap::value_parser!(StatusCodeRange))]
    status_codes: Vec<StatusCodeRange>,

    /// Negative status codes (overrides --status-codes if set)
    #[arg(long, value_delimiter = ',', value_parser = clap::value_parser!(StatusCodeRange))]
    status_codes_blacklist: Vec<StatusCodeRange>,

    #[clap(skip)]
    authority: OnceCell<Authority>,

    #[clap(skip)]
    addr: OnceCell<SocketAddr>,
}

#[async_trait]
impl Subcommand for DirArgs {
    async fn pre_run(&self, args: &Args) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let authority = args.url.authority().ok_or("URL missing authority")?.clone();

        let host = authority.host().to_string();
        let port = authority.port_u16().unwrap_or_else(|| {
            if args.url.scheme_str() == Some("https") {
                443
            } else {
                80
            }
        });

        let mut addrs = tokio::net::lookup_host((host, port)).await?;
        let addr = addrs.next().ok_or("could not resolve address")?;

        self.authority
            .set(authority)
            .map_err(|_| "pre_run called twice")?;

        self.addr.set(addr).map_err(|_| "pre_run called twice")?;

        Ok(())
    }

    async fn process_word(
        &self,
        args: &Args,
        _word: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let addr = self
            .addr
            .get()
            .expect("pre_run must be called before process_word");

        let authority = self
            .authority
            .get()
            .expect("pre_run must be called before process_word");

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
        let path = args
            .url
            .path_and_query()
            .map(|pq| pq.as_str())
            .unwrap_or("/");

        let req = Request::builder()
            .uri(path)
            .header(hyper::header::HOST, authority.as_str())
            .body(Empty::<Bytes>::new())?;

        let res = sender.send_request(req).await?;

        let b = status_code_range::is_code_in_ranges(res.status(), &self.status_codes);

        println!("Response: {}, in range: {}", res.status(), b);

        Ok(())
    }
}
