// The launcher copies a hooked function with its module's imports into its own crate, where the game's
// `pub(crate)` re-exports of these two traits are out of reach. This module shadows the game's
// `api_values` with the same items, the traits taken from the crate that defines them.
pub use ::horizon_client::game::api_values::*;
pub use horizon_gameplay::api_values::{TryIntoApi, TryIntoEngine};
