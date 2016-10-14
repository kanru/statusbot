extern crate hyper;
extern crate irc;
extern crate regex;
extern crate rusqlite;
extern crate rustc_serialize;
extern crate time;

mod bzapi;

use std::collections::HashMap;
use std::default::Default;
use std::thread::spawn;
use irc::client::prelude::*;
use irc::client::data::Command::{PRIVMSG};
use regex::Regex;
use rusqlite::Connection;
use time::Timespec;

fn titlecase(input: &str) -> String {
    input.chars()
        .enumerate()
        .map(|(i, c)| {
            if i == 0 {
                c.to_uppercase().next().unwrap()
            } else {
                c
            }
        })
        .collect()
}

fn textify(maybe_html: &str) -> String {
    let bug_re = Regex::new("<a href=\"http://bugzilla[^\"]+\">[Bb]ug\\s+(?P<number>\\d+)</a>")
        .unwrap();
    let text = bug_re.replace_all(maybe_html, "$number");

    let bug_re = Regex::new("(?P<number>\\d{5,})").unwrap();
    bug_re.replace_all(&text, "bug $number")
}

fn extract_bug_numbers(input: &str) -> Vec<u32> {
    let bug_re = Regex::new("[Bb]ug\\s+(?P<number>\\d+)").unwrap();
    bug_re.captures_iter(input)
        .map(|caps| caps.name("number").unwrap().parse().unwrap())
        .collect()
}

fn summarize_reports(statuses: Vec<Status>) -> String {
    let mut text = String::new();
    let mut reports = HashMap::new();
    let mut bug_numbers = Vec::new();

    for status in &statuses {
        let vec = reports.entry(&status.name).or_insert_with(Vec::new);
        vec.push(titlecase(&textify(&status.report)));
        bug_numbers.extend(extract_bug_numbers(&status.report));
    }
    let bug_details = bzapi::get_bugs(&bug_numbers);

    for (username, status) in &mut reports {
        status.sort();
        status.dedup();

        text.push_str(&format!("\n== {} ==\n", username));
        let mut bugs_map = HashMap::new();
        let mut no_bugs_reports = Vec::new();
        for content in status {
            let bugs = extract_bug_numbers(content);
            if bugs.is_empty() {
                no_bugs_reports.push(content.clone());
            } else {
                for bug in bugs {
                    let vec = bugs_map.entry(bug).or_insert_with(Vec::new);
                    vec.push(content.clone());
                }
            }
        }
        for report in no_bugs_reports {
            text.push_str(&format!("* {}\n", report));
        }
        for (bug, vec) in &bugs_map {
            match bug_details.get(bug) {
                Some(bug_data) =>
                    text.push_str(&format!("* {{{{bug|{}}}}} {}\n", bug, bug_data)),
                None =>
                    text.push_str(&format!("* {{{{bug|{}}}}} {}\n", bug, "Invalid bug or security bug")),
            };
            for content in vec {
                text.push_str(&format!("** {}\n", content));
            }
        }
    }
    text
}

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
                            println!("{}", summarize_reports(db.reports(&dates["start"], &dates["end"])));
                            server.send_privmsg(target, "done").unwrap();
                        }
                    }
                }
            },
            _ => ()
        }
    }
}
