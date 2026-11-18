// The network thread starts here, before the socket and the login. The typed address is the only
// stable destination input (an SRV record resolves the connect port, a friend tunnel hides it), so
// the login-brand decision for this very connection is taken now and read below by `brand()`.
fn claim_borders_brand_gate(address: &str) {
    claim_borders_brand::force_for(address);
}
