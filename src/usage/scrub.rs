use super::MAX_STR;
use std::collections::{HashMap, HashSet};

pub(crate) struct Scrub {
    me: HashSet<String>,
    names: Vec<String>,
    tags: HashMap<String, String>,
    pub salt: String,
}

fn edge(c: Option<char>) -> bool {
    c.is_none_or(|c| !c.is_alphanumeric())
}

fn find_word(s: &str, name: &str, mut from: usize) -> Option<usize> {
    let open = name.chars().next().is_some_and(char::is_alphanumeric);
    let close = name.chars().next_back().is_some_and(char::is_alphanumeric);
    while let Some(i) = s.get(from..)?.find(name) {
        let at = from + i;
        let end = at + name.len();
        if (!open || edge(s[..at].chars().next_back())) && (!close || edge(s[end..].chars().next()))
        {
            return Some(at);
        }
        from = at + s[at..].chars().next().map_or(1, char::len_utf8);
    }
    None
}

impl Scrub {
    pub(crate) fn new(me: &[String], others: &[String]) -> Scrub {
        let mut names: Vec<String> = me
            .iter()
            .chain(others)
            .map(|n| n.trim().to_string())
            .filter(|n| !n.is_empty())
            .collect();
        names.sort_by(|a, b| b.len().cmp(&a.len()).then_with(|| a.cmp(b)));
        names.dedup();
        Scrub {
            me: me.iter().map(|n| n.trim().to_string()).collect(),
            names,
            tags: HashMap::new(),
            salt: String::new(),
        }
    }

    pub(crate) fn reset(&mut self) {
        self.tags.clear();
    }

    pub(crate) fn who(&mut self, name: &str) -> String {
        let name = name.trim();
        if self.me.contains(name) {
            return "you".to_string();
        }
        let next = format!("P{}", self.tags.len() + 1);
        self.tags.entry(name.to_string()).or_insert(next).clone()
    }

    pub(crate) fn clean(&mut self, s: &str) -> String {
        let mut out: String = s
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        let hits: Vec<String> = self
            .names
            .iter()
            .filter(|n| out.contains(n.as_str()))
            .cloned()
            .collect();
        for name in hits {
            let mut tag = None;
            let mut from = 0;
            while let Some(at) = find_word(&out, &name, from) {
                let tag = tag.get_or_insert_with(|| self.who(&name)).clone();
                out.replace_range(at..at + name.len(), &tag);
                from = at + tag.len();
            }
        }
        out = drop_ids(&out);
        out.chars()
            .take(MAX_STR)
            .collect::<String>()
            .trim()
            .to_string()
    }
}

fn drop_ids(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(i) = rest.find("usr_") {
        out.push_str(&rest[..i]);
        out.push_str("usr");
        rest = rest[i + 4..].trim_start_matches(|c: char| c.is_ascii_alphanumeric() || c == '-');
    }
    out.push_str(rest);
    out
}
