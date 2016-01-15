use ircnvim::channel::IsChannelName;
use ircnvim::config::AuthMethod;
use ircnvim::config::Config;
use ircnvim::irc::IrcMessage;
use ircnvim::msg::Message;
use ircnvim::room::Room;
use ircnvim::text::Text;
use ircnvim::user::User;
use ircnvim::_my_nick_regex;
use regex::Regex;
use regex;
use rustc_serialize::base64::ToBase64;
use rustc_serialize::base64;
use std::ascii::AsciiExt;
use std::fs;
use std::io::BufRead;
use std::io::Read;
use std::io::Write;
use std::io;
use std::net::TcpStream;
use std::process;
use std::ptr;
use std::str;
use std::sync::{Arc, Mutex};
use std::thread;
use time;
use time::Timespec;

const BUFFER_SIZE: usize = 512;
const MIN_UPDATE_INTERVAL_MS: u32 = 200;

pub struct Session {
    stream: TcpStream,
    config: Config,
    rooms: Vec<Room>,
    active_room: usize,
    status_line: String,
}

unsafe impl Send for Session { }

impl Session {

    pub fn new(config: Config) -> Result<Session, String> {
        /*
         * Attempt to make a connection to the IRC server.
         */
        let stream = match TcpStream::connect(&config.server[..]) {
            Ok(stream) => stream,
            Err(e)     => return Err(e.to_string())
        };
        /*
         * Make sure all of the necessary directories exist.
         * Try to create them if they don't.
         */
        if let Err(e) = fs::create_dir_all(format!("{}/{}", config.directory, config.server)) {
            return Err(e.to_string());
        }

        /*
         * Create the global nick regex.
         */
        unsafe {
            _my_nick_regex = Box::into_raw(Box::new(Regex::new(&format!(r"([^A-Z]|\b){}([^A-Z]|\b)", regex::quote(&config.nick))).unwrap()));
        }

        let rooms = vec![Room::server(&config)];

        return Ok(Session {
            stream: stream,
            config: config,
            rooms: rooms,
            active_room: 0,
            status_line: String::new(),
        });
    }

    pub fn run(mut self) {

        /*
         * First of all, tell the client to load the server buffer.
         *
         * On the Neovim side, we don't know the location of the server
         * file yet, so this is necessary in order for the client to be
         * able to begin displaying incoming messages.
         *
         * We also need to send our nick, escaped, so that it can be used
         * to create the proper syntax rules.
         */
        println!("NICK {}", regex::quote(&self.config.nick));
        self.server().goto();

        /*
         * Move self into an Arc<Mutex<>> so that we can share it between
         * multiple threads.
         */
        let session = Arc::new(Mutex::new(self));

        /*
         * First, we spawn a thread that will send update requests to the
         * client periodically, if necessary.
         */
        let session_clone = session.clone();
        thread::spawn(move || {
            let session = session_clone;

            /*
             * The number of consecutive times we've tried to lock without success.
             */
            let mut n = 0;
            loop {
                if let Ok(mut session) = session.try_lock() {
                    n = 0;
                    if session.should_update() {
                        session.update();
                    }
                    if session.update_status_line() {
                        println!("STATUS {}", session.status_line);
                    }
                } else {
                    n += 1;
                }

                /*
                 * If we've tried to lock the mutex 3 times without success,
                 * force an update even though it may not be necessary.
                 */
                if n == 3 {
                    n = 0;
                    println!("UPDATE");
                }

                thread::sleep_ms(MIN_UPDATE_INTERVAL_MS);
            }
        });

        /*
         * Next, we identify and optionally authenticate.
         */
        match session.lock() {
            Ok(mut session) => {
                if let Some(error) = match session.config.auth {
                    AuthMethod::NoAuth   => session.identify(),
                    AuthMethod::NickServ => session.auth_nickserv(),
                    AuthMethod::SASL     => session.auth_sasl(),
                } {
                    session.die(&format!("failed to authenticate: {}", error));
                }
            },
            _           => panic!("something has gone terribly wrong")
        }


        /*
         * Spawn a thread that will read from the IRC network connection
         * and handle incoming messages.
         */
        let session_clone = session.clone();
        let mut stream = session.lock().unwrap().stream.try_clone().unwrap();
        thread::spawn(move || {
            let session = session_clone;
            let mut buf = [0u8; BUFFER_SIZE];
            let mut idx = 0;
            while let Ok(bytes) = Session::read_message(&mut stream, &mut buf, &mut idx) {
                let message = IrcMessage::parse(&bytes).expect("message parse");
                match session.lock() {
                    Ok(mut session) => session.handle_message(message),
                    _               => { log!("Error taking lock"); }
                }
            }
        });

        /*
         * Everything is ready to go; notify the client that they should
         * load the server file.
         */
        session.lock().unwrap().server().goto();

        let stdin = io::stdin();
        for line in stdin.lock().lines() {
            let line = match line {
                Ok(line) => line.trim_right().to_string(),
                Err(_)   => { continue }
            };

            let (command, rest) = match line.find(' ') {
                Some(i) => line.split_at(i),
                None    => (&line[..], "")
            };

            let rest = rest.trim_left();

            let mut session = session.lock().unwrap();
            match command {
                "INPUT"            => {
                    session.handle_input(rest)
                },
                "ROOM-PREVIOUS"    => {
                    if session.active_room > 0 {
                        session.active_room -= 1;
                        session.active_room().goto();
                    }
                },
                "ROOM-NEXT"        => {
                    if session.active_room + 1 < session.rooms.len() {
                        session.active_room += 1;
                        session.active_room().goto();
                    }
                },
                _                  => { }
            }
            
            session.update();
        }
    }

