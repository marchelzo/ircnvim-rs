use ircnvim::channel::Channel;
use ircnvim::channel::IsChannelName;
use ircnvim::config::Config;
use ircnvim::msg::Message;
use ircnvim::text::Text;
use ircnvim::user::User;
use ircnvim::_my_nick_regex;
use std::fs::File;
use std::io::Write;
use std::iter::Iterator;
use self::NotifyLevel::*;

pub enum RoomType {
    Channel(Channel),
    Private(String),
    Server
}

#[derive(PartialEq, Eq)]
pub enum NotifyLevel {
    Nothing,     // literally no activity
    Unimportant, // joins, parts, quits, etc., but no privmsgs
    Normal,      // privmsg
    Important,   // privmsg containing our nick
}

 pub struct Room {
     kind: RoomType,
     msgs: Vec<Message>,
     pub escaped_file_name: String,
     file: File,
     notify: NotifyLevel
 }

impl Room {
    pub fn new(name: &str, config: &Config) -> Room {
        let kind = if name.is_channel_name() {
            RoomType::Channel(Channel::new(name))
        } else {
            RoomType::Private(name.to_string())
        };

        return Room::make(kind, config);
    }

    pub fn server(config: &Config) -> Room {
        return Room::make(RoomType::Server, config);
    }

    fn make(kind: RoomType, config: &Config) -> Room {
        let file_name = format!("{}/{}/{}", config.directory, config.server, kind.file_name());
        let escaped_file_name = file_name.replace("#", "\\#");

        let file = match File::create(file_name) {
            Ok(file) => file,
            Err(e)   => panic!("{}", e)
        };

        return Room {
            kind: kind,
            msgs: Vec::new(),
            escaped_file_name: escaped_file_name,
            file: file,
            notify: Nothing,
        };
    }

    pub fn is_channel(&self) -> bool {
        match self.kind {
            RoomType::Channel(_) => true,
            _                    => false
        }
    }

    pub fn is_private(&self) -> bool {
        match self.kind {
            RoomType::Private(_) => true,
            _                    => false
        }
    }

    pub fn is_server(&self) -> bool {
        match self.kind {
            RoomType::Server => true,
            _                => false
        }
    }

    pub fn make_message(&mut self, input: &str) -> String {
        return match self.kind {
            RoomType::Server         => input.to_string(),
            RoomType::Channel(ref c) => format!("PRIVMSG {} :{}", c.name, input),
            RoomType::Private(ref n) => format!("PRIVMSG {} :{}", n, input)
        };
    }

    pub fn target(&self) -> &str {
        match self.kind {
            RoomType::Server         => "server",
            RoomType::Channel(ref c) => &c.name,
            RoomType::Private(ref n) => &n,
        }
    }

    pub fn add_message(&mut self, message: Message) {
        let nick_regex = unsafe { &*_my_nick_regex };
        writeln!(self.file, "{}", message.text).unwrap();
        if message.is_notification {
            self.notify = Unimportant;
        } else if nick_regex.is_match(message.body.text()) {
            self.notify = Important;
        } else {
            self.notify = Normal;
        }
        self.msgs.push(message);
    }

    pub fn messages(&self) -> &[Message] {
        return &self.msgs[..];
    }

    pub fn handle_join(&mut self, user: User) {
        let notification: String;
        match self.kind {
            RoomType::Channel(ref mut c) => {
                notification = format!("{} [{}] has joined {}", &user.nick, user.to_string(), &c.name);
                c.add_user(user);
            },
            _                            => unreachable!()
        }

        self.notify(&notification);
    }

    pub fn warn(&mut self, warning: &str) {
        let warning = Message::new(
            Text::from_string("!!!    ".to_string()),
            Text::from_string(warning.to_string())
        );
        self.add_message(warning);
    }

    pub fn notify(&mut self, message: &str) {
        let notification = Message::notification(Text::from_string(message.to_string()));
        self.add_message(notification);
    }

    pub fn is_user_present(&self, user: &User) -> bool {
        return match self.kind {
            RoomType::Channel(ref c) => c.is_user_present(user),
            RoomType::Private(ref n) => &n[..] == user.nick,
            _                        => false
        };
    }

    pub fn add_user(&mut self, user: User) {
        match self.kind {
            RoomType::Channel(ref mut c) => c.add_user(user),
            _                            => unreachable!()
        }
    }

    pub fn rename_user(&mut self, user: &User, new_nick: &str) {
        match self.kind {
            RoomType::Private(_)         => self.kind = RoomType::Private(new_nick.to_string()),
            RoomType::Channel(ref mut c) => c.rename(user, new_nick),
            _                            => unreachable!()
        }
    }

    pub fn handle_quit(&mut self, user: &User, reason: &str) {
        self.notify(&format!("{} [{}] has quit ({})", user.nick, user.to_string(), reason));
        match self.kind {
            RoomType::Channel(ref mut c) => c.remove_user(user),
            RoomType::Private(_)         => { },
            _                            => unreachable!()
        }
    }

    pub fn handle_part(&mut self, user: &User) {
        let notification = format!("{} [{}] has left {}", user.nick, user.to_string(), self.target());
        self.notify(&notification);
        match self.kind {
            RoomType::Channel(ref mut c) => c.remove_user(user),
            RoomType::Private(_)         => { },
            _                            => unreachable!()
        }
    }
    
    pub fn notify_topic(&mut self) {
        let topic = match self.kind {
            RoomType::Channel(ref c) => c.topic.as_ref().map(|s| s.clone()),
            _                        => unreachable!()
        };

        let target = self.target().to_string();

        if let Some(topic) = topic {
            self.notify(&format!("The topic for {} is {}", target, topic));
        }
    }

    pub fn set_topic(&mut self, topic: String) {
        match self.kind {
            RoomType::Channel(ref mut c) => c.set_topic(topic),
            _                            => unreachable!()
        }
    }

    pub fn goto(&self) {
        println!("GOTO {} {}", self.target(), self.escaped_file_name);
    }

    pub fn should_update(&self) -> bool {
        return self.notify != Nothing;
    }

    pub fn clear_notify(&mut self) {
        self.notify = Nothing;
    }

    pub fn status_string(&self) -> String {
        return format!(
            "{}{}",
            self.target(),
            match self.notify {
                Nothing     => "",
                Unimportant => ".",
                Normal      => "!",
                Important   => "*!"
            }
        );
    }

    pub fn status_string_active(&self) -> String {
        return format!(
            "[{}{}]",
            self.target(),
            match self.kind {
                RoomType::Channel(ref c) => format!(":{}", c.num_users()),
                _                        => "".to_string()
            }
        );
    }
}

impl RoomType {
    pub fn file_name(&self) -> String {
        match *self {
            RoomType::Server         => format!("server"),
            RoomType::Channel(ref c) => format!("channel_{}", c.name),
            RoomType::Private(ref n) => format!("private_{}", n)
        }
    }
}
