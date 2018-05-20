extern crate boguin;
extern crate clap;
extern crate env_logger;
extern crate http;
#[macro_use]
extern crate log;

use std::error::Error;
use std::io::{self, Read, Write};

use boguin::Client;
use clap::{App, Arg, ArgMatches};
use http::{Request, Response, Url};

fn build_request(matches: ArgMatches) -> Result<Request<Vec<u8>>, String> {
    let url = Url::parse(matches.value_of("url").expect("url is present"))
        .map_err(|_| "Invalid URL".to_owned())?;
    let mut request = Request::builder(url);

    request.method(matches.value_of("method").expect("method is present"));

    if matches.is_present("header") {
        for header in matches.values_of("header").expect("header is present") {
            let mut parts = header.splitn(2, ':');
            let key = parts
                .next()
                .ok_or_else(|| format!("header {:?} is invalid", header))?
                .trim();
            let value = parts
                .next()
                .ok_or_else(|| format!("header {:?} is invalid", header))?
                .trim();
            assert!(parts.next().is_none());
            request.header(key, value);
        }
    }

    if matches.is_present("stdin") {
        let mut data = Vec::new();
        io::stdin()
            .read_to_end(&mut data)
            .map_err(|e| e.to_string())?;
        return request.body(data).map_err(|e| e.description().to_owned());
    }

    return request.body(Vec::new()).map_err(|e| e.to_string());
}

fn print_response(response: &Response<Vec<u8>>) {
    // Note: Writes to stderr always succeed. 
    let mut stderr = io::stderr();
    writeln!(stderr, "{:?} {}", response.version(), response.status()).unwrap();
    for (key, value) in response.headers() {
        let value = value.to_str().unwrap_or("<binary value>");
        writeln!(stderr, "{}: {}", key.as_str(), value).unwrap();
    }
    writeln!(stderr, "").unwrap();
    let mut stdout = io::stdout();
    stdout.write_all(response.body()).unwrap();
}

fn main() {
    env_logger::init();
    let matches = App::new("boguin")
        .version("0.1.0")
        .about("A small HTTP command line client.")
        .author("Pyfisch")
        .arg(
            Arg::with_name("method")
                .value_name("METHOD")
                .required(true)
                .help("Sets the request method"),
        )
        .arg(
            Arg::with_name("url")
                .value_name("URL")
                .required(true)
                .help("Sets the request target"),
        )
        .arg(
            Arg::with_name("header")
                .short("H")
                .long("header")
                .takes_value(true)
                .multiple(true)
                .help("Send a custom request header"),
        )
        .arg(
            Arg::with_name("stdin")
                .long("stdin")
                .help("Read a HTTP body from standard input"),
        )
        .get_matches();
    let request = match build_request(matches) {
        Ok(request) => request,
        Err(e) => {
            error!("{}", e);
            return;
        }
    };
    let mut client = Client::new();
    let response: Response<Vec<u8>> = match client.fetch(request) {
        Ok(response) => response,
        Err(e) => {
            error!("{}", e);
            return;
        }
    };
    print_response(&response);
}