    /*
     * Handle a line of input that was received from the client.
     */
    fn handle_input(&mut self, input: &str) {

        /*
         * First check to see if the input begins with a /. If it does
         * then we should treat it as a command and not a regular message.
         */
        if input.as_bytes()[0] == b'/' {
            let end = input.find(' ').unwrap_or(input.len());
            self.handle_command(&input[1..end], &input[end..].trim_left());
            return;
        }

        /*
         * Prepare the message and send it to the server.
         */
        let message = self.active_room_mut().make_message(input);
        self.send(&message);

        /*
         * Add the message to the active room's message list
         * so that it becomes visible in the client.
         */
        let source = Text::decorate_nick(&self.config.nick);
        let message = Message::new(source, Text::from_string(input.to_string()));
        self.active_room_mut().add_message(message);
    }

    fn handle_command(&mut self, command_name: &str, arg: &str) {
        macro_rules! commands {
            ($($($c:ident)|* => $s:stmt),*) => { $(if $(command_name.eq_ignore_ascii_case(stringify!($c)))||* { $s; return })* }
        }

        log!("command_name: `{}`", command_name); 
        log!("arg: `{}`", arg); 

        commands! {
            j | join => {
                for channel in arg.split_whitespace() {
                    self.send(&format!("JOIN {}", channel));
                }
            },
            p | part => {
                let part_message = arg.trim();
                self.part(part_message);
            },
            msg  => {
                match arg.find(' ') {
                    Some(i) => {
                        let target = &arg[..i];
                        let message = &arg[i+1..];
                        self.send(&format!("PRIVMSG {} :{}", target, message)); 
                    },
                    None    => {
                        self.active_room_mut().warn("Invalid syntax in /msg command. Syntax is /msg <target> <message>.");
                    }
                }
            },
            quit => {
                let quit_message = arg.trim();
                self.quit(quit_message);
            },
            raw => {
                self.send(arg);
            },
            nick => {
                self.send(&format!("NICK {}", arg.trim()));
            }
        };

        self.active_room_mut().warn(&format!("{} is not a recognized command", command_name));
    }

