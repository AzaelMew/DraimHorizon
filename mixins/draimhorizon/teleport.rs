// Only a server-sourced, exact request can create a local teleport card.
use std::collections::VecDeque;
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Request {
    pub trade: bool,
    pub name: String,
    pub accept: String,
    pub deny: String,
    pub at: u64,
    pub id: u64,
}
#[derive(Default)]
pub struct Requests {
    pub entries: VecDeque<Request>,
    next: u64,
}
pub fn name(text: &str) -> bool {
    !text.is_empty()
        && text.len() <= 16
        && text.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
}
pub fn parse(text: &str) -> Option<(String, String, String)> {
    if text.len() > 512 {
        return None;
    }
    let text = super::claim_borders::strip_formatting(text);
    if let Some((who, tail)) = text.trim().split_once(" wants to trade with you: ") {
        if !name(who) {
            return None;
        }
        let tail = tail.strip_suffix(" (60 s).")?;
        let (accept, deny) = tail.split_once(" or ")?;
        if accept != format!("/trade accept {who}") || deny != format!("/trade deny {who}") {
            return None;
        }
        return Some((
            who.into(),
            format!("trade accept {who}"),
            format!("trade deny {who}"),
        ));
    }
    let (who, tail) = text.trim().split_once(" asks to teleport to you: ")?;
    if !name(who) {
        return None;
    }
    let tail = tail.strip_suffix('.')?;
    let (a, d) = tail.split_once(" or ")?;
    let (accept, target) = a.split_once(' ')?;
    let (deny, other) = d.split_once(' ')?;
    if !["/tpaccept", "/tpaaccept"].contains(&accept)
        || !["/tpdeny", "/tpadeny"].contains(&deny)
        || target != who
        || other != who
    {
        return None;
    }
    Some((
        who.into(),
        format!("{} {who}", &accept[1..]),
        format!("{} {who}", &deny[1..]),
    ))
}
impl Requests {
    pub fn capture(&mut self, text: &str, now: u64) -> bool {
        let Some((name, accept, deny)) = parse(text) else {
            return false;
        };
        let trade = accept.starts_with("trade ");
        self.prune(now);
        if let Some(old) = self
            .entries
            .iter()
            .find(|r| r.name == name && r.trade == trade)
            && now.saturating_sub(old.at) < 1000
        {
            return true;
        }
        self.entries.retain(|r| r.name != name || r.trade != trade);
        if self.entries.len() >= 8 {
            self.entries.pop_front();
        }
        self.next = self.next.wrapping_add(1).max(1);
        self.entries.push_back(Request {
            trade,
            name,
            accept,
            deny,
            at: now,
            id: self.next,
        });
        true
    }
    pub fn prune(&mut self, now: u64) {
        self.entries.retain(|r| now.saturating_sub(r.at) < 60000);
    }
    pub fn choose(&mut self, id: u64, accept: bool, now: u64) -> Option<String> {
        self.prune(now);
        let i = self.entries.iter().position(|r| r.id == id)?;
        let r = self.entries.remove(i)?;
        Some(if accept { r.accept } else { r.deny })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn accepts_both_server_command_spellings_and_rejects_injection() {
        assert_eq!(
            parse("Zen0g asks to teleport to you: /tpaccept Zen0g or /tpdeny Zen0g.")
                .unwrap()
                .1,
            "tpaccept Zen0g"
        );
        assert_eq!(
            parse("Alex asks to teleport to you: /tpaaccept Alex or /tpadeny Alex.")
                .unwrap()
                .2,
            "tpadeny Alex"
        );
        for bad in [
            "<Alex> Steve asks to teleport to you: /tpaccept Steve or /tpdeny Steve.",
            "Steve asks to teleport to you: /op Steve or /tpdeny Steve.",
            "Steve asks to teleport to you: /tpaccept Other or /tpdeny Steve.",
        ] {
            assert!(parse(bad).is_none());
        }
    }
    #[test]
    fn click_is_once_only_and_expired_requests_cannot_run() {
        let mut r = Requests::default();
        let text = "Alex asks to teleport to you: /tpaccept Alex or /tpdeny Alex.";
        r.capture(text, 10);
        r.capture(text, 100);
        assert_eq!(r.entries[0].at, 10);
        let id = r.entries[0].id;
        assert!(r.choose(id, true, 60010).is_none());
        r.capture(text, 70000);
        let id = r.entries[0].id;
        assert_eq!(r.choose(id, false, 70001), Some("tpdeny Alex".into()));
        assert!(r.choose(id, true, 70002).is_none());
        r.capture(text, 80000);
        let old = r.entries[0].id;
        r.capture(text, 82000);
        assert_eq!(r.entries[0].at, 82000);
        assert!(r.choose(old, true, 82001).is_none());
    }
}

#[cfg(test)]
mod trade_tests {
    use super::*;
    #[test]
    fn trade_and_teleport_coexist_and_commands_are_consumed_once() {
        let mut requests = Requests::default();
        assert!(requests.capture(
            "Alex wants to trade with you: /trade accept Alex or /trade deny Alex (60 s).",
            100
        ));
        assert!(requests.capture(
            "Alex asks to teleport to you: /tpaccept Alex or /tpdeny Alex.",
            100
        ));
        assert_eq!(requests.entries.len(), 2);
        let id = requests.entries[0].id;
        assert_eq!(
            requests.choose(id, true, 101).as_deref(),
            Some("trade accept Alex")
        );
        assert!(requests.choose(id, true, 102).is_none());
        assert!(
            parse("Alex wants to trade with you: /trade accept Other or /trade deny Alex (60 s).")
                .is_none()
        );
        requests.prune(60100);
        assert!(requests.entries.is_empty());
    }
}
