// Read-only item search; no container clicks or inventory writes.
use crate::game::{Game,Screen,claim_borders as claims,claim_borders_input as input,claim_borders_ui_view as ui,native_hooks::NativeScreenView};
use horizon_client::{items::ItemStack,hud::HudBatch,clipboard::Clipboard};
use std::{collections::BTreeSet,sync::{Mutex,OnceLock}};
#[derive(Default)]
struct State {text:input::TextInput,focus:bool,rect:Option<[f32;4]>,copy:Option<String>,paste:bool,found:usize,total:usize,pressed:BTreeSet<i32>}
fn state<T>(f:impl FnOnce(&mut State)->T)->T {
    static STATE:OnceLock<Mutex<State>>=OnceLock::new();
    f(&mut STATE.get_or_init(||Mutex::new(State::default())).lock().unwrap_or_else(|p|p.into_inner()))
}
fn eligible(game:&Game<'_>)->bool {
    game.native_on(claims::ID)&&game.screen==Screen::Inventory&&game.native_mod_screen().is_none()
        &&!game.remote_menu_title().is_some_and(|t|crate::game::claim_borders_inventory_preferences::economic_title(&t.plain()))
}
fn name(game:&Game<'_>,stack:&ItemStack)->String {
    let(ns,path)=stack.item.split_once(':').unwrap_or(("minecraft",&stack.item));let key=path.replace('/',".");
    format!("{} {} {} {}",stack.item,game.translate(&format!("item.{ns}.{key}"),&path.replace('_'," ")),
        game.translate(&format!("block.{ns}.{key}"),&path.replace('_'," ")),claims::strip_formatting(&stack.custom_name().unwrap_or_default()))
}
pub(super)fn reset(){state(|s|*s=State::default());}
pub(super)fn frame(game:&mut Game<'_>){
    if !eligible(game){state(|s|{s.focus=false;s.rect=None;});return;}
    let(paste,copy)=state(|s|{let p=s.paste;s.paste=false;(p,s.copy.take())});
    if let Some(text)=copy{game.clipboard.set(&text);}
    if paste{let text=game.clipboard.get();state(|s|{if s.focus{s.text.insert(&text,96);}});}
    let query=state(|s|s.text.value.clone());let(mut found,mut total)=(0,0);
    for slot in game.slots(){if let Some(item)=game.slot_view(slot){total+=1;if input::matches(&query,&name(game,&item)){found+=1;}}}
    state(|s|{s.found=found;s.total=total;});
}
pub(super)fn decorate(game:&Game<'_>,stack:&ItemStack,x:f32,y:f32,batch:&mut HudBatch){
    if !eligible(game){return;}let query=state(|s|s.text.value.clone());
    if query.trim().is_empty()||!input::matches(&query,&name(game,stack)){return;}
    let color=0xe9000000|ui::preferences(game).accent.color();
    for r in [[x-1.,y-1.,18.,1.],[x-1.,y+16.,18.,1.],[x-1.,y,1.,16.],[x+16.,y,1.,16.]]{game.native_fill(batch,r,color);}
}
pub(super)fn render(game:&Game<'_>,batch:&mut HudBatch,screen:&NativeScreenView<'_>){
    if !eligible(game){return;}let Some(rect)=input::search_rect(screen.rect,[batch.width,batch.height])else{return;};
    state(|s|{
        s.rect=Some(rect);ui::button_base(game,batch,rect,"inventory-search",s.focus,screen.mouse);
        ui::icon(game,batch,ui::Icon::Eye,[rect[0]+5.,rect[1]+4.,9.,9.],ui::preferences(game).accent.color());
        let has=!s.text.value.is_empty();let clear=if has{17.0}else{0.0};
        let count=if rect[2]>=150.&&has{format!("{} / {}",s.found,s.total)}else{String::new()};
        let extra=if count.is_empty(){0.0}else{game.native_text_width(&count)+6.};
        let width=(rect[2]-22.-clear-extra).max(4.);
        let mut shown=if has{s.text.value.clone()}else{"Поиск предмета".into()};
        if s.focus{shown=s.text.value[..s.text.cursor].to_owned()+"│"+&s.text.value[s.text.cursor..];}
        while game.native_text_width(&shown)>width&&shown.chars().count()>1{let n=shown.chars().next().unwrap().len_utf8();shown.drain(..n);}
        ui::text(game,batch,&shown,[rect[0]+18.,rect[1]+5.],if has||s.focus{0xe3ebf3}else{0x9aacc0},false);
        if !count.is_empty(){ui::text(game,batch,&count,[rect[0]+rect[2]-clear-extra,rect[1]+5.],0x9aacc0,false);}
        if has{ui::button(game,batch,[rect[0]+rect[2]-16.,rect[1]+1.,15.,15.],"X",false,screen.mouse);}
        if crate::game::claim_borders_ui_style::contains(rect,screen.mouse)&&!s.focus{
            ui::tooltip(game,batch,&format!("Поиск по названию и ID · Ctrl+F · Найдено {} из {} слотов",s.found,s.total),screen.mouse);
        }
    });
}
pub(super)fn mouse(game:&Game<'_>,screen:&NativeScreenView<'_>,button:i32,canceled:&mut bool)->bool{
    if !eligible(game){return false;}
    state(|s|{
        if s.rect.is_some_and(|r|crate::game::claim_borders_ui_style::contains(r,screen.mouse)){
            *canceled=true;s.pressed.insert(button);
            if button==0{let r=s.rect.unwrap();if !s.text.value.is_empty()&&screen.mouse[0]>=r[0]+r[2]-17.{s.text=input::TextInput::default();}s.focus=true;}
            true
        }else{s.focus=false;false}
    })
}
pub(super)fn release(button:i32,canceled:&mut bool)->bool{
    state(|s|{if s.pressed.remove(&button){*canceled=true;true}else{false}})
}
pub(super)fn key(game:&Game<'_>,key:&str,canceled:&mut bool)->bool{
    if !eligible(game){return false;}
    state(|s|{
        if key=="key.keyboard.f"&&game.modifiers.control_key(){s.focus=true;s.text.anchor=Some(0);s.text.cursor=s.text.value.len();*canceled=true;return true;}
        if !s.focus{return false;}*canceled=true;
        if matches!(key,"key.keyboard.escape"|"key.keyboard.enter"){s.focus=false;return true;}
        match s.text.key(key,game.modifiers.control_key(),game.modifiers.shift_key()){
            input::Edit::Copy(t)=>s.copy=Some(t),input::Edit::Paste=>s.paste=true,_=>{}
        }true
    })
}
pub(super)fn character(game:&Game<'_>,ch:char,canceled:&mut bool)->bool{
    if !eligible(game){return false;}
    state(|s|{if !s.focus{return false;}*canceled=true;if !game.modifiers.control_key(){s.text.insert(&ch.to_string(),96);}true})
}
