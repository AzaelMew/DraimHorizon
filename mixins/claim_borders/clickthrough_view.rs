// This module lives below game::remote to use its normal client interaction path.
use crate::game::{Game, Hit, claim_borders as claims, claim_borders_clickthrough as model,
    claim_borders_inventory_sort_view as inventory};
use horizon_client::first_person::Hand;
use winit::keyboard::KeyCode;
fn container(game:&Game<'_>,pos:[i32;3])->bool {
    let name=&game.res.facts().get(game.world.block(pos[0],pos[1],pos[2])).name;
    // A container with an actual menu. Server protections and locked chests are checked by useItemOn.
    matches!(&**name,"minecraft:chest"|"minecraft:trapped_chest"|"minecraft:barrel"|"minecraft:ender_chest"
        |"minecraft:hopper"|"minecraft:dropper"|"minecraft:dispenser"|"minecraft:furnace"
        |"minecraft:blast_furnace"|"minecraft:smoker"|"minecraft:brewing_stand"|"minecraft:crafter")
        || name.ends_with("_shulker_box")||&**name=="minecraft:shulker_box"
}
fn target(game:&Game<'_>)->Option<Hit> {
    if !game.native_on(claims::ID)||game.keys.contains(&KeyCode::ShiftLeft)
        ||game.keys.contains(&KeyCode::ShiftRight)||game.guest_use.is_some()||game.lan_acting.is_some()
        ||game.using.is_some()||game.spectator() {return None;}
    let [frames,signs]=inventory::interactions(game);
    let original=game.pick()?;
    let (eye,dir)=game.aim()?;
    let support=if let Some(entity)=game.pick_entity_hit() {
        let id=entity?;
        if !frames {return None;}
        let frame=game.item_frames.iter().find(|f|f.id as i32==id)?;
        let pos=model::supporting_frame(frame.pos,frame.step())?;
        // The frame must be the nearest entity, and its supporting block the first block hit.
        if original.pos!=pos{return None;}
        pos
    } else {
        if !signs{return None;}
        let state=game.world.block(original.pos[0],original.pos[1],original.pos[2]);
        let name=&game.res.facts().get(state).name;
        let facing=game.pack.assets.registry.state(state).and_then(|s|s.properties.get("facing")).map(String::as_str);
        let pos=model::supporting_sign(name,original.pos,facing)?;
        // Continue the same ray just past the sign. Never jump over another block.
        let distance=(0..3).map(|a|(original.point[a]-eye[a]).powi(2)).sum::<f64>().sqrt();
        let mut start=distance+0.02;
        let mut found=None;
        // Sign outline can be thick and can contain disjoint boxes. At most one sign block is skipped.
        while start<=game.block_reach.min(distance+1.75) {
            let origin=std::array::from_fn(|a|eye[a]+dir[a]*start);
            let next=game.clip_ray(origin,dir,game.block_reach-start)?;
            if next.pos!=original.pos {found=Some(next.pos);break;}
            start+=0.05;
        }
        if found!=Some(pos){return None;}
        pos
    };
    if !container(game,support){return None;}
    let (face,point)=model::support_hit(eye,dir,support,game.block_reach)?;
    Some(Hit{pos:support,face,point})
}
/// Called inside native_interaction_key, before any frame interaction packet is sent.
pub(crate) fn remote(game:&mut Game<'_>,button:i32,hand:Hand,swing:&mut bool,canceled:&mut bool) {
    if *canceled||button!=1||hand!=Hand::Main||game.remote.is_none(){return;}
    let Some(hit)=target(game) else{return;};
    let saved=game.forced_hit.replace(hit);
    // This sends the ordinary UseItemOn sequence and predicts only the normal container response.
    let _=game.remote_use_item_on(Hand::Main);
    game.forced_hit=saved;
    *swing=false;*canceled=true;
}
pub(crate) fn local(game:&mut Game<'_>)->Option<horizon_client::Result<Option<Hand>>> {
    if game.remote.is_some(){return None;}
    let hit=target(game)?;
    Some(game.with_hand(Hand::Main,|g|g.use_hand(Some(&hit))).map(|used|used.then_some(Hand::Main)))
}
/// Mark only a menu that follows a use on a matching physical storage block as trusted.
pub(crate) fn opened(game:&Game<'_>,packet:&horizon_protocol::packets::play::OpenScreen) {
    let remote=game.remote.as_ref();
    let menu=remote.and_then(|r|r.builtin.get("minecraft:menu")?.name(packet.menu)).unwrap_or("");
    let physical=remote.and_then(|r|r.use_target).is_some_and(|pos|{
        let name=&game.res.facts().get(game.world.block(pos[0],pos[1],pos[2])).name;
        crate::game::claim_borders_inventory_preferences::storage_menu(name,menu)
    });
    inventory::opened(packet.container,menu,physical);
}
