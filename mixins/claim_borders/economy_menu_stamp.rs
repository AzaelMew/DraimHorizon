// The server's state id acknowledges content updates, including unchanged prices.
impl super::Game<'_> {
    pub(in crate::game) fn draimhorizons_menu_stamp(&self) -> i32 {
        self.remote.as_ref().map_or(0, |r| r.state_id)
    }
}
