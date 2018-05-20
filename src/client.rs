use std::collections::HashMap;
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::net::TcpStream;

use http::{header, Method, Request, Response};
use http::url::Origin;
use native_tls::{HandshakeError, TlsConnector, TlsStream};

use body::{Body, FromBody};
use http1;
use util::{is_persistent_connection, is_redirect_method_get, is_redirect_status, wrap_error};

/// A HTTP(S) client.
///
/// Use `Client::new().fetch(request)` to make a single request.
pub struct Client {
    tcp_streams: HashMap<Origin, TcpStream>,
    tls_streams: HashMap<Origin, TlsStream<TcpStream>>,
    tls_connector: Option<TlsConnector>,
}

impl Client {
    /// Creates a new client.
    ///
    /// Try to use a client for multiple connections as the client may
    /// be able to reuse existing connections.
    pub fn new() -> Client {
        Client {
            tcp_streams: HashMap::new(),
            tls_streams: HashMap::new(),
            tls_connector: None,
        }
    }

    fn get_tls_connector(&mut self) -> io::Result<&TlsConnector> {
        if let Some(ref connector) = self.tls_connector {
            return Ok(connector);
        } else {
            self.tls_connector = Some(wrap_error(wrap_error(TlsConnector::builder())?.build())?);
            return self.get_tls_connector();
        }
    }

    /// Send a HTTP request.
    ///
    /// This is the main function of the crate.
    /// It will send the request using either HTTP or HTTPS to the server.
    /// If possible it will reuse connections from the same client.
    /// The client follows up to 20 redirects.
    /// The body is automatically converted to the expected format.
    pub fn fetch<A, B: FromBody>(&mut self, request: Request<A>) -> io::Result<Response<B>> {
        info!("Fetching {} {}", request.method(), request.url());
        match self.fetch_redirect(request, 0) {
            Ok(response) => Ok(response),
            Err(err) => {
                warn!("Encountered error: {:?}", err);
                Err(err)
            }
        }
    }

