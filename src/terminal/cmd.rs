pub struct Cmd {
    pub prefix: char,
    pub name: String,
    pub args: Vec<String>,
}

impl Cmd {
    pub fn arg(&self, i: usize) -> &str {
        self.args.get(i).map(String::as_str).unwrap_or("")
    }

    pub fn rest_from(&self, from: usize) -> String {
        let idx = from.min(self.args.len());
        self.args[idx..].join(" ")
    }
}

pub fn parse_cmd(input: &str) -> Option<Cmd> {
    let s = input.trim();
    let prefix = s.chars().next()?;
    if prefix != ':' && prefix != '.' {
        return None;
    }
    let after = s[prefix.len_utf8()..].trim_start();
    let (name_part, rest) = match after.find(|c: char| c.is_whitespace()) {
        Some(i) => (&after[..i], after[i + 1..].trim()),
        None => (after, ""),
    };
    let name = name_part.trim().to_lowercase();
    if name.is_empty() {
        return None;
    }
    let args: Vec<String> = rest.split_whitespace().map(String::from).collect();
    Some(Cmd { prefix, name, args })
}
