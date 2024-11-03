use serde::{Serialize, Deserialize};
use serde_json::{Value, Result};

#[derive(Serialize, Deserialize, Debug)]
pub enum MarketSide {
    Bid,
    Ask,
    Unknown, // Should never happen according to API docs
}

impl MarketSide {
    pub fn from_string(message: &str) -> MarketSide {
        return match message {
            "ask" => MarketSide::Ask,
            "bid" => MarketSide::Bid,
            _ => MarketSide::Unknown,
        }
    }
}

#[derive(Serialize, Deserialize)]
pub enum MessageType {
    Trade,
    Change,
    Unknown, // Should never happen according to API docs
}

impl MessageType {
    pub fn from_string(message: &str) -> Self {
        return match message {
            "trade" => Self::Trade,
            "change" => Self::Change,
            _ => Self::Unknown
        }
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Quote {
    pub price: f64,
    pub reason: String,
    pub remaining: f64,
    pub side: MarketSide,
    pub delta: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Trade {
    pub price: f64,
    pub amount: f64,
    pub maker_side: MarketSide,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum Event {
    Trade(Trade),
    Quote(Quote),
    Unknown,
}

#[derive(Serialize, Deserialize, Debug)]
pub struct MarketMessage {
    pub event_id: u64,
    pub events: Vec<Event>,
    pub timestamp: Option<u64>,
    pub timestampms: Option<u64>,
    pub socket_sequence: u32,
}

impl Event {
    pub fn new(message: &[u8]) -> MarketMessage {
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
}

#[derive(Serialize, Deserialize, Debug)]
pub struct BestBidOffer {
    pub best_bid: f64,
    pub best_offer: f64,
    pub bid_amount_remaining: f64,
    pub ask_amount_remaining: f64,
}

impl BestBidOffer {
    pub fn new() -> Self {
        Self {
            best_bid: 0.,
            best_offer: 0.,
            bid_amount_remaining: 0.,
            ask_amount_remaining: 0.,
        }
    }
}