    fn fetch_redirect<A, B: FromBody>(
        &mut self,
        mut request: Request<A>,
        counter: u8,
    ) -> io::Result<Response<B>> {
        if counter >= 20 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                Error::TooManyRedirects,
            ));
        }
        let response = self.fetch_network(&mut request)?;
        if is_redirect_status(response.status()) {
            if let Some(location) = response.headers().get(header::LOCATION) {
                let location_url = wrap_error(request.url().join(wrap_error(location.to_str())?))?;
                *request.url_mut() = location_url;
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    Error::BadResponse,
                ));
            }
            info!(
                "Following '{}' redirect to {}",
                response.status(),
                request.url()
            );
            if is_redirect_method_get(response.status(), request.method()) {
                info!(
                    "Method changed in redirect from {} to GET",
                    request.method()
                );
                let head = request.into_parts().0;
                let mut request = Request::from_parts(head, ());
                *request.method_mut() = Method::GET;
                return self.fetch_redirect(request, counter + 1);
            } else {
                return self.fetch_redirect(request, counter + 1);
            };
        } else {
            Ok(response)
        }
    }

    fn fetch_network<A, B: FromBody>(
        &mut self,
        request: &mut Request<A>,
    ) -> io::Result<Response<B>> {
        if request.url().scheme() == "http" {
            self.fetch_network_http(request)
        } else if request.url().scheme() == "https" {
            self.fetch_network_https(request)
        } else {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                Error::WrongScheme,
            ));
        }
    }

    fn fetch_network_http<A, B: FromBody>(
        &mut self,
        request: &mut Request<A>,
    ) -> io::Result<Response<B>> {
        let origin = request.url().origin();
        let mut stream = if let Some(stream) = self.tcp_streams.remove(&origin) {
            debug!("Reusing connection to {:?}", origin);
            stream
        } else {
            TcpStream::connect(request.url())?
        };
        let response = Client::fetch_data(request, &mut stream)?;
        if is_persistent_connection(
            response.version(),
            response.headers().get_all(header::CONNECTION),
        ) {
            debug!("Keeping connection to {:?} for later use", origin);
            self.tcp_streams.insert(origin, stream);
        } else {
            debug!("Closed connection to {:?}", origin);
        }
        Ok(response)
    }

    fn fetch_network_https<A, B: FromBody>(
        &mut self,
        request: &mut Request<A>,
    ) -> io::Result<Response<B>> {
        let origin = request.url().origin();
        let mut tls_stream = if let Some(stream) = self.tls_streams.remove(&origin) {
            debug!("Reusing connection to {:?}", origin);
            stream
        } else {
            let domain = if let Some(domain) = request.url().domain() {
                domain
            } else {
                return Err(io::Error::new(io::ErrorKind::InvalidInput, Error::NoDomain));
            };
            let stream = TcpStream::connect(request.url())?;
            let connector = self.get_tls_connector()?;
            match connector.connect(domain, stream) {
                Ok(stream) => stream,
                Err(HandshakeError::Failure(err)) => return wrap_error(Err(err)),
                Err(HandshakeError::Interrupted(_)) => {
                    panic!("TcpStream should never raise WouldBlock.")
                }
            }
        };
        let response = Client::fetch_data(request, &mut tls_stream)?;
        if is_persistent_connection(
            response.version(),
            response.headers().get_all(header::CONNECTION),
        ) {
            debug!("Keeping secure connection to {:?} for later use", origin);
            self.tls_streams.insert(origin, tls_stream);
        } else {
            debug!("Closed secure connection to {:?}", origin);
        }
        Ok(response)
    }

    fn fetch_data<A, B: FromBody, S: Read + Write>(
        request: &mut Request<A>,
        mut stream: S,
    ) -> io::Result<Response<B>> {
        {
            let mut buf_writer = BufWriter::new(&mut stream);
            http1::write_request_header(&mut buf_writer, &request)?;
            buf_writer.flush()?;
        }
        let mut buf_reader = BufReader::new(&mut stream);
        let parts = http1::read_response_header(&mut buf_reader)?;
        let mut body = Body::from_response(buf_reader, &parts, request.method() == &Method::HEAD)?;
        let typed_body = FromBody::from_body(&parts, &mut body)?;
        Ok(Response::from_parts(parts, typed_body))
    }
}

impl Default for Client {
    fn default() -> Client {
        Client::new()
    }
}

/// HTTP specific errors.
///
/// These are used together with an `ErrorKind` in an `io::Error`.
/// The `ErrorKind` is usually either `InvalidInput` when the
/// user of the crate provided invalid info for a request or
/// `InvalidData` when the server returned wrong or broken info.
///
/// Many other Errors are just wrapped inside an `io::Error`.
/// They originate from a dependency and are not part of
/// the public API.
/// (They are here for information and logging
/// and should not be dependent on in code.)
#[derive(Debug)]
pub enum Error {
    /// An invalid scheme was encountered in a request URL.
    ///
    /// Currently allowed are only HTTP and HTTPS.
    WrongScheme,
    /// URL contains no domain.
    ///
    /// TLS requires a domain name to verify the certificate.
    /// If the URL contains an IP address it has no domain.
    NoDomain,
    /// The client tried to follow too many redirects and gave up.
    ///
    /// Current limit is 20 redirects maximum.
    TooManyRedirects,
    /// There was some logic error in the response received.
    ///
    /// Such problems may not always raise this error but
    /// instead provide more specific information from the original error.
    BadResponse,
    #[doc(hidden)]
    __Nonexhaustive,
}

impl ::std::error::Error for Error {
    fn description(&self) -> &str {
        match *self {
            Error::WrongScheme => "request URL has an unsupported scheme",
            Error::NoDomain => "URL contains no domain for TLS connection",
            Error::TooManyRedirects => "encountered too many redirects",
            Error::BadResponse => "bad response received",
            _ => panic!(),
        }
    }
}

impl ::std::fmt::Display for Error {
    fn fmt(&self, f: &mut ::std::fmt::Formatter) -> ::std::fmt::Result {
        f.write_str(::std::error::Error::description(self))
    }
}
