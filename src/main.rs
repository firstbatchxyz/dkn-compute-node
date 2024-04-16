use clients::waku::WakuClient;

mod clients;
mod utils;

#[tokio::main]
async fn main() {
    let waku = WakuClient::new(None);
    // call waku.health
    let health = waku.version();
    let result = health.await.unwrap();
    println!("{:?}", result);
}
