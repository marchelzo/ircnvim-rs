use ircnvim::text::Text;
use ircnvim::user::User;
use std::io::Write;
use std::io;
use std::iter::Peekable;
use std::option::Option;
use std::str;

use self::IrcMessagePrefix::*;

#[derive(Debug)]
pub enum IrcMessageType {
    ChannelURL,
    Error,
    Join,
    LUserClient,
    LUserOp,
    LUserUnknown,
    LUserChannels,
    LUserMe,
    Mode,
    MOTD,
    MOTDEnd,
    MOTDStart,
    Names,
    NamesEnd,
    Nick,
    Notice,
    Part,
    Ping,
    PrivMsg,
    Quit,
    Topic,
    TopicWhoTime,
    UnknownCommand,
    Welcome,
    YourHost,
    Created,
    MyInfo,
    NotImplemented
}

#[derive(Debug)]
pub enum IrcMessagePrefix {
    ServerPrefix(String),
    UserPrefix(User)
}

#[derive(Debug)]
pub struct IrcMessage {
    pub kind: IrcMessageType,
    pub prefix: Option<IrcMessagePrefix>,
    params: Vec<Text>,
    pub raw: Option<String>
}

/*
 * Helper functions that should probably go somewhere else.
 */
fn take_while_ref<T, I, F>(iter: &mut Peekable<I>, f: F) -> Vec<T>
    where F: Fn(&T) -> bool,
          I: Iterator<Item=T>,
          T: Copy {

    let mut result = Vec::new();
    
    loop {
        if !iter.peek().map(&f).unwrap_or(false) { break }
        result.push(iter.next().unwrap());
    }

    return result;
}

fn skip_while_ref<T, I, F>(iter: &mut Peekable<I>, f: F)
    where F: Fn(&T) -> bool,
          I: Iterator<Item=T>,
          T: Copy {

    loop { if !iter.peek().map(&f).unwrap_or(false) { break } }
}

impl IrcMessage {


    pub fn parse(bytes: &[u8]) -> Result<IrcMessage, String> {

        let raw = match str::from_utf8(bytes) {
            Ok(s) => Some(s.to_string()),
            _     => None
        };

        let mut bytes = bytes.iter().cloned().peekable();

        let mut prefix: Option<IrcMessagePrefix> = None;

        let first: String;
        let mut second = String::new();
        let mut third = String::new();

        if bytes.peek() == Some(&b':') {
            bytes.next();
            first = String::from_utf8(take_while_ref(&mut bytes, |&c| !(c == b' ' || c == b'@' || c == b'!'))).expect("first");

            let sep = match bytes.peek() {
                Some(&b' ') => None,
                Some(&b)    => Some(b),
                None        => return Err(format!("unexpected end of input while parsing message prefix"))
            };

            if sep.is_some() {
                bytes.next();
                second = String::from_utf8(take_while_ref(&mut bytes, |&c| !(c == b' ' || c == b'@'))).expect("second");
                bytes.next();
                third = String::from_utf8(take_while_ref(&mut bytes, |&c| c != b' ')).expect("third");
            }

            if second.is_empty() {
                if first.contains('.') {
                    prefix = Some(ServerPrefix(first));
                } else {
                    prefix = Some(UserPrefix(User::new(first, None, None)));
                }
            } else if sep == Some(b'!') {
                prefix = Some(UserPrefix(User::new(first, Some(second), Some(third))));
            } else {
                prefix = Some(UserPrefix(User::new(first, None, Some(second))));
            }

            match bytes.next() {
                Some(b' ')      => { },
                Some(b)         => return Err(format!("expecting ' ' but found: {}", b as char)),
                None            => return Err(format!("expecting ' ' but encountered end of input"))
            }
        }

        let kind_string = String::from_utf8(take_while_ref(&mut bytes, |&b| b != b' ')).expect("kind string");
        
        if kind_string.is_empty() {
            match String::from_utf8(bytes.collect::<Vec<_>>()) {
                Ok(remaining) => return Err(format!("reply type missing from message: {}", remaining)),
                _             => return Err(format!("reply type missing from message"))
            }
        }

        let kind = match &kind_string[..] {
            "328"     => IrcMessageType::ChannelURL,
            "NOTICE"  => IrcMessageType::Notice,
            "PRIVMSG" => IrcMessageType::PrivMsg,
            "ERROR"   => IrcMessageType::Error,
            "NICK"    => IrcMessageType::Nick,
            "JOIN"    => IrcMessageType::Join,
            "QUIT"    => IrcMessageType::Quit,
            "PART"    => IrcMessageType::Part,
            "251"     => IrcMessageType::LUserClient,
            "252"     => IrcMessageType::LUserOp,
            "253"     => IrcMessageType::LUserUnknown,
            "254"     => IrcMessageType::LUserChannels,
            "255"     => IrcMessageType::LUserMe,
            "MODE"    => IrcMessageType::Mode,
            "372"     => IrcMessageType::MOTD,
            "375"     => IrcMessageType::MOTDStart,
            "376"     => IrcMessageType::MOTDEnd,
            "001"     => IrcMessageType::Welcome,
            "002"     => IrcMessageType::YourHost,
            "003"     => IrcMessageType::Created,
            "004"     => IrcMessageType::MyInfo,
            "421"     => IrcMessageType::UnknownCommand,
            "353"     => IrcMessageType::Names,
            "366"     => IrcMessageType::NamesEnd,
            "PING"    => IrcMessageType::Ping,
            "332"     => IrcMessageType::Topic,
            "333"     => IrcMessageType::TopicWhoTime,
            _         => IrcMessageType::NotImplemented,
        };

        let mut params = Vec::new();

        while bytes.next() == Some(b' ') {
            /*
             * If only trailing spaces remain, just break.
             */
            skip_while_ref(&mut bytes, |&b| b == b' ');
            if bytes.peek() == None { break }

            if bytes.peek() == Some(&b':') {
                bytes.next();
                let bytes = bytes.collect::<Vec<_>>();
                if bytes.is_empty() { break }
                log!("calling Text::from_bytes on:\n  {}", str::from_utf8(&bytes[..]).unwrap());
                params.push(Text::from_bytes(bytes));
                break;
            } else {
                let bytes = take_while_ref(&mut bytes, |&b| b != b' ');
                log!("PARAMETER: `{}`\n", str::from_utf8(&bytes[..]).unwrap());
                params.push(Text::from_bytes(bytes));
            }
        }

        return Ok(IrcMessage {
            kind: kind,
            prefix: prefix,
            params: params,
            raw: raw
        });
    }

