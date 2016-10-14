use hyper::Client;
use hyper::header::Connection;
use rustc_serialize::json;
use std::collections::HashMap;
use std::fmt;
use std::io::Read;

#[derive(RustcDecodable)]
struct Response {
    pub bugs: Vec<BugData>,
}

#[derive(RustcDecodable)]
pub struct BugData {
    pub id: u32,
    pub resolution: String,
    pub status: String,
    pub summary: String,
}

impl fmt::Display for BugData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}::{} {}", self.status, self.resolution, self.summary)
    }
}

pub fn get_bugs(bugs: &[u32]) -> HashMap<u32, BugData> {
    let mut api_endpoint = "https://bugzilla.mozilla.org/rest/bug".to_string();
    api_endpoint.push_str("?id=");
    for bug in bugs {
        api_endpoint.push_str(&format!("{},", bug));
    }
    api_endpoint.push_str("?include_fields=summary,status,resolution");

    let client = Client::new();
    let maybe_res = client.get(&api_endpoint)
        .header(Connection::close())
        .send();
    let mut ret = HashMap::new();
    if let Ok(mut res) = maybe_res {
        let mut body = String::new();
        if let Ok(_) = res.read_to_string(&mut body) {
            if let Ok(response) = json::decode::<Response>(&body) {
                for data in response.bugs {
                    ret.insert(data.id, data);
                }
            }
        }
    }
    ret
}
