//! Native Sunrise entry point. Only the declared connection mixin changes behavior.
use mod_api::{self, Hooks};

mod policy;
pub use policy::brand_for_address;

#[derive(Default)]
struct Mod;

#[mod_api::sunrise_mod]
impl Hooks for Mod {}
