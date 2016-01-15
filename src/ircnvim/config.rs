use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::collections::HashMap;

#[derive(PartialEq, Eq)]
pub enum AuthMethod {
    NoAuth,
    NickServ,
    SASL
}

pub struct Config {
    pub nick: String,
    pub username: String,
    pub password: Option<String>,
    pub server: String,
    pub directory: String,
    pub auth: AuthMethod
}

impl Config {
    /*
     * The configuration file should be of the form:
     *
     * *(<option> <value> \n)
     *
     * e.g.,
     *
     * nick marchelzo
     * username marchelzo
     * password foobarbaz
     *
     * Lines beginning with # are comments and are ignored.
     */
    pub fn load(directory: String, mut profile: Option<String>) -> Result<Config, String> {
        let config_path = format!("{}/config", directory);
        let f =  File::open(config_path);

        if let Err(e) = f {
            return Err(e.to_string());
        }

        let mut profiles: HashMap<String, HashMap<String, String>> = HashMap::new();

        let mut lines = BufReader::new(f.unwrap()).lines();
        while let Some(Ok(line)) = lines.next() {
            if line.starts_with("#") || line.is_empty() { continue }
            let mut p: HashMap<String, String> = HashMap::new();
            while let Some(Ok(option)) = lines.next() {
                if line.starts_with("#") { continue }
                let option = option.trim_right();
                if option.is_empty() { break }
                match &option.split(" ").collect::<Vec<_>>()[..] {
                    [key, val] => { p.insert(key.to_string(), val.to_string()); },
                    _          => return Err(format!("invalid option in configuration file: {}", option))
                }
                
            }
            profiles.insert(line.clone(), p);
            if profile.is_none() { profile = Some(line) }
        }


        let mut p = match profile {
            Some(name) => {
                match profiles.remove(&name) {
                    Some(p) => p,
                    None    => return Err(format!("no profile named {} defined in configuration file", name))
                }
            },
            None       => return Err(format!("no profiles defined in configuartion file"))
        };

        macro_rules! get_option {
            ($o:expr) => {{
                if let Some(v) = p.remove($o) {
                    v
                } else {
                    return Err(format!("{} is missing from profile", $o));
                }
            }}
        }

        let nick = get_option!("nick");
        let username = get_option!("username");
        let password = p.remove("password");
        let server   = get_option!("server");
        let auth = match p.remove("auth").map(|s| s.to_string().to_lowercase()).as_ref().map(|s| &s[..]) {
            None             => if password.is_some() { AuthMethod::SASL } else { AuthMethod::NoAuth },
            Some("nickserv") => AuthMethod::NickServ,
            Some("sasl")     => AuthMethod::SASL,
            Some("none")     => AuthMethod::NoAuth,
            Some(t)          => return Err(format!("invalid authentication method: {}", t))
        };

        if auth != AuthMethod::NoAuth && password.is_none() {
            return Err(format!("an authentication method was specified but no password was provided"));
        }

        return Ok(Config {
            nick: nick,
            username: username,
            password: password,
            server: server,
            directory: directory,
            auth: auth
        });
    }
}
