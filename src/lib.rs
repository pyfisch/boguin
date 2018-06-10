extern crate http_with_url as http;
extern crate httparse;
#[macro_use]
extern crate log;

#[cfg(not(feature="rust_tls"))]
extern crate native_tls;

#[cfg(feature="rust_tls")]
extern crate rustls;
#[cfg(feature="rust_tls")]
extern crate webpki;
#[cfg(feature="rust_tls")]
extern crate webpki_roots;

pub use body::{Body, FromBody};
pub use client::{Client, Error};

mod body;
mod client;
mod http1;
mod util;
