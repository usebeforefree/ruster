mod dir;
mod status_code_range;
mod subcommand;
mod utils;

use crate::subcommand::Subommand;

use async_channel::bounded;
use clap::{ArgAction, Parser};
use cookie::{Cookie, CookieJar};
use http::header::USER_AGENT;
use hyper::header::{HeaderName, HeaderValue};
use hyper::{Method, Uri};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs::File;
use tokio::io::{self, AsyncBufReadExt};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let args = Arc::new(Args::parse());

    let subcommand = subcommand::to_subcommand(args.command.clone())?;
    subcommand.pre_run(&args.clone()).await.unwrap();

    // multiple consumer single producer pool
    let mut handles = Vec::new();

    let (tx, rx) = bounded::<String>(128);

    for id in 0..args.threads {
        let rx = rx.clone();
        let subcommand = subcommand.clone();
        let args = args.clone();

        let handle = tokio::spawn(async move {
            while let Ok(word) = rx.recv().await {
                if let Err(e) = subcommand.process_word(&args, &word).await {
                    eprintln!("worker {id} request error: {e}");
                }

                // add delay between requests
                if args.delay > 0 {
                    sleep(Duration::from_millis(args.delay)).await;
                }
            }
        });

        handles.push(handle);
    }

    // read wordlist file
    let file = File::open(&args.wordlist).await?;
    let reader = io::BufReader::new(file);
    let mut wordlist = reader.lines();

    // skip the first `wordlist_offset` lines
    for _ in 0..args.wordlist_offset {
        if wordlist.next_line().await?.is_none() {
            break;
        }
    }

    // send the remaining words
    while let Some(word) = wordlist.next_line().await? {
        tx.send(word).await.unwrap();
    }

    drop(tx);

    for handle in handles {
        let _ = handle.await;
    }

    Ok(())
}

#[derive(clap::Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[command(subcommand)]
    command: Subommand,

    /// Set the User-Agent string
    #[arg(
        long,
        default_value_t = String::from(concat!("buster ", env!("CARGO_PKG_VERSION"))))]
    user_agent: String,

    /// TODO Use random User-Agent strings
    #[arg(long, default_value_t = false)]
    random_agent: bool,

    /// TODO Proxy to use for requests [http(s)://host:port] or [socks5://host:port]
    #[arg(long, value_parser = proxy_parser)]
    proxy: Option<Uri>,

    /// HTTP Timeout in Ms
    #[arg(long, default_value_t = 10_000)]
    timeout: u64,

    /// TODO Skip TLS certificate verification
    #[arg(short = 'k', long, default_value_t = false)]
    no_tls_validation: bool,

    /// Should retry on request timeout
    #[arg(long, default_value_t = false)]
    retry: bool,

    /// Times to retry on request timeout
    #[arg(long, default_value_t = 3)]
    retry_attempts: u32,

    /// The target URL
    #[arg(short, long, required = true, value_parser = url_parser)]
    url: Uri,

    /// TODO Cookies to use for the requests
    #[arg(short, long, value_parser = cookie_parser)]
    cookies: Option<CookieJar>,

    /// Follow redirects
    #[arg(short = 'r', long, default_value_t = false)]
    follow_redirects: bool,

    /// Number of redirects to follow (not guaranteed because requests can timeout)
    #[arg(long, default_value_t = 2)]
    max_redirects: u8,

    /// Headers to use for the requests ["Key: Value,Key2: Value2"]
    #[arg(short = 'H',
        long,
        value_delimiter = ',',
        action = ArgAction::Append,
        value_parser = header_parser)]
    headers: Vec<(HeaderName, HeaderValue)>,

    /// Path to wordlist file
    #[arg(short, long, default_value = "stdin")]
    wordlist: PathBuf,

    /// Resume from a given position in the wordlist
    #[arg(long, default_value_t = 0)]
    wordlist_offset: usize,

    /// HTTP method to use
    #[arg(short, long,  default_value_t = Method::GET)]
    method: Method,

    /// Time each thread waits between requests in Ms
    #[arg(short, long, default_value_t = 0)]
    delay: u64,

    /// Number of concurrent threads
    #[arg(short, long, default_value_t = 10)]
    threads: u8,

    /// TODO Path to output file
    #[arg(short, long, default_value = "stdout")]
    output: PathBuf,
}