    fn handle_message(&mut self, message: IrcMessage) -> () {
        use ircnvim::irc::IrcMessageType::*;
        match message.kind {
            Nick => {
                let sender = message.user();
                let new_nick = message.param(0).text();
                let me = message.source() == self.config.nick;
                if me {
                    self.config.nick = new_nick.to_string();
                    println!("NICK {}", regex::quote(new_nick));
                    self.active_room_mut().notify(&format!("You are now known as {}", new_nick));
                }
                for room in &mut self.rooms {
                    if room.is_user_present(&sender) {
                        room.rename_user(&sender, new_nick);
                        if !me { room.notify(&format!("{} is now known as {}", sender.nick, new_nick)) }
                    }
                }
            },
            ChannelURL => {
                let channel = message.param(1).text();
                let url = message.param(2).text();
                self.get_room(channel).unwrap().notify(&format!("The website for {} is {}", channel, url));
            },
            TopicWhoTime => {
                let channel = message.param(1).text();
                let nick = message.param(2).text();
                let timestamp = message.param(3).text().parse::<i64>().unwrap();
                let time_fmt = time::at(Timespec::new(timestamp, 0)).rfc822().to_string();
                self.get_room(channel).unwrap().notify(&format!("The topic was last set by {} on {}", nick, time_fmt));
            },
            Topic => {
                let channel = message.param(1).text();
                let topic = message.param(2).text();
                let room = self.get_room(channel).unwrap();
                room.set_topic(topic.to_string());
                room.notify_topic();
            }
            NotImplemented => {
                self.server().notify(&message.sequence(0));
            },
            LUserClient | LUserOp | LUserUnknown | LUserChannels | LUserMe => {
                self.server().notify(&message.sequence(0));
            },
            UnknownCommand => {
                self.active_room_mut().warn(&format!("Unknown command: {}", message.param(1).text()));
            },
            Welcome | YourHost | Created | MOTDStart | MOTD | MOTDEnd => {
                self.server().notify(message.param(1).text());
            },
            Error   => {
                self.server().notify(&message.sequence(0));
            },
            Names   => {
                let channel = message.param(2).text();
                let room = self.get_room(channel).unwrap();
                let nicks = message.param(3).text();
                for nick in nicks.split_whitespace() {
                    room.add_user(User::from_nick(nick.to_string()));
                }
            },
            Mode    => {
                let source = message.source();
                let target = message.param(0).text();
                let room = if self.get_room(target).is_some() {
                    self.get_room(target).unwrap()
                } else {
                    self.server()
                };
                let mode_string = message.sequence(1);
                let notification = if message.params().len() == 2 {
                    format!("{} sets mode {} for {}", source, mode_string, target)
                } else {
                    format!("{} sets mode {}", source, mode_string)
                };
                room.notify(&notification);
            },
            Notice  => {
                let (target, text) = message.get_notice_components();
                if self.get_room(target).is_some() {
                    self.get_room(target).unwrap().notify(text);
                } else {
                    self.server().notify(text);
                }
            },
            Ping    => {
                self.send("PONG");
            },
            Part    => {
                let room_name = message.param(0).text();
                if let Some(room) = self.get_room(room_name) {
                    room.handle_part(&message.user());
                }
            },
            Quit    => {
                let user = message.user();
                let reason = message.param(0).text();
                for room in &mut self.rooms {
                    if room.is_user_present(&user) {
                        room.handle_quit(&user, reason);
                    }
                }
            },
            Join    => {
                /*
                 * If this is about us, then create a new channel room and add it
                 * to self.rooms; otherwise add the user of the person who joined
                 * to the list of users in the target room.
                 */
                let room_name = message.param(0).text().to_string();
                if message.source() == self.config.nick {
                    self.join_room(&room_name);
                } else {
                    let room = self.get_room(&room_name).unwrap();
                    room.handle_join(message.user());
                }
            },
            PrivMsg => {
                let source = Text::decorate_nick(message.source());
                let privmsg = Message::new(source, message.param(1).clone());
                let target = message.param(0).text().to_string();
                let target = if target == self.config.nick { message.source() } else { &target[..] };
                let already_in_room = self.get_room(target).is_some();
                if !already_in_room {
                    self.join_room(target);
                }
                self.get_room(target).unwrap().add_message(privmsg);
            },
            _                       => { }
        }
    }

    fn get_room(&mut self, name: &str) -> Option<&mut Room> {
        for room in &mut self.rooms {
            if room.target() == name {
                return Some(room);
            }
        }

        return None;
    }

    fn update_status_line(&mut self) -> bool {
        let new_status_line = self.status_line();
        if new_status_line != self.status_line {
            self.status_line = new_status_line;
            return true;
        }
        return false;
    }

    fn status_line(&self) -> String {
        let mut status = String::new();
        for (i, room) in self.rooms.iter().enumerate() {
            let s = if i == self.active_room {
                room.status_string_active()
            } else {
                room.status_string()
            };
            status.push_str(&format!(" {} ", s));
        }
        return status;
    }

    fn should_update(&self) -> bool {
        return self.active_room().should_update();
    }

    /*
     * Let the client know that they should update the active room,
     * and clear the updated flag.
     */
    fn update(&mut self) {
        println!("UPDATE");
        self.active_room_mut().clear_notify();
    }

    fn active_room(&self) -> &Room {
        return &self.rooms[self.active_room];
    }

    fn active_room_mut(&mut self) -> &mut Room {
        return &mut self.rooms[self.active_room];
    }

    fn server(&mut self) -> &mut Room {
        return &mut self.rooms[0];
    }

    fn join_room(&mut self, name: &str) -> &mut Room {
        self.rooms.push(Room::new(name, &self.config));
        self.active_room = self.rooms.len() - 1;
        self.active_room().goto();
        return &mut self.rooms[self.active_room];
    }

