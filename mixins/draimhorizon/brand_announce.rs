// The first `minecraft:brand` this client sends, before the server has said anything; the official
// SMP judges exactly that value and reads it later in the same exchange. Every other destination
// keeps the client's own bytes (`vanilla`), so a Paper or vanilla server still sees an unmodified
// client, and the client's own server-brand-driven upgrade in `configure` is left untouched.
fn claim_borders_brand_announce(value: Vec<u8>) -> Vec<u8> {
    claim_borders_brand::announced_brand(value)
}
