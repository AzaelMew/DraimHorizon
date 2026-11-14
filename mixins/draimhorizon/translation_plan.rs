// Which received chat lines still need a translation request, and when to look again.
//
// The scheduler reproduces the original per-frame loop exactly: every render offers each eligible
// latest line that is not in flight, settled or backing off to the worker, and a full queue is
// offered again on the very next render. It only avoids re-judging lines whose answer cannot have
// changed.
use claim_borders_relay_client::translation::{Submission, TargetLanguage};
use std::{
    collections::{HashMap, HashSet, hash_map::DefaultHasher},
    hash::{Hash, Hasher},
};

const MAX_SETTLED: usize = 4096;

fn digest(text: &str) -> u64 {
    let mut hasher = DefaultHasher::new();
    text.hash(&mut hasher);
    hasher.finish()
}

/// Everything the candidate list depends on besides time and the plan's own state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Gate {
    pub feed_revision: u64,
    pub show_players: bool,
    pub death_events: bool,
    pub target: TargetLanguage,
    /// Changes with every new translator (source policy, key, relay, toggles).
    pub translator: u64,
}

#[derive(Default)]
pub struct Plan {
    /// Sent to the worker; the answer has not been applied yet, or it was a success (translated or
    /// returned unchanged) and the line stays settled, as in the original `requested` set.
    requested: HashMap<u64, u64>,
    /// Lines the worker declined for good, with the digest of the judged text.
    skipped: HashMap<u64, u64>,
    /// Failed lines: retry deadline and attempt count.
    retry: HashMap<u64, (u64, u8)>,
    failed_at: Option<u64>,
    gate: Option<Gate>,
    /// First deadline (a retry) that can make a line eligible again.
    wake: u64,
    /// The last pass ended on a full queue: every render offers the lines again.
    busy: bool,
}
impl Plan {
    /// Forget everything: another target, source policy, translator or chat scope.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
    pub fn failed_at(&self) -> Option<u64> {
        self.failed_at
    }
    pub fn retain(&mut self, ids: &HashSet<u64>) {
        self.requested.retain(|id, _| ids.contains(id));
        self.retry.retain(|id, _| ids.contains(id));
        self.skipped.retain(|id, _| ids.contains(id));
    }
    pub fn is_requested(&self, id: u64) -> bool {
        self.requested.contains_key(&id)
    }
    /// A result arrived and the provider did not fail: the line stays settled whether or not the
    /// text changed (a no-op result must not be sent again).
    pub fn finished(&mut self, id: u64) {
        self.failed_at = None;
        self.retry.remove(&id);
    }
    /// The provider failed this line: retry with growing delay.
    pub fn failed(&mut self, id: u64, now: u64, base_ms: u64) {
        self.requested.remove(&id);
        let attempts = self.retry.get(&id).map_or(1, |(_, n)| n.saturating_add(1));
        let at = now + (base_ms << attempts.saturating_sub(1).min(3));
        self.retry.insert(id, (at, attempts));
        self.failed_at = Some(now);
        self.wake = self.wake.min(at);
    }
    /// Offer `candidates` (newest first, already limited to translatable kinds) to `submit`.
    pub fn schedule<'a>(
        &mut self,
        gate: Gate,
        now: u64,
        candidates: impl Iterator<Item = (u64, &'a str)>,
        mut submit: impl FnMut(u64, &str) -> Submission,
    ) {
        if !self.busy && self.gate == Some(gate) && now < self.wake {
            return;
        }
        self.gate = Some(gate);
        self.wake = u64::MAX;
        self.busy = false;
        for (id, text) in candidates {
            let seen = digest(text);
            // In flight or settled for this very text; changed text is a new question.
            if self.requested.get(&id) == Some(&seen) {
                continue;
            }
            if let Some((at, _)) = self.retry.get(&id)
                && now < *at
            {
                self.wake = self.wake.min(*at);
                continue;
            }
            if self.skipped.get(&id) == Some(&seen) {
                continue;
            }
            match submit(id, text) {
                Submission::Queued => {
                    self.requested.insert(id, seen);
                }
                Submission::Skipped => {
                    if self.skipped.len() >= MAX_SETTLED {
                        self.skipped.clear();
                    }
                    self.skipped.insert(id, seen);
                }
                // The original kept offering a line the full queue refused on every render.
                Submission::Busy => self.busy = true,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    const TARGET: TargetLanguage = TargetLanguage::Russian;
    fn gate(feed_revision: u64) -> Gate {
        Gate {
            feed_revision,
            show_players: true,
            death_events: true,
            target: TARGET,
            translator: 1,
        }
    }

    /// One chat line as the scheduler sees it.
    #[derive(Clone)]
    struct Line {
        id: u64,
        text: String,
        translated: bool,
    }
    /// The original draw loop, verbatim in behavior: `requested`/`retry` sets and a boolean submit.
    #[derive(Default)]
    struct Original {
        requested: HashSet<u64>,
        retry: HashMap<u64, (u64, u8)>,
    }
    impl Original {
        fn result_ok(&mut self, id: u64) {
            self.retry.remove(&id);
        }
        fn result_failed(&mut self, id: u64, now: u64, base: u64) {
            self.requested.remove(&id);
            let attempts = self.retry.get(&id).map_or(1, |(_, n)| n.saturating_add(1));
            let delay = base << attempts.saturating_sub(1).min(3);
            self.retry.insert(id, (now + delay, attempts));
        }
        fn frame(&mut self, lines: &[Line], now: u64, mut submit: impl FnMut(u64, &str) -> bool) {
            for l in lines.iter().rev().take(64) {
                if !l.translated
                    && !self.requested.contains(&l.id)
                    && self.retry.get(&l.id).is_none_or(|(at, _)| now >= *at)
                    && submit(l.id, &l.text)
                {
                    self.requested.insert(l.id);
                }
            }
        }
    }
    /// A worker queue of fixed capacity that judges eligibility by content.
    struct Queue {
        capacity: usize,
        jobs: VecDeque<u64>,
    }
    impl Queue {
        fn eligible(text: &str) -> bool {
            text.starts_with("ru")
        }
        fn try_submit(&mut self, id: u64, text: &str) -> Submission {
            if !Self::eligible(text) {
                Submission::Skipped
            } else if self.jobs.len() >= self.capacity {
                Submission::Busy
            } else {
                self.jobs.push_back(id);
                Submission::Queued
            }
        }
    }

    fn lines(n: u64) -> Vec<Line> {
        (1..=n)
            .map(|id| Line {
                id,
                text: if id % 3 == 0 {
                    format!("en {id}")
                } else {
                    format!("ru {id}")
                },
                translated: false,
            })
            .collect()
    }

    #[test]
    fn a_no_op_result_settles_the_line_and_unrelated_changes_never_resend_it() {
        let mut plan = Plan::default();
        let mut sent = Vec::new();
        let feed = lines(2);
        let mut revision = 1;
        let offer = |plan: &mut Plan, revision, now, sent: &mut Vec<u64>| {
            plan.schedule(
                gate(revision),
                now,
                feed.iter().rev().map(|l| (l.id, l.text.as_str())),
                |id, _| {
                    sent.push(id);
                    Submission::Queued
                },
            );
        };
        offer(&mut plan, revision, 0, &mut sent);
        assert_eq!(sent, [2, 1]);
        // The provider answers both with text that equals the original (no translation applied).
        plan.finished(1);
        plan.finished(2);
        for now in 1..200 {
            revision += 1; // unrelated feed mutations: new lines, repeats, toggles, scrolling
            offer(&mut plan, revision, now, &mut sent);
        }
        assert_eq!(sent, [2, 1], "settled no-op lines are never resubmitted");
        // A changed text is a new question; a reset (policy, target, scope) asks again.
        let changed = [(1u64, "ru changed")];
        plan.schedule(gate(revision + 1), 500, changed.into_iter(), |id, _| {
            sent.push(id);
            Submission::Queued
        });
        assert_eq!(sent, [2, 1, 1]);
        plan.reset();
        sent.clear();
        offer(&mut plan, 1, 600, &mut sent);
        assert_eq!(sent, [2, 1]);
    }

    #[test]
    fn a_changed_target_or_translator_generation_reconsiders_skipped_lines() {
        let mut plan = Plan::default();
        let calls = std::cell::Cell::new(0);
        let offer = |plan: &mut Plan, g: Gate, verdict| {
            plan.schedule(g, 0, [(1u64, "hello")].into_iter(), |_, _| {
                calls.set(calls.get() + 1);
                verdict
            });
        };
        offer(&mut plan, gate(1), Submission::Skipped);
        offer(&mut plan, gate(2), Submission::Skipped);
        assert_eq!(
            calls.get(),
            1,
            "the skip verdict stands for this text and policy"
        );
        // Another target/policy discards the translator, and with it the plan.
        plan.reset();
        offer(
            &mut plan,
            Gate {
                target: TargetLanguage::English,
                ..gate(2)
            },
            Submission::Queued,
        );
        assert_eq!(calls.get(), 2);
        assert!(plan.is_requested(1));
    }

    #[test]
    fn a_full_queue_is_offered_again_on_the_very_next_render() {
        let mut plan = Plan::default();
        let mut queue = Queue {
            capacity: 2,
            jobs: VecDeque::new(),
        };
        let feed = lines(8);
        let mut offered = Vec::new();
        for frame in 0..4 {
            plan.schedule(
                gate(1),
                frame,
                feed.iter().rev().map(|l| (l.id, l.text.as_str())),
                |id, text| {
                    offered.push((frame, id));
                    queue.try_submit(id, text)
                },
            );
        }
        // Frame 0 fills the queue and every later frame probes again with no added delay.
        assert!(offered.iter().any(|(f, _)| *f == 3));
        assert_eq!(queue.jobs.len(), 2);
        // Draining makes room: the next render queues the next lines, nothing was dropped.
        queue.jobs.clear();
        let before = plan.requested.len();
        plan.schedule(
            gate(1),
            4,
            feed.iter().rev().map(|l| (l.id, l.text.as_str())),
            |id, text| queue.try_submit(id, text),
        );
        assert_eq!(plan.requested.len(), before + 2);
    }

    /// Scripted frames: new lines, results (translated / no-op / failed), queue draining. The plan
    /// must offer exactly the lines the original loop offered, frame for frame.
    #[test]
    fn scheduling_matches_the_original_loop_frame_for_frame() {
        let mut seed = 0x1234_5678_9abc_def1u64;
        let mut rnd = move |n: u64| {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed % n
        };
        for capacity in [1, 2, 4, 32] {
            let base = 50;
            let mut feed: Vec<Line> = Vec::new();
            let mut revision = 1u64;
            let (mut original, mut plan) = (Original::default(), Plan::default());
            let (mut q_old, mut q_new) = (
                Queue {
                    capacity,
                    jobs: VecDeque::new(),
                },
                Queue {
                    capacity,
                    jobs: VecDeque::new(),
                },
            );
            for frame in 0..600u64 {
                let now = frame * 16;
                // Results arrive for the jobs the workers finished (the same for both).
                let done = (rnd(3) as usize).min(q_old.jobs.len());
                for _ in 0..done {
                    let id = q_old.jobs.pop_front().unwrap();
                    assert_eq!(q_new.jobs.pop_front(), Some(id));
                    match rnd(3) {
                        0 => {
                            // Translated: the line gets its translation.
                            original.result_ok(id);
                            plan.finished(id);
                            if let Some(l) = feed.iter_mut().find(|l| l.id == id) {
                                l.translated = true;
                            }
                            revision += 1;
                        }
                        1 => {
                            // Success without a translation (no-op / already target).
                            original.result_ok(id);
                            plan.finished(id);
                        }
                        _ => {
                            original.result_failed(id, now, base);
                            plan.failed(id, now, base);
                        }
                    }
                }
                if rnd(4) == 0 {
                    let id = feed.len() as u64 + 1;
                    let text = if rnd(3) == 0 {
                        format!("en {id}")
                    } else {
                        format!("ru {id}")
                    };
                    feed.push(Line {
                        id,
                        text,
                        translated: false,
                    });
                    revision += 1;
                }
                if rnd(9) == 0 {
                    revision += 1; // an unrelated mutation (toggle, repeat count, scroll)
                }
                let (mut old_calls, mut new_calls) = (Vec::new(), Vec::new());
                original.frame(&feed, now, |id, text| {
                    old_calls.push(id);
                    q_old.try_submit(id, text) == Submission::Queued
                });
                plan.schedule(
                    gate(revision),
                    now,
                    feed.iter()
                        .rev()
                        .take(64)
                        .filter(|l| !l.translated)
                        .map(|l| (l.id, l.text.as_str())),
                    |id, text| {
                        new_calls.push(id);
                        q_new.try_submit(id, text)
                    },
                );
                // Same jobs reach the worker in the same order on the same frame.
                assert_eq!(q_old.jobs, q_new.jobs, "capacity {capacity} frame {frame}");
                // The plan may skip lines the original re-judged to no effect, never the reverse.
                for id in &new_calls {
                    assert!(old_calls.contains(id));
                }
            }
        }
    }

    #[test]
    fn failures_back_off_per_line_and_wake_the_gate() {
        let mut plan = Plan::default();
        let feed = [(1u64, "ru one")];
        let calls = std::cell::Cell::new(0);
        let run = |plan: &mut Plan, now| {
            plan.schedule(gate(1), now, feed.into_iter(), |_, _| {
                calls.set(calls.get() + 1);
                Submission::Queued
            });
        };
        run(&mut plan, 0);
        plan.failed(1, 100, 15_000);
        assert_eq!(plan.failed_at(), Some(100));
        run(&mut plan, 14_999);
        assert_eq!(calls.get(), 1);
        run(&mut plan, 15_100);
        assert_eq!(
            calls.get(),
            2,
            "retry at the deadline, with an unchanged gate"
        );
        plan.failed(1, 15_100, 15_000);
        run(&mut plan, 15_100 + 29_999);
        assert_eq!(calls.get(), 2);
        run(&mut plan, 15_100 + 30_000);
        assert_eq!(calls.get(), 3);
        plan.finished(1);
        assert_eq!(plan.failed_at(), None);
    }

    #[test]
    fn departed_lines_are_forgotten_and_the_skip_memory_is_bounded() {
        let mut plan = Plan::default();
        let feed = [(1u64, "ru one"), (2, "ru two")];
        plan.schedule(gate(1), 0, feed.into_iter(), |_, _| Submission::Queued);
        plan.retain(&HashSet::from([2]));
        assert!(plan.is_requested(2) && !plan.is_requested(1));
        let texts: Vec<_> = (0..6_000u64).map(|i| (i, format!("line {i}"))).collect();
        plan.schedule(
            gate(2),
            0,
            texts.iter().map(|(id, t)| (*id, t.as_str())),
            |_, _| Submission::Skipped,
        );
        assert!(plan.skipped.len() <= MAX_SETTLED);
    }
}
