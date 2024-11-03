use url;
use clap::Parser;

use futures_util::{future, pin_mut, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use std::sync::{Arc, Mutex};

mod models;
use models::*;

#[derive(Parser)]
struct Cli {
    #[arg(long)]
    symbol: String,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let ws_url = format!("wss://api.gemini.com/v1/marketdata/{}?top_of_book=true", cli.symbol);
    let url = url::Url::parse(&ws_url).unwrap();

    let (ws_stream, _)  = connect_async(url).await.expect("Failed to connect");
    println!("WebSocket handshake has been completed!");

    let (write, read) = ws_stream.split();
    let bbo = Arc::new(Mutex::new(BestBidOffer::new()));
    let ws_to_stdout = {
        read.for_each(|message| async {
            let m = message.unwrap();
            if m.is_empty() {
                return;
            }
            let data = m.into_data();
            let event = Event::new(data.as_slice());
            for e in event.events {
                match e {
                    Event::Trade(t) => {
                        let dollar_amt = t.amount * t.price;
                        let msg = format!("{:?} ${}\n", t, dollar_amt);
                        tokio::io::stdout().write_all(msg.as_bytes()).await.unwrap();
                    },
                    Event::Quote(q) => {
                        match q.side {
                            MarketSide::Ask => {
                                {
                                    let mut bbo = bbo.lock().unwrap();
                                    bbo.best_offer = q.price;
                                    bbo.ask_amount_remaining = q.remaining;
                                }
                            },
                            MarketSide::Bid => {
                                {
                                    let mut bbo = bbo.lock().unwrap();
                                    bbo.best_bid = q.price;
                                    bbo.bid_amount_remaining = q.remaining;
                                }
                            },
                            MarketSide::Unknown => {},
                        }
                        {
                            let msg = format!("{:?}\n", bbo.lock().unwrap());
                            tokio::io::stdout().write_all(msg.as_bytes()).await.unwrap();
                        }
                    },
                    Event::Unknown => {},
                }
            }
        })
    };

    ws_to_stdout.await;
}
