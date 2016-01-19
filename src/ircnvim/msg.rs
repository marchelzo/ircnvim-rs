use ircnvim::text::Text;
use time;
use time::Tm;

pub struct Message {
    pub time: Tm,
    pub source: Text,
    pub body: Text,
    pub text: String,
    pub is_notification: bool
}

impl Message {
    pub fn new(source: Text, body: Text) -> Message {
        let time = time::now();
        let text = if body.action {
            format!(" [{}] {: >18}  {} {}", time.strftime("%H:%M:%S").unwrap(), " ", source.text(), body.text())
        } else  {
            format!(" [{}] {: >18}  {}", time.strftime("%H:%M:%S").unwrap(), source.text(), body.text())
        };
        return Message {
            time: time,
            source: source,
            body: body,
            text: text,
            is_notification: false
        };
    }

    /*
     * Make an ACTION from ourselves.
     */
    pub fn action(nick: &str, body: String) -> Message {
        let source = Text::decorate_nick(nick);
        let body = Text::action(body);
        return Message::new(source, body);
    }

    pub fn notification(body: Text) -> Message {
        let time = time::now();
        let text = format!(" [{}] {: >18}  {}", time.strftime("%H:%M:%S").unwrap(), "", body.text());
        return Message {
            time: time,
            source: Text::from_string(String::new()),
            body: body,
            text: text,
            is_notification: true
        };
    }
}
