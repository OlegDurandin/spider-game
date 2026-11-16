use spider_engine::Session as Game;

#[derive(Default)]
pub struct SaveQueue {
    pub revision: i64,
    pub pending: Option<Game>,
    pub sending: bool,
    pub conflict: bool,
}
impl SaveQueue {
    pub fn enqueue(&mut self, game: Game) {
        self.pending = Some(game);
    }
    pub fn start(&mut self) -> bool {
        if self.sending || self.conflict || self.pending.is_none() {
            false
        } else {
            self.sending = true;
            true
        }
    }
    pub fn next(&mut self) -> Option<(Game, i64)> {
        if let Some(game) = self.pending.take() {
            Some((game, self.revision))
        } else {
            self.sending = false;
            None
        }
    }
    pub fn acknowledge(&mut self, revision: i64) {
        self.revision = revision;
    }
    pub fn fail(&mut self, game: Game, conflict: bool) {
        if self.pending.is_none() {
            self.pending = Some(game);
        }
        self.sending = false;
        self.conflict = conflict;
    }
    pub fn idle_at(&self, revision: i64) -> bool {
        !self.sending && self.pending.is_none() && !self.conflict && self.revision == revision
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use spider_engine::{Kind, Mode};
    fn game(seed: u64) -> Game {
        Game::new(Kind::Spider, seed, Mode::One)
    }
    #[test]
    fn sequential_queue_keeps_latest_move_and_new_revision() {
        let first = game(1);
        let mut latest = first.clone();
        if let Game::Spider(g) = &mut latest {
            g.deal().unwrap();
        }
        let mut q = SaveQueue::default();
        q.enqueue(first.clone());
        assert!(q.start());
        assert_eq!(q.next(), Some((first, 0)));
        q.enqueue(game(2));
        q.enqueue(latest.clone());
        assert!(!q.start());
        q.acknowledge(1);
        assert_eq!(q.next(), Some((latest, 1)));
        q.acknowledge(2);
        assert!(q.next().is_none());
        assert!(q.idle_at(2));
        assert!(!q.idle_at(1));
    }
    #[test]
    fn failed_request_retries_latest_state_without_losing_a_new_move() {
        let first = game(1);
        let latest = game(2);
        let mut q = SaveQueue::default();
        q.enqueue(first.clone());
        q.start();
        q.next();
        q.enqueue(latest.clone());
        q.fail(first, false);
        assert!(q.start());
        assert_eq!(q.next(), Some((latest, 0)));
    }
    #[test]
    fn conflict_prevents_automatic_overwrite_and_retains_local_branch() {
        let game = game(1);
        let mut q = SaveQueue::default();
        q.enqueue(game.clone());
        q.start();
        q.next();
        q.fail(game.clone(), true);
        assert!(!q.start());
        assert!(!q.idle_at(0));
        assert_eq!(q.pending, Some(game));
    }
    #[test]
    fn switching_games_does_not_acknowledge_or_replace_another_slots_pending_save() {
        let mut spider = SaveQueue::default();
        let mut klondike = SaveQueue::default();
        let first = game(42);
        let mut next = Game::new(Kind::Klondike, u64::MAX, Mode::One);
        if let Game::Klondike(g) = &mut next {
            g.apply(spider_engine::klondike::Command::Draw).unwrap();
        }
        spider.enqueue(first.clone());
        assert!(spider.start());
        assert_eq!(spider.next(), Some((first, 0)));
        klondike.enqueue(next.clone());
        assert!(klondike.start());
        assert_eq!(klondike.next(), Some((next.clone(), 0)));
        spider.acknowledge(7);
        assert_eq!(klondike.revision, 0);
        klondike.fail(next.clone(), false);
        assert_eq!(klondike.pending, Some(next));
        assert!(spider.next().is_none());
    }
}
