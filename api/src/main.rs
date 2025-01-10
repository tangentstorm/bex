use warp::Filter;
use dotenv::dotenv;
use std::env;
use lazy_static::lazy_static;
use std::sync::Mutex;
use bex::bdd::BddBase;
use bex::nid::NID;
use bex::base::Base;

lazy_static! {
    pub static ref BDD_BASE: Mutex<BddBase> = Mutex::new(BddBase::new());
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    let host = env::var("HOST").unwrap_or_else(|_| "127.0.0.1".to_string());
    let port = env::var("PORT").unwrap_or_else(|_| "3030".to_string()).parse().expect("PORT must be a number");

    let version = env!("CARGO_PKG_VERSION");
    let hello = warp::path::end().map(move || format!("bex-api version: {}", version));

    let vhl = warp::path!("ite" / NID / NID / NID)
        .map(|vid: NID, nid1: NID, nid2: NID| {
            let mut bdd_base = BDD_BASE.lock().unwrap();
            let new_nid = bdd_base.ite(vid, nid1, nid2);
            format!("{new_nid}")});

    let xor = warp::path!("xor" / NID / NID)
        .map(|nid1: NID, nid2: NID| {
            let mut bdd_base = BDD_BASE.lock().unwrap();
            let new_nid = bdd_base.xor(nid1, nid2);
            format!("{new_nid}")});

    let and = warp::path!("and" / NID / NID)
        .map(|nid1: NID, nid2: NID| {
            let mut bdd_base = BDD_BASE.lock().unwrap();
            let new_nid = bdd_base.and(nid1, nid2);
            format!("{new_nid}")});

    let or = warp::path!("or" / NID / NID)
        .map(|nid1: NID, nid2: NID| {
            let mut bdd_base = BDD_BASE.lock().unwrap();
            let new_nid = bdd_base.or(nid1, nid2);
            format!("{new_nid}")});

    let nid = warp::path!("nid" / NID)
            .map(|nid: NID| {
                if nid.is_lit() || nid.is_const() || nid.is_fun() { format!("{nid}") }
                else {
                    let bdd_base = BDD_BASE.lock().unwrap();
                    let (v, hi, lo) = bdd_base.get_vhl(nid);
                    format!("v: {v} hi: {hi} lo: {lo}") }});

    let routes = hello.or(vhl).or(xor).or(and).or(or).or(nid);

    let addr = (host.parse::<std::net::IpAddr>().expect("HOST must be a valid IP address"), port);

    println!("Server listening on http://{}:{}", host, port);

    warp::serve(routes).run(addr).await;
}
