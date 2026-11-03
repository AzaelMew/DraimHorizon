fn claim_borders_actionbar_received(game:&crate::game::Game<'_>,packet:&horizon_protocol::packets::play::Clientbound){
    use horizon_protocol::packets::play::Clientbound as C;
    let text=match packet{
        C::SetActionBarText(p)=>remote_text(&p.text),
        C::SystemChat(p) if p.overlay=>remote_text(&p.content),
        _=>return,
    };
    crate::game::claim_borders_claim_observations_view::action_bar(game,&text.plain());
}
