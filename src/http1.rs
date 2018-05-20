use std::io::{self, BufRead, Write};

use http::{header, request, response, Request, Version};
use httparse;

use util::wrap_error;
use Error;

pub fn write_request_header<T, W: Write>(writer: &mut W, req: &Request<T>) -> io::Result<()> {
    let (_, authority, path) = wrap_error(request::get_target_components(req))?;
    let version_str = match req.version() {
        Version::HTTP_10 => "HTTP/1.0",
        Version::HTTP_11 => "HTTP/1.1",
        v => panic!("Unsupported version: {:?}", v),
    };
    write!(writer, "{} {} {}", req.method(), path, version_str)?;
    write!(writer, "\r\n{}: {}", header::HOST.as_str(), authority)?;
    for (name, value) in req.headers().iter() {
        write!(writer, "\r\n{}: ", name.as_str())?;
        writer.write(value.as_bytes())?;
    }
    write!(writer, "\r\n\r\n")?;
    Ok(())
}

pub fn read_response_header<R: BufRead>(reader: &mut R) -> io::Result<response::Parts> {
    loop {
        let len;
        let mut builder = response::Builder::new();
        {
            let mut headers = [httparse::EMPTY_HEADER; 64];
            let mut resp = httparse::Response::new(&mut headers);
            let buf = reader.fill_buf()?;
            let parse_state = wrap_error(resp.parse(buf))?;
            if parse_state.is_partial() {
                continue;
            }
            // Note: Unwrap is safe because the response is complete.
            len = parse_state.unwrap();

            builder.status(resp.code.unwrap());
            builder.version(match resp.version.unwrap() {
                0 => Version::HTTP_10,
                1 => Version::HTTP_11,
                _ => unreachable!(),
            });
            for header in resp.headers {
                builder.header(
                    header.name,
                    wrap_error(header::HeaderValue::from_bytes(header.value))?,
                );
            }
        }
        reader.consume(len);
        return Ok(wrap_error(builder.body(()))?.into_parts().0);
    }
}

pub fn read_chunked_body<R: BufRead>(
    reader: &mut R,
    buf: &mut [u8],
    chunk_len: &mut usize,
    last: &mut bool,
) -> io::Result<usize> {
    use httparse::{parse_chunk_size, Status};
    loop {
        match (*chunk_len, *last) {
            (0, false) => {
                let consumed_len = {
                    let buf = reader.fill_buf()?;
                    match parse_chunk_size(buf) {
                        Ok(Status::Complete((consumed, len))) => {
                            *chunk_len = len as usize + 2;
                            *last = len == 0;
                            consumed
                        }
                        Ok(Status::Partial) => return Err(io::ErrorKind::Interrupted.into()),
                        _ => {
                            return Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                Error::BadResponse,
                            ))
                        }
                    }
                };
                reader.consume(consumed_len);
            }
            (0, true) => return Ok(0),
            (2, _) => {
                {
                    let buf = reader.fill_buf()?;
                    if buf.len() < 2 {
                        return Err(io::ErrorKind::Interrupted.into());
                    }
                }
                if buf[..1] != [b'\r', b'\n'] {
                    reader.consume(2);
                } else {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        Error::BadResponse,
                    ));
                }
                *chunk_len = 0;
            }
            (n, _) => {
                let buf_len = buf.len();
                let read_len = reader.read(&mut buf[..(n - 2).min(buf_len)])?;
                *chunk_len = n - read_len;
                return Ok(read_len);
            }
        }
    }
}
