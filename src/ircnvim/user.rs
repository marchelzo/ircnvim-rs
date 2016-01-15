use std::hash::Hash;
use std::hash::Hasher;
use std::option::Option;
use std::string::ToString;

#[derive(Debug, Clone, Eq)]
pub struct User {
    pub nick: String,
    pub username: Option<String>,
    pub host: Option<String>
}

impl User {
    pub fn new(nick: String, username: Option<String>, host: Option<String>) -> User {
        return User {
            nick: nick,
            username: username,
            host: host
        };
    }

    pub fn from_nick(nick: String) -> User {
        return User {
            nick: nick,
            username: None,
            host: None
        }
    }

    pub fn actual_nick(&self) -> &str {
        let i = self.nick.find(|c| c != '@' && c != '+').unwrap();
        return &self.nick[i..];
    }
}

impl ToString for User {
    fn to_string(&self) -> String {
        return format!(
            "{}!{}@{}",
            self.actual_nick(),
            self.username.as_ref().map(|s| &s[..]).unwrap_or(""),
            self.host.as_ref().map(|s| &s[..]).unwrap_or("")
        );
    }
}

impl PartialEq<User> for User {
    fn eq(&self, other: &User) -> bool {
        return self.actual_nick() == other.actual_nick();
    }
}

impl Hash for User {
    fn hash<H>(&self, hasher: &mut H) where H: Hasher {
        self.actual_nick().hash(hasher);
    }
}
