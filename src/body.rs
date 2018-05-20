use std::cmp::min;
use std::io::{self, BufReader, Read};

use http::{header, StatusCode};
use http::response::Parts;

use http1::read_chunked_body;
use util::{get_content_length, is_chunked};
use Error;

#[derive(Debug, PartialEq)]
enum BodyKind {
    None,
    Fixed(usize),
    Chunked(usize, bool),
    CloseDelimited,
}

/// Contains a raw HTTP response body.
///
/// The body is readable and can be transformed to another more specific
/// representation like a string or a custom type with the `FromBody` trait.
pub struct Body<R> {
    kind: BodyKind,
    reader: BufReader<R>,
}

impl<R> Body<R> {
    /// Returns true if the message has no body.
    ///
    /// Responses to HEAD requests and responses with a 1xx, 204 and 304
    /// do not have a body. All other responses have a body but
    /// it may be empty.
    pub fn is_none(&self) -> bool {
        self.kind == BodyKind::None
    }

    pub(crate) fn from_response(
        reader: BufReader<R>,
        response: &Parts,
        head: bool,
    ) -> io::Result<Body<R>> {
        // See http://httpwg.org/specs/rfc7230.html#rfc.section.3.3.3 for steps
        // 1. no-body messages
        let status = response.status;
        if head || status.is_informational() || status == StatusCode::NO_CONTENT
            || status == StatusCode::NOT_MODIFIED
        {
            return Ok(Body {
                kind: BodyKind::None,
                reader,
            });
        }
        // 2. CONNECT messages (not implemented)
        // 3. Chunked message
        if response.headers.contains_key(&header::TRANSFER_ENCODING) {
            if is_chunked(response.headers.get_all(&header::TRANSFER_ENCODING)) {
                return Ok(Body {
                    kind: BodyKind::Chunked(0, false),
                    reader,
                });
            } else {
                return Ok(Body {
                    kind: BodyKind::CloseDelimited,
                    reader,
                });
            }
        }
        // 4. + 5. Fixed body messages
        if response.headers.contains_key(&header::CONTENT_LENGTH) {
            if let Some(len) = get_content_length(response.headers.get_all(&header::CONTENT_LENGTH))
            {
                return Ok(Body {
                    kind: BodyKind::Fixed(len),
                    reader,
                });
            } else {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    Error::BadResponse,
                ));
            }
        }
        // (6. request only)
        // 7. read until connection is closed
        return Ok(Body {
            kind: BodyKind::CloseDelimited,
            reader,
        });
    }
}

impl<R: Read> Read for Body<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        match self.kind {
            BodyKind::None => Ok(0),
            BodyKind::Fixed(0) => Ok(0),
            BodyKind::Fixed(ref mut len) => {
                let buf_len = buf.len();
                let read_len = self.reader.read(&mut buf[..min(buf_len, *len)])?;
                *len -= read_len;
                Ok(read_len)
            }
            BodyKind::Chunked(ref mut chunk_len, ref mut last) => {
                read_chunked_body(&mut self.reader, buf, chunk_len, last)
            }
            BodyKind::CloseDelimited => self.reader.read(buf),
        }
    }
}

/// Read response bodies as strong types.
///
/// Used to convert untyped message bodies to a more specific representation.
///
/// Read the response header values to determine
/// text encoding, compression applied and content type transmitted.
pub trait FromBody: Send + Sync + Sized + 'static {
    fn from_body<R: Read>(response: &Parts, body: &mut Body<R>) -> io::Result<Self>;
}

impl FromBody for () {
    fn from_body<R: Read>(_response: &Parts, body: &mut Body<R>) -> io::Result<Self> {
        if body.is_none() {
            Ok(())
        } else {
            Err(io::Error::new(
                io::ErrorKind::InvalidData,
                Error::BadResponse,
            ))
        }
    }
}

impl FromBody for Vec<u8> {
    fn from_body<R: Read>(_response: &Parts, body: &mut Body<R>) -> io::Result<Self> {
        // TODO: Handle compression encoding.
        let mut data = Vec::new();
        body.read_to_end(&mut data)?;
        Ok(data)
    }
}

impl FromBody for String {
    fn from_body<R: Read>(_response: &Parts, body: &mut Body<R>) -> io::Result<Self> {
        // TODO: Handle text encodings other than UTF-8
        let mut data = String::new();
        body.read_to_string(&mut data)?;
        Ok(data)
    }
}
