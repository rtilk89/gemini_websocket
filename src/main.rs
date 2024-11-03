use url;
use clap::Parser;

use serde::{Serialize, Deserialize};
use serde_json::{Value, Result};
use futures_util::{future, pin_mut, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio_tungstenite::{connect_async, tungstenite::protocol::Message};
use std::sync::{Arc, Mutex};

#[derive(Parser)]
struct Cli {
    #[arg(long)]
    symbol: String,
}

#[derive(Serialize, Deserialize, Debug)]
enum MarketSide {
    Bid,
    Ask,
    Unknown, // Should never happen according to API docs
}

impl MarketSide {
    fn from_string(message: &str) -> MarketSide {
        return match message {
            "ask" => MarketSide::Ask,
            "bid" => MarketSide::Bid,
            _ => MarketSide::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize)]
enum MessageType {
    Trade,
    Change,
    Unknown, // Should never happen according to API docs
}

impl MessageType {
    fn from_string(message: &str) -> Self {
        return match message {
            "trade" => Self::Trade,
            "change" => Self::Change,
            _ => Self::Unknown
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct Quote {
    price: f64,
    reason: String,
    remaining: f64,
    side: MarketSide,
    delta: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug)]
struct Trade {
    price: f64,
    amount: f64,
    maker_side: MarketSide,
}

#[derive(Serialize, Deserialize, Debug)]
enum Event {
    Trade(Trade),
    Quote(Quote),
    Unknown,
}

#[derive(Serialize, Deserialize, Debug)]
struct MarketMessage {
    event_id: u64,
    events: Vec<Event>,
    timestamp: Option<u64>,
    timestampms: Option<u64>,
    socket_sequence: u32,
}

impl Event {
    fn new(message: &[u8]) -> MarketMessage {
        let m: Value = serde_json::from_slice(message).expect("Something went wrong with deseralization");

        let events: Vec<_> = m["events"].as_array().unwrap().iter()
        .map(|e| {
            let price = e["price"].as_str().unwrap().parse::<f64>().unwrap();
            match MessageType::from_string(e["type"].as_str().unwrap()) {
                MessageType::Change => {
                    let q = Quote {
                        price,
                        reason: match e["reason"].as_str() {
                            Some(n) => n.to_string(),
                            None => String::from(""),
                        },
                        remaining: match e["remaining"].as_str() {
                            Some(n) => n.parse::<f64>().unwrap(),
                            None => 0.,
                        },
                        side: match e["side"].as_str() {
                            Some(n) => MarketSide::from_string(n),
                            None => MarketSide::Unknown,
                        },
                        delta: match e["delta"].as_str() {
                            Some(n) => Some(n.parse::<f64>().unwrap()),
                            None => None,
                        }
                    };
                    Event::Quote(q)
                },
                MessageType::Trade => {
                    let t = Trade {
                        price,
                        amount: e["amount"].as_str().unwrap().parse::<f64>().unwrap(),
                        maker_side: match e["makerSide"].as_str() {
                            Some(n) => MarketSide::from_string(n),
                            None => MarketSide::Unknown,
                        }
                    };
                    Event::Trade(t)
                }
                MessageType::Unknown => Event::Unknown,
            }
        }).collect();

        MarketMessage {
            event_id: m["eventId"].as_u64().unwrap(),
            events,
            timestamp: m["timestamp"].as_u64(),
            timestampms: m["timestampms"].as_u64(),
            socket_sequence: m["socket_sequence"].as_u64().unwrap() as u32,
        }
    }

    fn as_str(&self) -> String {
        serde_json::to_string(&self).unwrap()
    }
}

#[derive(Serialize, Deserialize, Debug)]
struct BestBidOffer {
    best_bid: f64,
    best_offer: f64,
    bid_amount_remaining: f64,
    ask_amount_remaining: f64,
}

impl BestBidOffer {
    fn new() -> Self {
        Self {
            best_bid: 0.,
            best_offer: 0.,
            bid_amount_remaining: 0.,
            ask_amount_remaining: 0.,
        }
    }
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
