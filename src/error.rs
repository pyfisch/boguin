#[derive(Debug)]
pub struct Error {
    inner: ErrorKind,
}

#[derive(Debug)]
enum ErrorKind {
    Io(::std::io::Error),
    Http(::http::Error),
    Httparse(::httparse::Error),
    Tls(::native_tls::Error),
}

impl From<::std::io::Error> for Error {
    fn from(err: ::std::io::Error) -> Error {
        Error {
            inner: ErrorKind::Io(err),
        }
    }
}

impl From<::http::Error> for Error {
    fn from(err: ::http::Error) -> Error {
        Error {
            inner: ErrorKind::Http(err),
        }
    }
}

impl From<::httparse::Error> for Error {
    fn from(err: ::httparse::Error) -> Error {
        Error {
            inner: ErrorKind::Httparse(err),
        }
    }
}

impl From<::native_tls::Error> for Error {
    fn from(err: ::native_tls::Error) -> Error {
        Error {
            inner: ErrorKind::Tls(err),
        }
    }
}