// TODO global:
// //\\--client-cert-pem <STR> Public key in PEM format for optional TLS client certificates.
// //\\--client-cert-pem-key <STR> Private key in PEM format for optional TLS client certificates (key must have no password).
// //\\--client-cert-p12 <STR> A p12 file to use for optional TLS client certificates.
// //\\--client-cert-p12-password <STR> The password for the p12 file.
// //\\--tls-renegotiation Enable TLS renegotiation. (bool flag)
// //\\--interface <STR> Specify network interface to use. Can't be used with --local-ip.
// //\\--local-ip <STR> Specify local ip of network interface to use. Can't be used with --interface.
// //\\-U, --username <STR> Username for Basic Auth.
// // \\-P, --password <STR> Password for Basic Auth.
// //\\--no-canonicalize-headers Do not canonicalize HTTP header names. If set header names are sent as is. (bool flag, default: false)
// //\\-q, --quiet Don't print the banner and other noise. (bool flag, default: false)
// //\\--no-progress Don't display progress. (bool flag, default: false)
// //\\--no-error Don't display errors. (bool flag, default: false)
// //\\-p, --pattern <STR> File containing replacement patterns.
// //\\--discover-pattern <STR> File containing replacement patterns applied to successful guesses.
// //\\--no-color Disable color output. (bool flag, default: false)
// //\\--debug Enable debug output. (bool flag, default: false)
//
//

pub fn header_parser(
    header_pair: &str,
) -> Result<(HeaderName, HeaderValue), Box<dyn std::error::Error + Send + Sync>> {
    let (key, value) = header_pair
        .split_once(':')
        .ok_or_else(|| format!("Invalid header format: {}", header_pair))?;

    let name = HeaderName::from_bytes(key.trim().as_bytes())?;
    let val = HeaderValue::from_str(value.trim())?;

    if name == USER_AGENT {
        return Err(
            "User-Agent cannot be set in headers. Use the --user-agent flag instead.".into(),
        );
    }

    Ok((name, val))
}

fn proxy_parser(s: &str) -> Result<Uri, String> {
    // Try to parse the string into a Uri
    match s.parse::<Uri>() {
        Ok(uri) => {
            // Optionally validate the scheme
            if !["http", "https", "socks5"].contains(&uri.scheme_str().unwrap()) {
                return Err(format!(
                    "Invalid scheme in URL: {}. Only 'http', 'https', and 'socks5' are allowed.",
                    uri.scheme_str().unwrap_or("")
                ));
            }
            // Return the `Uri` object directly
            Ok(uri)
        }
        Err(e) => Err(format!("Invalid URL: {}", e)),
    }
}

fn url_parser(s: &str) -> Result<Uri, String> {
    // Try to parse the string into a Uri
    match s.parse::<Uri>() {
        Ok(uri) => {
            // Optionally validate the scheme
            if !["http", "https"].contains(&uri.scheme_str().unwrap()) {
                return Err(format!(
                    "Invalid scheme in URL: {}. Only 'http' and 'https' are allowed.",
                    uri.scheme_str().unwrap_or("")
                ));
            }
            // Return the `Uri` object directly
            Ok(uri)
        }
        Err(e) => Err(format!("Invalid URL: {}", e)),
    }
}

fn cookie_parser(cookie_str: &str) -> Result<CookieJar, String> {
    let mut jar = CookieJar::new();

    // Split the cookie string by semicolons, which separate multiple cookies
    for cookie in cookie_str.split(';') {
        let trimmed_cookie = cookie.trim();

        // Parse the cookie into a `Cookie` object
        match Cookie::parse(trimmed_cookie.to_string()) {
            // Convert to String here
            Ok(parsed_cookie) => {
                jar.add(parsed_cookie);
            }
            Err(e) => {
                return Err(format!(
                    "Failed to parse cookie: '{}'. Error: {}",
                    trimmed_cookie, e
                ));
            }
        }
    }

    Ok(jar)
}