    /*
     * Read bytes from a TcpStream until CRLF is encountered.
     */
    fn read_message(stream: &mut TcpStream, buf: &mut [u8], idx: &mut usize) -> Result<Vec<u8>, String> {

        loop {
            let mut i = 0;
            while i + 1 < *idx {
                if buf[i] == b'\r' && buf[i+1] == b'\n' {
                    let result = buf[..i].iter().cloned().collect::<Vec<_>>();
                    unsafe { ptr::copy(buf[i+2..].as_ptr(), buf[..].as_mut_ptr(), BUFFER_SIZE - (i + 2)); }
                    *idx -= i + 2;
                    if let Ok(s) = str::from_utf8(&result[..]) {
                        log!("RECEIVED: {}", s);
                    }
                    return Ok(result);
                }

                i += 1;
            }

            match stream.read(&mut buf[*idx..]) {
                Ok(n)  => *idx += n,
                Err(e) => return Err(e.to_string())
            };

        }
    }

    fn die(&self, error: &str) {
        log!("Error: {}", error);
        process::exit(-1);
    }

    fn quit(&mut self, message: &str) {
        self.send(&format!("QUIT: {}", message));
        println!("QUIT");
        process::exit(0);
    }

    fn part(&mut self, message: &str) {
        if self.active_room().is_server() {
            /*
             * If /part is used in the server room, we simply quit.
             */
            self.quit(message);
        } else {
            let target = self.active_room().target().to_string();

            /*
             * If we're in a channel, send a PART message; otherwise, we
             * are in a private chat and therefore do not need to send one.
             */
            if target.is_channel_name() {
                self.send(&format!("PART {} :{}", target, message));
            }

            /*
             * Remove the current room from the room list, and adjust the current
             * room index if it's now out of range.
             */
            self.rooms.remove(self.active_room);
            if self.active_room == self.rooms.len() {
                self.active_room -= 1;
            }

            self.active_room().goto();
        }
    }

    fn auth_sasl(&mut self) -> Option<String> {
        self.send("CAP REQ :sasl");

        self.identify();

        if !self.wait_for_or("ACK :sasl", "NAK :sasl") {
            return Some(format!("the ircd at {} does not support SASL", self.config.server));
        }

        self.send("AUTHENTICATE PLAIN");
        if !self.wait_for("AUTHENTICATE +") {
            return Some(format!("timed out waiting for reponse"));
        }

        let auth_string = {
            let username = &self.config.username[..];
            let password = self.config.password.as_ref().map(|s| &s[..]).unwrap();
            format!("{}\0{}\0{}", username, username, password)
                .as_bytes()
                .to_base64(base64::Config {
                    char_set:    base64::CharacterSet::Standard,
                    newline:     base64::Newline::CRLF,
                    pad:         true,
                    line_length: None
                })
        };

        self.send(&format!("AUTHENTICATE {}", &auth_string));

        if !self.wait_for_or("authentication successful", "authentication failed") {
            return Some(format!("invalid username/password combination"));
        }

        self.send("CAP END");

        return None;
    }

    fn auth_nickserv(&mut self) -> Option<String> {
        None
    }

    fn identify(&mut self) -> Option<String> {
        let nick_string = format!("NICK {}", self.config.nick);
        self.send(&nick_string);

        let user_string = format!("USER {} {} _ :{}", self.config.username, "_", "_");
        self.send(&user_string);

        return None;
    }

    fn send(&mut self, text: &str) {
        log!("SENDING: {}", text);
        self.stream.write(text.as_bytes()).unwrap();
        self.stream.write(b"\r\n").unwrap();
    }

    /*
     * Wait until an incoming message matches either `good` or `bad` (or we time out).
     *
     * good          -> true
     * bad / timeout -> false
     */
    fn wait_for_or(&mut self, good: &str, bad: &str) -> bool {
        let mut buf = [0u8; BUFFER_SIZE];
        let mut idx = 0usize;
        while let Ok(msg) = Session::read_message(&mut self.stream, &mut buf, &mut idx).and_then(|bs| IrcMessage::parse(&bs[..])) {
            msg.log();
            let good = msg.contains(good);
            let bad = msg.contains(bad);
            self.handle_message(msg);
            if good { return true }
            if bad { return false }
        }

        return false;
    }

    /*
     * Wait until either an incoming message matches either `good`, or we time out.
     *
     * good    -> true
     * timeout -> false
     */
    fn wait_for(&mut self, good: &str) -> bool {
        let mut buf = [0u8; BUFFER_SIZE];
        let mut idx = 0usize;
        while let Ok(msg) = Session::read_message(&mut self.stream, &mut buf, &mut idx).and_then(|bs| IrcMessage::parse(&bs[..])) {
            msg.log();
            let good = msg.contains(good);
            self.handle_message(msg);
            if good { return true }
        }

        return false;
    }
}
