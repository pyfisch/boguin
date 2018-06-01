# Boguin - Simple HTTP client

The client supports HTTP/1.1, TLS and redirects.
It is a demo for the *[http-with-url](https://github.com/pyfisch/http-with-url)* crate.


```rust
extern crate boguin;
extern crate http_with_url as http;

fn main() {
    let mut client = boguin::Client::new();
    let url = http::Url::parse("https://httpbin.org/status/418").unwrap();
    let request = http::Request::new(url, ());
    let response: http::Response<String> = client.fetch(request).expect("request works");
    println!("{}", response.status());
    println!("{}", response.body());
}
```

You can also use the command line client with `cargo run --example boguin`.
