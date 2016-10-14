extern crate irc;
extern crate regex;
extern crate rusqlite;
extern crate time;

use std::default::Default;
use std::thread::spawn;
use irc::client::prelude::*;
use irc::client::data::Command::{PRIVMSG};
use regex::Regex;
use rusqlite::Connection;
use time::Timespec;

#[derive(Debug)]
struct Status {
    id: i32,
    name: String,
    time_created: Timespec,
    report: String
}

struct StatusDb(Connection);

impl StatusDb {
    fn new() -> StatusDb {
        let db = Connection::open("db.sqlite").unwrap();
        db.execute("CREATE TABLE IF NOT EXISTS status (
                    id           INTEGER PRIMARY KEY,
                    name         TEXT NOT NULL,
                    time_created TEXT NOT NULL,
                    report       TEXT NOT NULL
                    )", &[]).unwrap();
        StatusDb(db)
    }
    fn add(&self, name: &str, report: &str) {
        let db = &self.0;
        db.execute("INSERT INTO status (name, time_created, report)
                    VALUES ($1, $2, $3)",
                   &[&name, &time::get_time(), &report]).unwrap();
    }
    fn reports(&self, start: &str, end: &str) -> Vec<Status> {
        let db = &self.0;
        let mut stmt = db.prepare("SELECT id, name, time_created, report FROM status
                                   WHERE date(time_created) >= date($1)
                                   AND date(time_created) <= date($2)").unwrap();
        let rows = stmt.query_map(&[&start, &end], |row| {
            Status {
                id: row.get(0),
                name: row.get(1),
                time_created: row.get(2),
                report: row.get(3)
            }
        }).unwrap();
        let mut reports = Vec::new();
        for report in rows {
            reports.push(report.unwrap());
        }
        reports
    }
}

static STANDUP_NICK : &'static str =  "standups";
static BOT_NICK: &'static str = "abot";

fn main() {
    let db = StatusDb::new();
    let config = Config {
        nickname: Some(String::from(BOT_NICK)),
        server: Some(String::from("irc.mozilla.org")),
        channels: Some(vec![String::from("#statusbot")]),
        .. Default::default()
    };
    let server = IrcServer::from_config(config).unwrap();
    server.identify().unwrap();

    let re = Regex::new(r"^(?P<nick>[^:]+):\s*(?P<msg>.*)$").unwrap();
    let report_re = Regex::new(r"(?P<start>\d{4}-\d{2}-\d{2})\s+to\s+(?P<end>\d{4}-\d{2}-\d{2})").unwrap();
    for msg in server.iter() {
        let msg = msg.unwrap();
        print!("{}", msg);
        match msg.command {
            PRIVMSG(ref target, ref message) => {
                if let Some(caps) = re.captures(message) {
                    if &caps["nick"] == STANDUP_NICK {
                        if let Some(nickname) = msg.source_nickname() {
                            db.add(nickname, message);
                        }
                    }
                    if &caps["nick"] == BOT_NICK {
                        if let Some(dates) = report_re.captures(&caps["msg"]) {
                            server.send_privmsg(target,
                                                &format!("{:?}", db.reports(&dates["start"], &dates["end"]))).unwrap();
                        }
                    }
                }
            },
            _ => ()
        }
    }
}
