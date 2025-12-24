use std::{net::SocketAddr, time::Duration};

use crate::{
    Args,
    status_code_range::{self, StatusCodeRange},
    subcommand::Subcommand,
    utils::make_request_with_timeout,
};

use async_trait::async_trait;
use clap::Parser;
use http::uri::Authority;
use tokio::sync::OnceCell;

#[derive(Clone, Parser)]
#[command(about, long_about = None)]
pub(crate) struct DirArgs {
    /// TODO HTTP status codes (e.g. 200-299,404,500)
    #[arg(long,
        value_delimiter = ',',
        value_parser = clap::value_parser!(StatusCodeRange),
        default_value = "404")]
    status_codes: Vec<StatusCodeRange>,

    /// TODO Negative status codes (overrides --status-codes if set)
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
        word: &str,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let addr = self
            .addr
            .get()
            .expect("pre_run must be called before process_word");
        let authority = self
            .authority
            .get()
            .expect("pre_run must be called before process_word");

        let base_path = args.url.path().trim_end_matches('/');
        let full_path = format!("{}/{}", base_path, word);

        let res = make_request_with_timeout(
            &args.method,
            &addr,
            &authority,
            &full_path,
            &args.user_agent,
            args.follow_redirects,
            args.max_redirects,
            Duration::from_millis(args.timeout),
            args.retry,
            args.retry_attempts,
        )
        .await?;

        let _b = status_code_range::is_code_in_ranges(res.status(), &self.status_codes);

        Ok(())
    }
}
