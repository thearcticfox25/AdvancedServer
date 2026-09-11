pub struct Cmd {
    pub name: String,
    pub args: Vec<String>,
}

impl Cmd {
    pub fn arg(&self, index: usize) -> &str {
        self.args.get(index).map(String::as_str).unwrap_or("")
    }
}

pub fn parse_cmd(input: &str) -> Option<Cmd> {
    let trimmed = input.trim();
    let prefix = trimmed.chars().next()?;
    if prefix != ':' && prefix != '.' {
        return None;
    }
    let after = trimmed[prefix.len_utf8()..].trim_start();
    let (name_part, rest) = match after.find(|ch: char| ch.is_whitespace()) {
        Some(space) => (&after[..space], after[space + 1..].trim()),
        None => (after, ""),
    };
    let name = name_part.trim().to_lowercase();
    if name.is_empty() {
        return None;
    }
    let args: Vec<String> = rest.split_whitespace().map(String::from).collect();
    Some(Cmd { name, args })
}
