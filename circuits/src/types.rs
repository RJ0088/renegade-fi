
use ark_bn254::{Fr as Bn254Fr};
use ark_ff::PrimeField;
use ark_r1cs_std::{prelude::AllocVar, R1CSVar, uint64::UInt64, uint8::UInt8};
use ark_relations::r1cs::{SynthesisError, Namespace};
use ark_sponge::{poseidon::PoseidonSponge, CryptographicSponge};
use num_bigint::BigUint;
use std::borrow::Borrow;

use crate::constants::{MAX_BALANCES, MAX_ORDERS};
use crate::gadgets::poseidon::PoseidonSpongeWrapperVar;

/**
 * Groups types definitions common to the circuit module
 */

// The scalar field used in the circuits
pub type SystemField = Bn254Fr;

// Represents a wallet and its analog in the constraint system
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Wallet {
    pub balances: Vec<Balance>,
    pub orders: Vec<Order>
}

impl Wallet {
    // Poseidon hash of the wallet
    pub fn hash(&self) -> BigUint {
        // Convert wallet to a vector of u64
        let mut hash_input = Vec::<u64>::new();
        for balance in self.balances.iter() {
            hash_input.append(&mut vec![balance.amount, balance.mint])
        }

        // Append empty balances up to MAX_BALANCES
        for _ in 0..(MAX_BALANCES - self.balances.len()) {
            hash_input.append(&mut vec![0, 0])
        }

        for order in self.orders.iter() {
            hash_input.append(&mut vec![order.base_mint, order.quote_mint, order.side.clone() as u64, order.price, order.amount]);
        }

        // Append empty orders up to MAX_ORDERS
        for _ in 0..(MAX_ORDERS - self.orders.len()) {
            hash_input.append(&mut vec![0, 0, 0, 0, 0])
        }

        let mut sponge = PoseidonSponge::<SystemField>::new(&PoseidonSpongeWrapperVar::default_params());
        for input in hash_input.iter() {
            sponge.absorb(input)
        }

        let sponge_out = sponge.squeeze_field_elements::<SystemField>(1)[0];

        // Convert to BigUInt
        sponge_out.into()
 
    }

    // Poseidon hash of the orders only 
    pub fn hash_orders(&self) -> BigUint {
        // Convert wallet to a vector of u64
        let mut hash_input = Vec::<u64>::new();
        for order in self.orders.iter() {
            hash_input.append(&mut vec![order.base_mint, order.quote_mint, order.side.clone() as u64, order.price, order.amount]);
        }

        // Append empty orders up to MAX_ORDERS
        for _ in 0..(MAX_ORDERS - self.orders.len()) {
            hash_input.append(&mut vec![0, 0, 0, 0, 0])
        }

        let mut sponge = PoseidonSponge::<SystemField>::new(&PoseidonSpongeWrapperVar::default_params());
        for input in hash_input.iter() {
            sponge.absorb(input)
        }

        let sponge_out = sponge.squeeze_field_elements::<SystemField>(1)[0];

        // Convert to BigUInt
        sponge_out.into()
    }
}

#[derive(Debug)]
pub struct WalletVar<F: PrimeField> {
    pub balances: Vec<BalanceVar<F>>,
    pub orders: Vec<OrderVar<F>>
}

impl<F: PrimeField> AllocVar<Wallet, F> for WalletVar<F> {
    // Allocates a new variable in the given CS
    fn new_variable<T: Borrow<Wallet>>(
        cs: impl Into<Namespace<F>>,
        f: impl FnOnce() -> Result<T, SynthesisError>,
        mode: ark_r1cs_std::prelude::AllocationMode,
    ) -> Result<Self, SynthesisError> {

        // Map each balance into a constraint variable
        f().and_then(|wallet| {
            let cs = cs.into();
            let wallet: &Wallet = wallet.borrow();
            let mut balances: Vec<BalanceVar<F>> = wallet.balances
                .iter()
                .map(|balance| {
                    BalanceVar::new_variable(cs.clone(), || Ok(balance), mode)
                })
                .collect::<Result<Vec<BalanceVar<F>>, SynthesisError>>()?;

            // Pad to the size of MAX_BALANCES with empty balances
            for _ in 0..(MAX_BALANCES - wallet.balances.len()) {
                balances.push(
                    BalanceVar::new_variable(cs.clone(), || Ok(Balance::default()), mode)?
                )
            }
            
            let mut orders: Vec<OrderVar<F>> = wallet.orders
                .iter()
                .map(|order| {
                    OrderVar::new_variable(cs.clone(), || Ok(order), mode)
                })
                .collect::<Result<Vec<OrderVar<F>>, SynthesisError>>()?;
            
            // Pad to the size of MAX_ORDERS with empty orders
            for _ in 0..(MAX_ORDERS - wallet.orders.len()) {
                orders.push(
                    OrderVar::new_variable(cs.clone(), || Ok(Order::default()), mode)?
                )
            }

            Ok(Self { balances, orders })
        }) 
    }
}

