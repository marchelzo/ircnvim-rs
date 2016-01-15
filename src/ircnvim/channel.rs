use std::collections::HashSet;
use ircnvim::user::User;

const CHANNEL_STARTING_CHARACTERS: &'static str = "#&+!~";

pub trait IsChannelName {
    fn is_channel_name(&self) -> bool;
}

impl<'a> IsChannelName for &'a str {
    fn is_channel_name(&self) -> bool {
        return self.chars().nth(0).and_then(|c| CHANNEL_STARTING_CHARACTERS.find(c)).is_some();
    }
}

impl IsChannelName for String {
    fn is_channel_name(&self) -> bool {
        return (&self[..]).is_channel_name();
    }
}

pub struct Channel {
    pub name: String,
    pub topic: Option<String>,
    users: HashSet<User>
}

impl Channel {
    pub fn new(name: &str) -> Channel {
        return Channel {
            name: name.to_string(),
            topic: None,
            users: HashSet::new()
        };
    }

    pub fn add_user(&mut self, user: User) {
        self.users.insert(user);
    }

    pub fn remove_user(&mut self, user: &User) {
        self.users.remove(user);
    }

    pub fn rename(&mut self, user: &User, new_nick: &str) {
        self.users.remove(user);
        self.users.insert(User::from_nick(new_nick.to_string()));
    }

    pub fn is_user_present(&self, user: &User) -> bool {
        return self.users.contains(user);
    }

    pub fn set_topic(&mut self, topic: String) {
        self.topic = Some(topic);
    }

    pub fn num_users(&self) -> usize {
        return self.users.len();
    }
}

#[cfg(test)]

mod tests {

    use super::*;

    #[test]
    fn test_is_channel_name() {
        assert!("##c".to_string().is_channel_name());
        assert!("&mychan".is_channel_name());
        assert!("~otherchan".is_channel_name());
    }
}
