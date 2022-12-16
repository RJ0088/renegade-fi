//! This crate manages all external event reporting, including 1) price feeds from centralized
//! exchanges, 2) StarkWare events, including nullifier reveals in order to hang up MPCs, and 3)
//! Ethereum events, like sequencer rotation or L1 deposits.

mod errors;
mod exchanges;
mod reporters;
mod tokens;

use dotenv::from_filename;
use std::{thread, time};

use crate::{exchanges::Exchange, reporters::PriceReporter, tokens::Token};

fn main() {
    from_filename("api_keys.env").ok();
    let mut binance_reporter =
        PriceReporter::new(Token::ETH, Token::USDC, Exchange::Binance).unwrap();
    let mut coinbase_reporter =
        PriceReporter::new(Token::ETH, Token::USDC, Exchange::Coinbase).unwrap();
    loop {
        thread::sleep(time::Duration::from_millis(100));
        let binance_midpoint = binance_reporter
            .get_current_report()
            .unwrap()
            .midpoint_price;
        let coinbase_midpoint = coinbase_reporter
            .get_current_report()
            .unwrap()
            .midpoint_price;
        println!(
            "Midpoint: Binance = {:.4}, Coinbase = {:.4}, Diff = {:.4}bp",
            binance_midpoint,
            coinbase_midpoint,
            (coinbase_midpoint - binance_midpoint) / binance_midpoint * 10_000.0,
        );
    }
}
