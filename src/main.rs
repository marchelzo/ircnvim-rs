#![feature(slice_patterns)]
#![feature(const_fn)]

extern crate regex;
extern crate time;
extern crate rustc_serialize;

use std::env;
use ircnvim::session::Session;
use ircnvim::config::Config;

mod ircnvim {
    macro_rules! log {
        ($($a:expr),*) => (io::stderr().write((format!($($a),*) + "\n").as_bytes()).unwrap())
    }

    pub mod session;
    pub mod config;
    pub mod channel;
    pub mod text;
    pub mod msg;
    pub mod room;
    pub mod irc;
    pub mod user;

    use regex::Regex;
    use std::ptr;

    static mut _my_nick_regex: *const Regex = ptr::null();
}

fn main() {
    /*
     * Get HOME from env so we can get the path to the ircnvim directory.
     */
    let path = match env::var("HOME") {
        Ok(home) => format!("{}/.ircnvim", home),
        Err(e)   => {
            println!("ERROR Error: couldn't get HOME from the environment: {}", e.to_string());
            return;
        }
    };
    let profile = env::args().nth(1);
    let config = Config::load(path, profile);
    match config {
        Err(e)     => println!("ERROR Error loading configuration file: {}", e),
        Ok(config) => {
            match Session::new(config) {
                Ok(session) => session.run(),
                Err(e)      => println!("ERROR Error connecting to IRC server: {}", e)
            }
        }
    }
}
