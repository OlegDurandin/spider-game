use spider_engine::{Card, Game, Move};
use std::{
    collections::HashSet,
    hash::{Hash, Hasher},
    sync::atomic::{AtomicUsize, Ordering},
};
fn rng(x: &mut u64) -> u64 {
    *x ^= *x << 13;
    *x ^= *x >> 7;
    *x ^= *x << 17;
    *x
}
fn key(g: &Game) -> u64 {
    let mut cols: Vec<Vec<_>> = g
        .state
        .columns
        .iter()
        .map(|c| c.iter().map(|c| (c.rank(), c.open)).collect())
        .collect();
    cols.sort();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    cols.hash(&mut h);
    g.state
        .stock
        .iter()
        .map(|c| c.rank())
        .collect::<Vec<_>>()
        .hash(&mut h);
    g.state.completed.len().hash(&mut h);
    h.finish()
}
fn main() {
    let count = AtomicUsize::new(0);
    std::thread::scope(|scope| {
        for worker in 0..4 {
            let count = &count;
            scope.spawn(move||{for attempt in 0..1500 {
 if count.load(Ordering::Relaxed)>=7{return;}
 let seed=(attempt*4+worker+1) as u64;let mut random=seed*7927;let mut g=Game::new(seed);
 if attempt<104 {let offset=(attempt+worker*13)%104;let mut deck:Vec<_>=(0..104u8).map(|i|Card{id:((i as usize+offset)%104) as u8,open:false}).collect();g.state.columns=std::array::from_fn(|i|{let mut c:Vec<_>=deck.drain(..if i<4{6}else{5}).collect();c.last_mut().unwrap().open=true;c});g.state.stock=deck;}
 let initial=g.clone();let mut seen=HashSet::new();seen.insert(key(&g));let mut cmds=vec![];
 for _ in 0..450 {
  if g.state.won(){break;}
  let mut best=None;let mut bestscore=i32::MIN;
  let closed=g.state.columns.iter().flatten().filter(|c|!c.open).count();
  for from in 0..10 {for start in 0..g.state.columns[from].len(){for to in g.targets(from,start){
   let c=&g.state.columns[from];let dst=&g.state.columns[to];if start==0&&dst.is_empty(){continue;}
   let m=Move{from,start,to};let mut trial=g.clone();trial.apply(m).unwrap();trial.history.clear();if seen.contains(&key(&trial)){continue;}
   let exposes=closed-trial.state.columns.iter().flatten().filter(|c|!c.open).count();
   let joined=if dst.is_empty(){0}else{1};let broken=if start>0&&c[start-1].open&&c[start-1].rank()==c[start].rank()+1{1}else{0};
   let score=(trial.state.completed.len()-g.state.completed.len()) as i32*1200+exposes as i32*160+(if start==0{100}else{0})+(joined-broken)*65+((c.len()-start) as i32)*3+(rng(&mut random)%120) as i32;
   if score>bestscore{bestscore=score;best=Some((trial,serde_json::json!({"move":[from,start,to]})));}
  }}}
  let mut dealt=g.clone();if dealt.deal().is_ok() {dealt.history.clear();let score=35+(rng(&mut random)%100) as i32;if score>bestscore&&!seen.contains(&key(&dealt)){best=Some((dealt,serde_json::json!({"deal":true})));}}
  let Some((next,cmd))=best else{break};g=next;seen.insert(key(&g));cmds.push(cmd);
 }
 if g.state.won(){let n=count.fetch_add(1,Ordering::Relaxed);if n<7{println!("FOUND {n} seed {seed} {} moves",cmds.len());std::fs::write(format!("/tmp/spider-daily-{n}.json"),serde_json::to_string_pretty(&serde_json::json!({"columns":initial.state.columns,"stock":initial.state.stock,"solution":cmds})).unwrap()).unwrap();}}
 if attempt%100==0{eprintln!("worker {worker}: {attempt}, found {}",count.load(Ordering::Relaxed));}
}});
        }
    });
}