impl<F: PrimeField> R1CSVar<F> for WalletVar<F> {
    type Value = Wallet;

    fn cs(&self) -> ark_relations::r1cs::ConstraintSystemRef<F> {
        self.balances.cs()
    }

    fn is_constant(&self) -> bool {
        self.balances.is_constant()
    }

    fn value(&self) -> Result<Self::Value, SynthesisError> {
        let balances = self.balances
            .iter()
            .map(|balance| {
                balance.value()
            })
            .collect::<Result<Vec<Balance>, SynthesisError>>()?;
        
        let orders = self.orders
            .iter()
            .map(|order| order.value())
            .collect::<Result<Vec<Order>, SynthesisError>>()?;
        
        Ok(Self::Value { balances, orders })
    }
}

// Represents a balance tuple and its analog in the constraint system
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Balance {
    pub mint: u64,
    pub amount: u64 
}

#[derive(Debug)]
pub struct BalanceVar<F: PrimeField> {
    pub mint: UInt64<F>,
    pub amount: UInt64<F>
}

impl<F: PrimeField> AllocVar<Balance, F> for BalanceVar<F> {
    fn new_variable<T: Borrow<Balance>>(
        cs: impl Into<Namespace<F>>,
        f: impl FnOnce() -> Result<T, SynthesisError>,
        mode: ark_r1cs_std::prelude::AllocationMode,
    ) -> Result<Self, SynthesisError> {
        f().and_then(|balance| {
            let cs = cs.into();
            let mint = UInt64::new_variable(
                cs.clone(), 
                || Ok(balance.borrow().mint), 
                mode
            )?;

            let amount = UInt64::new_variable(
                cs, 
                || Ok(balance.borrow().amount), 
                mode
            )?;

            Ok(Self { mint, amount })
        })
    }
}

impl<F: PrimeField> R1CSVar<F> for BalanceVar<F> {
    type Value = Balance;

    fn cs(&self) -> ark_relations::r1cs::ConstraintSystemRef<F> {
        self.amount.cs()
    }

    fn is_constant(&self) -> bool {
        self.amount.is_constant()
    }

    fn value(&self) -> Result<Self::Value, ark_relations::r1cs::SynthesisError> {
        Ok(
            Balance {
                mint: self.mint.value()?,
                amount: self.amount.value()?
            }
        )
    }
}

