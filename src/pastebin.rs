use hyper::Client;
use hyper::header::{Connection, ContentType, UserAgent};
use mime::Mime;
use url::form_urlencoded;

pub fn paste(text: String) -> Result<String, &'static str> {
    let api_endpoint = String::from("https://pastebin.mozilla.org/");
    let body: String = form_urlencoded::Serializer::new(String::new())
        .append_pair("parent_pid", "")
        .append_pair("format", "text")
        .append_pair("code2", &text)
        .append_pair("expiry", "d")
        .append_pair("paste", "Send")
        .append_pair("poster", "")
        .finish();
    let content_type: Mime = "application/x-www-form-urlencoded".parse().unwrap();
    let client = Client::new();
    let maybe_res = client.post(&api_endpoint)
        .header(Connection::close())
        .header(UserAgent(String::from("Statusbot 0.1.0")))
        .header(ContentType(content_type))
        .body(&body)
        .send();
    if let Ok(res) = maybe_res {
        Ok(res.url.clone().into_string())
    } else {
        Err("network error")
    }
}
