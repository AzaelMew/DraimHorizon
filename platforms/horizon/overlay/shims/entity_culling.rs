// `EntityCullDecision` and `cull_box` are private in the game's `entity_culling`; see shims/api_values.rs.
pub use ::horizon_client::game::entity_culling::*;
pub use horizon_client_gameplay::entity_occlusion::{EntityCullDecision, cull_box};
