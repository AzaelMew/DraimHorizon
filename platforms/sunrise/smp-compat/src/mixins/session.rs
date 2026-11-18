fn session(session: &Session) {
    let channel = std::sync::Arc::as_ptr(&session.payload_events.sender.0) as usize;
    let rendezvous = mod_draimhorizon_smp_compat::rendezvous();
    if !rendezvous.needs_registration(channel, session.serial()) {
        return;
    }
    let selected = mod_draimhorizon_smp_compat::is_horizon_smp(&session.address);
    if !selected {
        rendezvous.register(channel, session.serial(), None);
        return;
    }
    let owner = std::sync::Arc::downgrade(&session.payload_events.sender.0);
    let writer = std::sync::Arc::downgrade(&session.writer);
    let configuring = std::sync::Arc::downgrade(&session.configuring);
    rendezvous.register(
        channel,
        session.serial(),
        Some(std::sync::Arc::new(move || {
            let (Some(_owner), Some(writer), Some(configuring)) =
                (owner.upgrade(), writer.upgrade(), configuring.upgrade())
            else {
                return false;
            };
            let Ok(writer) = writer.lock() else {
                return false;
            };
            if !configuring.load(std::sync::atomic::Ordering::Acquire) {
                return false;
            }
            let Some(writer) = writer.as_ref() else {
                return false;
            };
            if !writer.alive.load(std::sync::atomic::Ordering::Acquire) {
                return false;
            }
            let packet = cfg::Serverbound::CustomPayload(cfg::CustomPayload {
                channel: "minecraft:brand".into(),
                data: b"\x07horizon".to_vec(),
            });
            let Ok(body) = packet.try_encode() else {
                return false;
            };
            let capacity = body.capacity();
            let Ok(sender) = writer.sender.lock() else {
                return false;
            };
            let Some(sender) = sender.as_ref() else {
                return false;
            };
            sender
                .try_send(queued_writer::WriteJob::Packet(body), capacity)
                .is_ok()
        })),
    );
}
