use super::App;
use crate::vr::{self, Seen, VrConfig};

impl App {
    pub fn set_vr_config(&mut self, config: VrConfig) {
        self.vr.set_config(config);
    }

    pub(super) fn buzz_alerts(&mut self) {
        let cur = Seen {
            target_since: self.gs.target_since,
            target_me: self.gs.boss.is_some() && self.gs.targets_me(),
            token_alerts: self.gs.token_alerts,
            clear: self.gs.stage_clear(),
            kill: self.gs.last_kill.as_ref().map(|k| k.ts),
        };
        if cur == self.buzz_seen {
            return;
        }
        let fired = vr::alerts(&self.buzz_seen, &cur);
        self.buzz_seen = cur;
        if !self.primed {
            return;
        }
        for a in fired {
            self.vr.buzz(a);
        }
    }
}