    /*
     * Returns the nick of the user who sent the message, or the string "server"
     * if it came from the server.
     */
    pub fn source(&self) -> &str {
        return match self.prefix {
            Some(UserPrefix(ref u)) => &u.nick,
            _                       => "server"
        };
    }

    pub fn param(&self, i: usize) -> &Text {
        return &self.params[i];
    }

    pub fn user(&self) -> User {
        return match self.prefix {
            Some(UserPrefix(ref u)) => u.clone(),
            _                       => panic!("message has no associated user")
        };
    }

    pub fn contains(&self, text: &str) -> bool {
        return match self.raw.as_ref() {
            Some(s) => s.contains(text),
            _       => false
        }
    }

    pub fn log(&self) {
        log!("{}", self.raw.as_ref().unwrap());
    }

    /*
     * Determine which room the NOTICE pretains to, and what the relevant information is.
     *
     *///                                 target  notice
    pub fn get_notice_components(&self) -> (&str, &str) {
        return match self.prefix {
            Some(ServerPrefix(_))   => {
                let t = self.param(0).text();
                let t = if t == "*" { "server" } else { t };
                (t, self.param(1).text())
            },
            Some(UserPrefix(ref u)) => {
                if u.nick == "ChanServ" {
                    let param = self.param(1).text();
                    let target = {
                        match param.find(']') {
                            Some(i) => &param[1..i],
                            None    => "server"
                        }
                    };
                    (target, param)
                } else {
                    ("server", self.param(1).text())
                }
            },
            _                       => unreachable!()
        };
    }

    pub fn params(&self) -> &[Text] {
        return &self.params[..];
    }

    /*
     * Returns the string formed by joining all parameters
     * (beginning with the ith one) by ' '.
     */
    pub fn sequence(&self, i: usize) -> String {
        let mut params = self.params.iter().skip(i).map(|t| t.text());
        return match params.next().map(|s| s.to_string()) {
            Some(s) => params.fold(s, |acc, param| acc + " " + param),
            _       => "".to_string()
        }
        
    }
}

#[cfg(test)]
mod tests {
    use ircnvim::irc::*;

    #[test]
    fn test_ping() {
        let message = IrcMessage::parse(b"PING :verne.freenode.net").unwrap();
        println!("{:?}", message);
        match message.kind {
            IrcMessageType::Ping => { },
            _                    => unreachable!()
        }
    }

    #[test]
    fn test_quit() {
        let message = IrcMessage::parse(b":harukomoto!~harukomot@93-34-148-177.ip50.fastwebnet.it QUIT :").unwrap();
        let message = IrcMessage::parse(b":Andrius!~andrius@5ec081cd.skybroadband.com QUIT :").unwrap();
        let message = IrcMessage::parse(b":randomstatistic!~randomsta@64.124.61.215 QUIT :").unwrap();
        assert_eq!(message.params().len(), 0);

    }

    #[test]
    fn test_mode_services() {
        let message = IrcMessage::parse(b":services. MODE #foobartest -o marchelzo").unwrap();
        println!("{:?}", message);
        match message.kind {
            IrcMessageType::Mode => { },
            _                    => unreachable!()
        }
        match message.prefix {
            Some(IrcMessagePrefix::ServerPrefix(_)) => { },
            _                                       => unreachable!()
        }

        assert_eq!(message.params().len(), 3);
    }
}
