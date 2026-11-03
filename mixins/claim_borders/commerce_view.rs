// Presentation of the live server menu. Transactions are revalidated against the current window.
use crate::game::{Game,Screen,claim_borders as claims,claim_borders_ui_view as ui,claim_borders_economy_client as data,claim_borders_economy_scan as scan,claim_borders_input::{TextInput,Edit}};
use horizon_client::{hud::HudBatch,clipboard::Clipboard};
use std::{sync::{Mutex,OnceLock},collections::BTreeSet};
#[derive(Clone)]
struct Pending{token:String,button:scan::Button,at:u64}
#[derive(Clone)]
enum Hit{Server(scan::Button),Confirm,Cancel,Page(i32),Search,Standard}
#[derive(Default)]
struct State{token:String,hidden:String,query:TextInput,focus:bool,page:usize,hits:Vec<([f32;4],Hit)>,pending:Option<Pending>,action:Option<Pending>,pressed:BTreeSet<i32>,notice:String,copy:Option<String>,paste:bool}
fn state<T>(f:impl FnOnce(&mut State)->T)->T{static S:OnceLock<Mutex<State>>=OnceLock::new();f(&mut S.get_or_init(||Mutex::new(State::default())).lock().unwrap_or_else(|p|p.into_inner()))}
pub(super)fn reset(){state(|s|*s=State::default());}
fn live(game:&Game<'_>)->Option<(scan::Page,String)>{
    let page=data::server_page(game)?;
    if state(|s|s.hidden==page.1){None}else{Some(page)}
}
pub(super)fn frame(game:&mut Game<'_>){
    if !game.native_on(claims::ID)||game.remote.is_none(){reset();return;}
    let (action,copy,paste)=state(|s|(s.action.take(),s.copy.take(),std::mem::take(&mut s.paste)));
    if let Some(copy)=copy{game.clipboard.set(&copy);}
    if paste{let v=game.clipboard.get();state(|s|{s.query.insert(&v,64);s.page=0;});}
    if let Some(p)=action{
        let ok=data::now().saturating_sub(p.at)<=10_000&&data::server_click(game,&p.token,&p.button);
        state(|s|s.notice=if ok{"Запрос отправлен серверу".into()}else{"Окно, цена или предмет изменились. Выберите действие заново.".into()});
    }
    if game.screen!=Screen::Inventory{state(|s|{s.pending=None;s.focus=false;s.hits.clear();s.hidden.clear();});}
}
fn nav(page:&scan::Page,b:&scan::Button)->bool{
    b.item=="minecraft:arrow"&&[45,47,51].contains(&b.slot)
    ||b.slot==53&&b.item=="minecraft:barrier"
    ||page.family==scan::Kind::Market&&b.slot<45&&b.lore.iter().any(|l|l.ends_with(" items")||l.ends_with(" предметов"))
    ||page.family==scan::Kind::Auction&&b.slot==10&&b.item=="minecraft:nether_star"&&(b.name.starts_with("All listings")||b.name.starts_with("Все лоты"))
    ||page.family==scan::Kind::Market&&b.slot<45&&b.lore.iter().any(|l|l.starts_with("Price: ")||l.starts_with("Цена: "))
}
fn caption(game:&Game<'_>,page:&scan::Page,b:&scan::Button)->String{
    let name=b.name.as_str();
    if name.starts_with("Buy ") {return name.replacen("Buy ","Купить ",1).replace(" for "," за ").replace("coins","монет");}
    if name.starts_with("Sell ") {return name.replacen("Sell ","Продать ",1).replace(" for "," за ").replace("coins","монет").replace("all","всё");}
    match name{"Next page"=>"Следующая страница".into(),"Previous page"=>"Предыдущая страница".into(),"Back"=>"Назад".into(),"Close"=>"Закрыть".into(),"Movers over 24 hours"=>"Динамика за 24 часа".into(),
        _=>if page.family==scan::Kind::Market&&b.slot<45&&b.lore.iter().any(|l|l.starts_with("Price: ")){crate::game::claim_borders_economy_view::item_label(game,&b.item)}else{name.into()}}
}
fn row(game:&Game<'_>,batch:&mut HudBatch,s:&mut State,r:[f32;4],text:&str,detail:&str,hit:Hit,mouse:[f32;2],icon:ui::Icon){
    ui::button_base(game,batch,r,text,false,mouse);
    ui::icon(game,batch,icon,[r[0]+6.0,r[1]+8.0,12.0,12.0],ui::preferences(game).accent.color());
    ui::text(game,batch,&ui::clip(game,text,r[2]-30.0),[r[0]+24.0,r[1]+5.0],0xe3ebf3,false);
    ui::text(game,batch,&ui::clip(game,detail,r[2]-30.0),[r[0]+24.0,r[1]+17.0],0x9aacc0,false);s.hits.push((r,hit));
}
pub(super)fn render(game:&Game<'_>,batch:&mut HudBatch,screen:&crate::game::native_hooks::NativeScreenView<'_>)->bool{
    let Some((page,token))=live(game)else{return false;};
    let w=(batch.width-16.0).clamp(220.0,740.0);let h=(batch.height-16.0).clamp(180.0,430.0);
    let x=(batch.width-w)*0.5;let y=(batch.height-h)*0.5;let start=batch.vertices.len();
    // Cover vanilla slots so the displayed cards and captured hit regions always agree.
    game.native_fill(batch,[0.0,0.0,batch.width,batch.height],0xff111b27);
    state(|s|{
        if s.token!=token{s.token=token.clone();s.pending=None;s.page=0;s.hits.clear();}
        s.hits.clear();
        ui::fill(game,batch,[x,y,w,h],0xee182636);
        ui::icon(game,batch,ui::Icon::Market,[x+10.0,y+9.0,14.0,14.0],ui::preferences(game).accent.color());
        ui::text(game,batch,if page.family==scan::Kind::Market{"MARKET · Серверный рынок"}else{"AH · Аукцион"},[x+32.0,y+12.0],0xe3ebf3,false);
        let query=[x+8.0,y+32.0,w-16.0,18.0];
        ui::button(game,batch,query,&format!("Поиск: {}{}",s.query.value,if s.focus{"│"}else{""}),s.focus,screen.mouse);s.hits.push((query,Hit::Search));
        let columns=if w>=560.0{3}else if w>=300.0{2}else{1};
        let rows=(((h-122.0)/32.0).floor() as usize).max(1);let per=columns*rows;
        let entries:Vec<_>=page.buttons.iter().filter(|b|crate::game::claim_borders_input::matches(&s.query.value,&format!("{} {} {}",caption(game,&page,b),b.item,b.lore.join(" ")))).collect();
        let pages=entries.len().div_ceil(per).max(1);s.page=s.page.min(pages-1);
        let cw=(w-16.0)/columns as f32;
        for(i,b)in entries.iter().skip(s.page*per).take(per).enumerate(){
            let r=[x+8.0+(i%columns) as f32*cw,y+57.0+(i/columns) as f32*32.0,cw-4.0,29.0];
            let detail=b.lore.iter().find(|l|l.starts_with("Price: ")||l.starts_with("Цена: ")).or_else(||b.lore.first()).map(String::as_str).unwrap_or(if nav(&page,b){"Открыть"}else{"Нажмите для просмотра"});
            row(game,batch,s,r,&caption(game,&page,b),detail,Hit::Server((*b).clone()),screen.mouse,if nav(&page,b){ui::Icon::Next}else{ui::Icon::Market});
        }
        let bottom=y+h-57.0;
        ui::button(game,batch,[x+8.0,bottom,28.0,18.0],"←",false,screen.mouse);s.hits.push(([x+8.0,bottom,28.0,18.0],Hit::Page(-1)));
        ui::text(game,batch,&format!("{} / {} · {} элементов",s.page+1,pages,entries.len()),[x+42.0,bottom+5.0],0x9aacc0,false);
        ui::button(game,batch,[x+w-36.0,bottom,28.0,18.0],"→",false,screen.mouse);s.hits.push(([x+w-36.0,bottom,28.0,18.0],Hit::Page(1)));
        let status=data::read(|book,_,status,_|if status.scanning{format!("Синхронизация · {} страниц · нажатие остановит обход",status.pages)}else{format!("Баланс {} · Лимит продаж {} / {}",book.balance.map_or("—".into(),|(v,_)|crate::game::claim_borders_economy::fmt(v)),book.coin_left.map_or("—".into(),|(v,_)|crate::game::claim_borders_economy::fmt(v)),book.coin_cap.map_or("—".into(),crate::game::claim_borders_economy::fmt))});
        ui::text(game,batch,&ui::clip(game,if s.notice.is_empty(){&status}else{&s.notice},w-16.0),[x+8.0,y+h-31.0],0xb8cdce,false);
        let raw=[x+8.0,y+h-19.0,w-16.0,15.0];ui::button(game,batch,raw,"Стандартный вид · F8",false,screen.mouse);s.hits.push((raw,Hit::Standard));
        if let Some(p)=s.pending.clone(){
            s.hits.clear();
            let mw=(w-16.0).min(440.0);let mh=(h-16.0).min(250.0);let mx=x+(w-mw)*0.5;let my=y+(h-mh)*0.5;
            game.native_fill(batch,[mx,my,mw,mh],0xff1c2b3c);
            ui::text(game,batch,"Проверьте действие и серверную цену",[mx+10.0,my+10.0],0xf0d68a,false);
            ui::text(game,batch,&ui::clip(game,&caption(game,&page,&p.button),mw-20.0),[mx+10.0,my+27.0],0xe3ebf3,false);
            let max=(((mh-94.0)/12.0).floor() as usize).max(1);
            for(i,l)in p.button.lore.iter().take(max).enumerate(){ui::text(game,batch,&ui::clip(game,l,mw-20.0),[mx+10.0,my+45.0+i as f32*12.0],0xb6c7d9,false);}
            let transaction=!nav(&page,&p.button);
            let cr=[mx+10.0,my+mh-28.0,(mw-26.0)*0.5,20.0];let ok=[mx+mw*0.5+3.0,my+mh-28.0,(mw-26.0)*0.5,20.0];
            ui::button(game,batch,cr,"Отмена",false,screen.mouse);s.hits.push((cr,Hit::Cancel));
            ui::button(game,batch,ok,if transaction{"Подтвердить действие"}else{"Открыть"},false,screen.mouse);s.hits.push((ok,Hit::Confirm));
            ui::text(game,batch,"Итог и ограничения определяет сервер",[mx+10.0,my+mh-43.0],0x9aacc0,false);
        }
    });
    ui::finish(game,batch,start,"commerce","live");true
}
pub(super)fn mouse(game:&Game<'_>,screen:&crate::game::native_hooks::NativeScreenView<'_>,button:i32,pressed:bool,canceled:&mut bool)->bool{
    if !pressed&&state(|s|s.pressed.remove(&button)){*canceled=true;return true;}
    let Some((page,token))=live(game)else{return false;};*canceled=true;
    if !pressed{return true;}
    state(|s|{
        s.pressed.insert(button);if button!=0{return;}
        let hit=s.hits.iter().rev().find(|(r,_)|crate::game::claim_borders_ui_style::contains(*r,screen.mouse)).map(|(_,h)|h.clone());
        s.focus=false;
        match hit{
            Some(Hit::Server(b))=>{let p=Pending{token:token.clone(),button:b.clone(),at:data::now()};if nav(&page,&b){s.action=Some(p)}else{s.pending=Some(p)}}
            Some(Hit::Confirm)=>{s.action=s.pending.take();}
            Some(Hit::Cancel)=>s.pending=None,
            Some(Hit::Page(n))=>s.page=if n<0{s.page.saturating_sub(1)}else{s.page+1},
            Some(Hit::Search)=>s.focus=true,
            Some(Hit::Standard)=>{s.hidden=token;s.pending=None;}
            _=>{}
        }
    });true
}
pub(super)fn key(game:&Game<'_>,key:&str,canceled:&mut bool)->bool{
    if live(game).is_none(){return false;}
    if key=="key.keyboard.escape"{
        if state(|s|s.pending.take().is_some()||std::mem::take(&mut s.focus)){*canceled=true;return true;}
        return false;
    }
    if key=="key.keyboard.e"&&!state(|s|s.focus){return false;}
    *canceled=true;
    state(|s|{
        if key=="key.keyboard.f8"{s.hidden=s.token.clone();return;}
        if game.modifiers.control_key()&&key=="key.keyboard.f"{s.focus=true;return;}
        if s.focus{match s.query.key(key,game.modifiers.control_key(),game.modifiers.shift_key()){Edit::Copy(t)=>s.copy=Some(t),Edit::Paste=>s.paste=true,_=>{}}s.page=0;}
    });true
}
pub(super)fn character(game:&Game<'_>,c:char,canceled:&mut bool)->bool{
    if live(game).is_none(){return false;}*canceled=true;
    if !game.modifiers.control_key(){state(|s|if s.focus{s.query.insert(&c.to_string(),64);s.page=0;});}true
}
pub(super)fn scroll(game:&Game<'_>,amount:f64,canceled:&mut bool)->bool{
    if live(game).is_none(){return false;}*canceled=true;state(|s|if s.pending.is_none(){s.page=if amount>0.0{s.page.saturating_sub(1)}else{s.page+1};});true
}
