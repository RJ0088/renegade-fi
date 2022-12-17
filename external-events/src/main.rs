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
    let mut median_reporter = PriceReporter::new(Token::ETH, Token::USDC, None).unwrap();
    let mut binance_reporter =
        PriceReporter::new(Token::ETH, Token::USDC, Some(vec![Exchange::Binance])).unwrap();
    let mut coinbase_reporter =
        PriceReporter::new(Token::ETH, Token::USDC, Some(vec![Exchange::Coinbase])).unwrap();
    let mut kraken_reporter =
        PriceReporter::new(Token::ETH, Token::USDC, Some(vec![Exchange::Kraken])).unwrap();
    loop {
        let median_midpoint = median_reporter.get_current_report().unwrap().midpoint_price;
        let binance_midpoint = binance_reporter
            .get_current_report()
            .unwrap()
            .midpoint_price;
        let coinbase_midpoint = coinbase_reporter
            .get_current_report()
            .unwrap()
            .midpoint_price;
        let kraken_midpoint = kraken_reporter.get_current_report().unwrap().midpoint_price;
        println!(
            "M = {:.4} (B = {:.4}, C = {:.4}, K = {:.4})",
            median_midpoint, binance_midpoint, coinbase_midpoint, kraken_midpoint,
        );
        thread::sleep(time::Duration::from_millis(100));
    }
}
