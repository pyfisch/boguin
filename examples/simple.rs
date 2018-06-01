extern crate boguin;
extern crate http;

fn main() {
    let mut client = boguin::Client::new();
    let url = http::Url::parse("https://httpbin.org/status/418").unwrap();
    let request = http::Request::new(url, ());
    let response: http::Response<String> = client.fetch(request).expect("request works");
    println!("{}", response.status());
    println!("{}", response.body());
}