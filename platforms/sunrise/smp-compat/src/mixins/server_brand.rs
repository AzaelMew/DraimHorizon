fn server_brand(payload: &cfg::CustomPayload, send: &ingress::Producer<Event>) {
    if payload.channel != "minecraft:brand" || payload.data != b"\x07horizon" {
        return;
    }
    let channel = std::sync::Arc::as_ptr(&send.sender.0) as usize;
    let outcome = mod_draimhorizon_smp_compat::rendezvous()
        .announce(channel, std::time::Duration::from_secs(2));
    // One line in latest.log per official-SMP join, so a refused login shows whether the brand went out.
    if outcome != mod_draimhorizon_smp_compat::Outcome::NotSelected {
        log::info!("DraimHorizon: Horizon SMP brand {outcome:?}");
    }
}
