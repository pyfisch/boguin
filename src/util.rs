use std::io;

use http::{Method, StatusCode, Version};
use http::header::{GetAll, HeaderValue};

pub(crate) fn is_redirect_status(status: StatusCode) -> bool {
    // https://fetch.spec.whatwg.org/#redirect-status
    status == StatusCode::MOVED_PERMANENTLY || status == StatusCode::FOUND
        || status == StatusCode::SEE_OTHER || status == StatusCode::TEMPORARY_REDIRECT
        || status == StatusCode::PERMANENT_REDIRECT
}

pub(crate) fn is_redirect_method_get(status: StatusCode, method: &Method) -> bool {
    // > If either actualResponse’s status is 301 or 302 and request’s method
    // > is `POST`, or actualResponse’s status is 303, set request’s method
    // > to `GET` and request’s body to null.
    (status == StatusCode::MOVED_PERMANENTLY || status == StatusCode::FOUND)
        && method == &Method::POST || (status == StatusCode::SEE_OTHER)
}

pub(crate) fn is_chunked(values: GetAll<HeaderValue>) -> bool {
    if let Some(last) = values.iter().last() {
        if let Ok(s) = last.to_str() {
            return s.ends_with("chunked");
        }
    }
    false
}

pub(crate) fn is_persistent_connection(
    version: Version,
    connection_header: GetAll<HeaderValue>,
) -> bool {
    // https://httpwg.org/specs/rfc7230.html#persistent.connections
    if version != Version::HTTP_11 {
        return false;
    }
    for header in connection_header {
        if let Ok(s) = header.to_str() {
            if s.contains("close") {
                return false;
            }
        } else {
            return false;
        }
    }
    true
}

pub(crate) fn get_content_length(values: GetAll<HeaderValue>) -> Option<usize> {
    // > If a message is received [...] with either multiple Content-Length
    // > header fields having differing field-values or a single
    // > Content-Length header field having an invalid value, then the message
    // > framing is invalid and the recipient MUST treat it as an
    // > unrecoverable error.
    let mut result = None;
    for value in values {
        if let Some(len) = value.to_str().ok().and_then(|v| v.parse().ok()) {
            if result.is_some() && result != Some(len) {
                return None;
            }
            result = Some(len)
        }
    }
    result
}

pub(crate) fn wrap_error<T, E>(result: Result<T, E>) -> io::Result<T>
where
    E: 'static + ::std::error::Error + ::std::marker::Send + ::std::marker::Sync,
{
    match result {
        Ok(v) => Ok(v),
        Err(e) => Err(io::Error::new(io::ErrorKind::Other, e)),
    }
}