// Represents an order and its analog in the consraint system
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Order {
    pub quote_mint: u64,
    pub base_mint: u64,
    pub side: OrderSide,
    pub price: u64,
    pub amount: u64
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OrderSide {
    Buy = 0,
    Sell 
}

// Default for an empty order is buy
impl Default for OrderSide {
    fn default() -> Self {
        OrderSide::Buy
    }
}

impl From<OrderSide> for u64 {
    fn from(order_side: OrderSide) -> Self {
        u8::from(order_side) as u64
    }
}

impl From<OrderSide> for u8 {
    fn from(order_side: OrderSide) -> Self {
        match order_side {
            OrderSide::Buy => { 0 }
            OrderSide::Sell => { 1 }
        }
    }
}

#[derive(Debug)]
pub struct OrderVar<F: PrimeField> {
    pub quote_mint: UInt64<F>,
    pub base_mint: UInt64<F>,
    pub side: UInt8<F>,
    pub price: UInt64<F>,
    pub amount: UInt64<F>,
}

impl<F: PrimeField> AllocVar<Order, F> for OrderVar<F> {
    fn new_variable<T: Borrow<Order>>(
        cs: impl Into<Namespace<F>>,
        f: impl FnOnce() -> Result<T, SynthesisError>,
        mode: ark_r1cs_std::prelude::AllocationMode,
    ) -> Result<Self, SynthesisError> {
        f().and_then(|order| {
            let cs = cs.into();
            let quote_mint = UInt64::new_variable(
                cs.clone(), 
                || Ok(order.borrow().quote_mint), 
                mode
            )?;

            let base_mint = UInt64::new_variable(
                cs.clone(),
                || Ok(order.borrow().base_mint), 
                mode
            )?;

            let side = UInt8::new_variable(
                cs.clone(), 
                || {
                    match &order.borrow().side {
                        OrderSide::Buy => { Ok(0) },
                        OrderSide::Sell => { Ok(1) }
                    }
                }, 
                mode
            )?;

            let price = UInt64::new_variable(
                cs.clone(), 
                || Ok(order.borrow().price), 
                mode
            )?;

            let amount = UInt64::new_variable(
                cs, 
                || Ok(order.borrow().amount), 
                mode
            )?;

            Ok(OrderVar { quote_mint, base_mint, side, price, amount })
        })
    }
}

impl<F: PrimeField> R1CSVar<F> for OrderVar<F> {
    type Value = Order;

    fn cs(&self) -> ark_relations::r1cs::ConstraintSystemRef<F> {
        self.amount.cs()
    }

    fn is_constant(&self) -> bool {
        self.amount.is_constant()
    }

    fn value(&self) -> Result<Self::Value, SynthesisError> {
        Ok(
            Order { 
                quote_mint: self.quote_mint.value()?,
                base_mint: self.base_mint.value()?,
                side: match self.side.value()? {
                    0 => { Ok(OrderSide::Buy) },
                    1 => { Ok(OrderSide::Sell) }
                    _ => { Err(SynthesisError::Unsatisfiable) }
                }?,
                price: self.price.value()?,
                amount: self.price.value()?
            }
        )
    }
}

// The result of a matches operation and its constraint system analog
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchResult {
    pub matches1: Vec<Match>,
    pub matches2: Vec<Match>
}

#[derive(Clone, Debug)]
pub struct MatchResultVariable<F: PrimeField> {
    pub matches1: Vec<MatchVariable<F>>,
    pub matches2: Vec<MatchVariable<F>>
}

impl<F: PrimeField> MatchResultVariable<F> {
    pub fn new() -> Self {
        Self { matches1: Vec::new(), matches2: Vec::new() }
    } 
}

impl<F: PrimeField> Default for MatchResultVariable<F> {
    fn default() -> Self {
        Self::new()
    }
}

impl<F: PrimeField> R1CSVar<F> for MatchResultVariable<F> {
    type Value = MatchResult;

    fn cs(&self) -> ark_relations::r1cs::ConstraintSystemRef<F> {
        self.matches1[0].cs()
    } 

    fn is_constant(&self) -> bool {
        self.matches1[0].is_constant()
    }

    fn value(&self) -> Result<Self::Value, SynthesisError> {
        let matches1 = self.matches1
            .iter()
            .map(|match_var| match_var.value())
            .collect::<Result<Vec<Match>, SynthesisError>>()?;
        
        let matches2 = self.matches2
            .iter()
            .map(|match_var| match_var.value())
            .collect::<Result<Vec<Match>, SynthesisError>>()?;
        
        Ok ( MatchResult { matches1, matches2 } )
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Match {
    pub mint: u64,
    pub amount: u64,
    pub side: OrderSide
}

#[derive(Debug, Clone)]
pub struct MatchVariable<F: PrimeField> {
    pub mint: UInt64<F>,
    pub amount: UInt64<F>,
    pub side: UInt8<F>
}

impl<F: PrimeField> R1CSVar<F> for MatchVariable<F> {
    type Value = Match;

    fn cs(&self) -> ark_relations::r1cs::ConstraintSystemRef<F> {
        self.mint.cs()
    }

    fn is_constant(&self) -> bool {
        self.mint.is_constant()
    }

    fn value(&self) -> Result<Self::Value, SynthesisError> {
        Ok(
            Match {
                mint: self.mint.value()?,
                amount: self.amount.value()?,
                side: match self.side.value()? {
                    0 => { Ok(OrderSide::Buy) },
                    1 => { Ok(OrderSide::Sell) },
                    _ => { Err(SynthesisError::Unsatisfiable) }
                }?
            }
        )
    }
}
