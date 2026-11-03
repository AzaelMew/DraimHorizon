fn claim_borders_live_inventory(game:&crate::game::Game<'_>)->core::ops::ControlFlow<()>{
    if crate::game::claim_borders_commerce_view::active(game){core::ops::ControlFlow::Break(())}else{core::ops::ControlFlow::Continue(())}
}
