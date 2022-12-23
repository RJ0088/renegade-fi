use core::time::Duration;
use create2;
use futures::{executor::block_on, StreamExt};
use hex;
use ring_channel::RingSender;
use std::{env, str::FromStr, thread};
use web3::{
    self, ethabi,
    signing::keccak256,
    types::{BlockId, BlockNumber, H160, H256, U256},
    Web3,
};

use crate::{exchanges::connection::get_current_time, reporter::PriceReport, tokens::Token};

#[derive(Clone, Debug)]
pub struct UniswapV3Handler;
impl UniswapV3Handler {
    const FACTORY_ADDRESS: &str = "1f98431c8ad98523631ae4a59f267346ea31f984";
    const POOL_INIT_CODE_HASH: &str =
        "e34f199b19b2b4f47f68442619d555527d244f78a3297ea89325f843f87b8b54";

    pub fn start_price_stream(
        base_token: Token,
        quote_token: Token,
        mut sender: RingSender<PriceReport>,
    ) {
        // Create the Web3 connection.
        let ethereum_wss_url = env::var("ETHEREUM_MAINNET_WSS").unwrap();
        let transport = block_on(web3::transports::WebSocket::new(&ethereum_wss_url)).unwrap();
        let web3_connection = Web3::new(transport);

        // Derive the Uniswap pool address from this Token pair.
        let pool_address = Self::get_pool_address(base_token, quote_token).unwrap();

        // Create a filter for Uniswap `Swap` events on this pool.
        let swap_event_abi = ethabi::Event {
            name: String::from("Swap"),
            inputs: vec![
                ethabi::EventParam {
                    name: String::from("sender"),
                    kind: ethabi::param_type::ParamType::Address,
                    indexed: true,
                },
                ethabi::EventParam {
                    name: String::from("recipient"),
                    kind: ethabi::param_type::ParamType::Address,
                    indexed: true,
                },
                ethabi::EventParam {
                    name: String::from("amount0"),
                    kind: ethabi::param_type::ParamType::Int(256),
                    indexed: false,
                },
                ethabi::EventParam {
                    name: String::from("amount1"),
                    kind: ethabi::param_type::ParamType::Int(256),
                    indexed: false,
                },
                ethabi::EventParam {
                    name: String::from("sqrtPriceX96"),
                    kind: ethabi::param_type::ParamType::Uint(160),
                    indexed: false,
                },
                ethabi::EventParam {
                    name: String::from("liquidity"),
                    kind: ethabi::param_type::ParamType::Uint(128),
                    indexed: false,
                },
                ethabi::EventParam {
                    name: String::from("tick"),
                    kind: ethabi::param_type::ParamType::Int(24),
                    indexed: false,
                },
            ],
            anonymous: false,
        };
        let swap_topic_filter = swap_event_abi
            .filter(ethabi::RawTopicFilter::default())
            .unwrap();
        let swap_filter = web3::types::FilterBuilder::default()
            .address(vec![pool_address])
            .topic_filter(swap_topic_filter)
            .build();
        let swap_filter =
            block_on(web3_connection.eth_filter().create_logs_filter(swap_filter)).unwrap();

        thread::spawn(move || {
            let swap_stream = swap_filter.stream(Duration::new(1, 0));
            futures::pin_mut!(swap_stream);
            loop {
                let swap = block_on(swap_stream.next()).unwrap().unwrap();
                let block_id = BlockId::Number(BlockNumber::Number(swap.block_number.unwrap()));
                let block_timestamp = block_on(web3_connection.eth().block(block_id))
                    .unwrap()
                    .unwrap()
                    .timestamp;
                let swap = swap_event_abi
                    .parse_log(ethabi::RawLog {
                        topics: swap.topics.clone(),
                        data: swap.data.clone().0,
                    })
                    .unwrap();
                let price_report = Self::handle_event(swap);
                if let Some(mut price_report) = price_report {
                    price_report.local_timestamp = get_current_time();
                    price_report.reported_timestamp = Some(block_timestamp.as_u128());
                    sender.send(price_report).unwrap();
                }
            }
        });
    }

    fn handle_event(swap: ethabi::Log) -> Option<PriceReport> {
        // Extract the `sqrtPriceX96` and convert it to the marginal price of the Uniswapv3 pool,
        // as per: https://docs.uniswap.org/sdk/v3/guides/fetching-prices#understanding-sqrtprice
        let sqrt_price_x96 = &swap.params[4].value;
        let sqrt_price_x96 = match sqrt_price_x96 {
            ethabi::Token::Uint(sqrt_price_x96) => sqrt_price_x96,
            _ => unreachable!(),
        };
        let price_numerator = U256::from(2).pow(U256::from(192));
        let price_denominator = U256::from(sqrt_price_x96).pow(U256::from(2));
        // The best way to convert U256 to f64 is unfortunately to parse via Strings. Big L.
        let price_numerator: f64 = price_numerator.to_string().parse().unwrap();
        let price_denominator: f64 = price_denominator.to_string().parse().unwrap();
        // Note that this price does not adjust for ERC-20 decimals yet.
        let price = price_numerator / price_denominator;
        Some(PriceReport {
            midpoint_price: price as f64,
            reported_timestamp: None,
            local_timestamp: Default::default(),
        })
    }

    fn get_pool_address(base_token: Token, quote_token: Token) -> Option<H160> {
        let base_token_addr = H160::from_str(base_token.get_addr()).unwrap();
        let quote_token_addr = H160::from_str(quote_token.get_addr()).unwrap();
        let (first_token, second_token) = if base_token_addr > quote_token_addr {
            (quote_token_addr, base_token_addr)
        } else {
            (base_token_addr, quote_token_addr)
        };
        let mut fee = [0_u8; 32];
        // Fee tiers;
        // HIGH = 10000
        // MEDIUM = 3000
        // LOW = 500
        // LOWEST = 100
        // TODO: Dynamically choose the fee tier?
        fee[32 - 4..].clone_from_slice(&500_u32.to_be_bytes());

        let pool_address = create2::calc_addr_with_hash(
            hex::decode(Self::FACTORY_ADDRESS).unwrap()[..20]
                .try_into()
                .unwrap(),
            &keccak256(
                &[
                    H256::from(first_token).as_bytes(),
                    H256::from(second_token).as_bytes(),
                    &fee,
                ]
                .concat()[..],
            ),
            hex::decode(Self::POOL_INIT_CODE_HASH).unwrap()[..32]
                .try_into()
                .unwrap(),
        );
        Some(H160::from(pool_address))
    }
}
