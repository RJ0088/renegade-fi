//! Defines the VALID COMMITMENTS circuit which proves knowledge of a balance
//! order and fee inside of a wallet that can be matched against
//!
//! A node in the relayer network will prove this statement for each order and
//! use it as part of the handshake process
//!
//! See the whitepaper (https://renegade.fi/whitepaper.pdf) appendix A.3
//! for a formal specification

use crypto::fields::scalar_to_biguint;
use curve25519_dalek::{ristretto::CompressedRistretto, scalar::Scalar};
use itertools::Itertools;
use mpc_bulletproof::{
    r1cs::{
        ConstraintSystem, LinearCombination, Prover, R1CSProof, RandomizableConstraintSystem,
        Variable, Verifier,
    },
    r1cs_mpc::R1CSError,
    BulletproofGens,
};
use rand_core::OsRng;

use crate::{
    errors::{ProverError, VerifierError},
    types::{
        balance::{Balance, BalanceVar, CommittedBalance},
        fee::{CommittedFee, Fee, FeeVar},
        order::{CommittedOrder, Order, OrderVar},
        wallet::{CommittedWallet, Wallet, WalletVar},
    },
    zk_gadgets::{
        commitments::{NullifierGadget, WalletCommitGadget},
        comparators::{EqVecGadget, EqZeroGadget, GreaterThanEqGadget},
        merkle::PoseidonMerkleHashGadget,
        select::CondSelectGadget,
    },
    CommitProver, CommitVerifier, SingleProverCircuit,
};

/// The circuitry for the VALID COMMITMENTS statement
#[derive(Clone, Debug)]
pub struct ValidCommitments<
    const MAX_BALANCES: usize,
    const MAX_ORDERS: usize,
    const MAX_FEES: usize,
> {}

impl<const MAX_BALANCES: usize, const MAX_ORDERS: usize, const MAX_FEES: usize>
    ValidCommitments<MAX_BALANCES, MAX_ORDERS, MAX_FEES>
where
    [(); MAX_BALANCES + MAX_ORDERS + MAX_FEES]: Sized,
{
    /// Apply the constraints for the VALID COMMITMENTS circuitry
    pub fn circuit<CS: RandomizableConstraintSystem>(
        witness: ValidCommitmentsWitnessVar<MAX_BALANCES, MAX_ORDERS, MAX_FEES>,
        merkle_root: Variable,
        match_nullifier: Variable,
        cs: &mut CS,
    ) -> Result<(), R1CSError> {
        // Compute the wallet commitment
        let wallet_commitment = WalletCommitGadget::wallet_commit(&witness.wallet, cs)?;

        // Verify the opening of the commitment to the Merkle root
        PoseidonMerkleHashGadget::compute_and_constrain_root_prehashed(
            wallet_commitment.clone(),
            witness.wallet_opening,
            witness.wallet_opening_indices,
            merkle_root.into(),
            cs,
        )?;

        // Compute the wallet match nullifier and constrain it to the expected value
        let match_nullifier_res =
            NullifierGadget::match_nullifier(witness.wallet.randomness, wallet_commitment, cs)?;
        cs.constrain(match_nullifier - match_nullifier_res);

        // Verify that the given balance, order, and fee are all valid members of the wallet
        Self::verify_wallet_contains_balance(witness.balance, &witness.wallet, cs);
        Self::verify_wallet_contains_balance(witness.fee_balance, &witness.wallet, cs);
        Self::verify_wallet_contains_order(witness.order, &witness.wallet, cs);
        Self::verify_wallet_contains_fee(witness.fee, &witness.wallet, cs);

        // Verify that the balance is for the correct mint
        let selected_mint = CondSelectGadget::select(
            witness.order.base_mint,
            witness.order.quote_mint,
            witness.order.side,
            cs,
        );
        cs.constrain(witness.balance.mint - selected_mint);

        // Verify that the given fee balance is the same mint as the committed fee
        cs.constrain(witness.fee.gas_addr - witness.fee_balance.mint);
        // Constrain the given fee balance to be larger than the fixed fee
        GreaterThanEqGadget::<64 /* bitlength */>::constrain_greater_than_eq(
            witness.fee_balance.amount,
            witness.fee.gas_token_amount,
            cs,
        );

        Ok(())
    }

    /// Verify that a given balance is in the list of the wallet's balances
    fn verify_wallet_contains_balance<CS: RandomizableConstraintSystem>(
        balance: BalanceVar,
        wallet: &WalletVar<MAX_BALANCES, MAX_ORDERS, MAX_FEES>,
        cs: &mut CS,
    ) {
        // Accumulate the boolean results of comparing the given order against the
        // orders in the wallet
        let mut balances_equal_sum: LinearCombination = Variable::Zero().into();
        for wallet_balance in wallet.balances.iter() {
            let b1_vars: Vec<Variable> = balance.into();
            let b2_vars: Vec<Variable> = (*wallet_balance).into();

            balances_equal_sum += EqVecGadget::eq_vec(&b1_vars, &b2_vars, cs);
        }

        // Constrain the EqZero gadget to return 0; i.e. the given order matches a wallet order
        let sum_eq_zero = EqZeroGadget::eq_zero(balances_equal_sum, cs);
        cs.constrain(sum_eq_zero.into());
    }

    /// Verify that a given order is in the list of the wallet's orders
    fn verify_wallet_contains_order<CS: RandomizableConstraintSystem>(
        order: OrderVar,
        wallet: &WalletVar<MAX_BALANCES, MAX_ORDERS, MAX_FEES>,
        cs: &mut CS,
    ) {
        // Accumulate the boolean results of comparing the given order against the
        // orders in the wallet
        let mut orders_equal_sum: LinearCombination = Variable::Zero().into();
        for wallet_order in wallet.orders.iter() {
            let o1_vars: Vec<Variable> = order.into();
            let o2_vars: Vec<Variable> = (*wallet_order).into();

            orders_equal_sum += EqVecGadget::eq_vec(&o1_vars, &o2_vars, cs);
        }

        // Constrain the EqZero gadget to return 0; i.e. the given order matches a wallet order
        let sum_eq_zero = EqZeroGadget::eq_zero(orders_equal_sum, cs);
        cs.constrain(sum_eq_zero.into());
    }

    /// Verify that a given fee is in the list of the wallet's fees
    fn verify_wallet_contains_fee<CS: RandomizableConstraintSystem>(
        fee: FeeVar,
        wallet: &WalletVar<MAX_BALANCES, MAX_ORDERS, MAX_FEES>,
        cs: &mut CS,
    ) {
        // Accumulate the boolean results of comparing the given order against the
        // orders in the wallet
        let mut fees_equal_sum: LinearCombination = Variable::Zero().into();
        for wallet_fee in wallet.fees.iter() {
            let f1_vars: Vec<Variable> = fee.into();
            let f2_vars: Vec<Variable> = (*wallet_fee).into();

            fees_equal_sum += EqVecGadget::eq_vec(&f1_vars, &f2_vars, cs);
        }

        // Constrain the EqZero gadget to return 0; i.e. the given order matches a wallet order
        let sum_eq_zero = EqZeroGadget::eq_zero(fees_equal_sum, cs);
        cs.constrain(sum_eq_zero.into());
    }
}

/// The witness type for VALID COMMITMENTS
#[derive(Clone, Debug)]
pub struct ValidCommitmentsWitness<
    const MAX_BALANCES: usize,
    const MAX_ORDERS: usize,
    const MAX_FEES: usize,
> where
    [(); MAX_BALANCES + MAX_ORDERS + MAX_FEES]: Sized,
{
    /// The wallet that the committed values come from
    pub wallet: Wallet<MAX_BALANCES, MAX_ORDERS, MAX_FEES>,
    /// The selected order to commit to
    pub order: Order,
    /// The selected balance to commit to
    pub balance: Balance,
    /// The balance used to pay out constant fees in
    pub fee_balance: Balance,
    /// The selected fee to commit to
    pub fee: Fee,
    /// The merkle proof that the wallet is valid within the state tree
    pub wallet_opening: Vec<Scalar>,
    /// The indices of the merkle proof that the wallet is valid
    pub wallet_opening_indices: Vec<Scalar>,
}

/// The witness type for VALID COMMITMENTS, allocated in a constraint system
#[derive(Clone, Debug)]
pub struct ValidCommitmentsWitnessVar<
    const MAX_BALANCES: usize,
    const MAX_ORDERS: usize,
    const MAX_FEES: usize,
> where
    [(); MAX_BALANCES + MAX_ORDERS + MAX_FEES]: Sized,
{
    /// The wallet that the committed values come from
    pub wallet: WalletVar<MAX_BALANCES, MAX_ORDERS, MAX_FEES>,
    /// The selected order to commit to
    pub order: OrderVar,
    /// The selected balance to commit to
    pub balance: BalanceVar,
    /// The balance used to pay out constant fees in
    pub fee_balance: BalanceVar,
    /// The selected fee to commit to
    pub fee: FeeVar,
    /// The merkle proof that the wallet is valid within the state tree
    pub wallet_opening: Vec<Variable>,
    /// The indices of the merkle proof that the wallet is valid
    pub wallet_opening_indices: Vec<Variable>,
}

/// The witness type for VALID COMMITMENTS, committed to by a prover
#[derive(Clone, Debug)]
pub struct ValidCommitmentsWitnessCommitment<
    const MAX_BALANCES: usize,
    const MAX_ORDERS: usize,
    const MAX_FEES: usize,
> where
    [(); MAX_BALANCES + MAX_ORDERS + MAX_FEES]: Sized,
{
    /// The wallet that the committed values come from
    pub wallet: CommittedWallet<MAX_BALANCES, MAX_ORDERS, MAX_FEES>,
    /// The selected order to commit to
    pub order: CommittedOrder,
    /// The selected balance to commit to
    pub balance: CommittedBalance,
    /// The balance used to pay out constant fees in
    pub fee_balance: CommittedBalance,
    /// The selected fee to commit to
    pub fee: CommittedFee,
    /// The merkle proof that the wallet is valid within the state tree
    pub wallet_opening: Vec<CompressedRistretto>,
    /// The indices of the merkle proof that the wallet is valid
    pub wallet_opening_indices: Vec<CompressedRistretto>,
}

impl<const MAX_BALANCES: usize, const MAX_ORDERS: usize, const MAX_FEES: usize> CommitProver
    for ValidCommitmentsWitness<MAX_BALANCES, MAX_ORDERS, MAX_FEES>
where
    [(); MAX_BALANCES + MAX_ORDERS + MAX_FEES]: Sized,
{
    type VarType = ValidCommitmentsWitnessVar<MAX_BALANCES, MAX_ORDERS, MAX_FEES>;
    type CommitType = ValidCommitmentsWitnessCommitment<MAX_BALANCES, MAX_ORDERS, MAX_FEES>;
    type ErrorType = ();

    fn commit_prover<R: rand_core::RngCore + rand_core::CryptoRng>(
        &self,
        rng: &mut R,
        prover: &mut Prover,
    ) -> Result<(Self::VarType, Self::CommitType), Self::ErrorType> {
        // Commit to the variables individually
        let (wallet_var, wallet_commit) = self.wallet.commit_prover(rng, prover).unwrap();
        let (order_var, order_commit) = self.order.commit_prover(rng, prover).unwrap();
        let (balance_var, balance_commit) = self.balance.commit_prover(rng, prover).unwrap();
        let (fee_balance_var, fee_balance_comm) =
            self.fee_balance.commit_prover(rng, prover).unwrap();
        let (fee_var, fee_commit) = self.fee.commit_prover(rng, prover).unwrap();

        // Commit to the Merkle proof
        let (merkle_opening_comms, merkle_opening_vars): (Vec<CompressedRistretto>, Vec<Variable>) =
            self.wallet_opening
                .iter()
                .map(|opening_elem| prover.commit(*opening_elem, Scalar::random(rng)))
                .unzip();
        let (merkle_index_comms, merkle_index_vars): (Vec<CompressedRistretto>, Vec<Variable>) =
            self.wallet_opening_indices
                .iter()
                .map(|opening_index| prover.commit(*opening_index, Scalar::random(rng)))
                .unzip();

        Ok((
            ValidCommitmentsWitnessVar {
                wallet: wallet_var,
                order: order_var,
                balance: balance_var,
                fee: fee_var,
                fee_balance: fee_balance_var,
                wallet_opening: merkle_opening_vars,
                wallet_opening_indices: merkle_index_vars,
            },
            ValidCommitmentsWitnessCommitment {
                wallet: wallet_commit,
                order: order_commit,
                balance: balance_commit,
                fee: fee_commit,
                fee_balance: fee_balance_comm,
                wallet_opening: merkle_opening_comms,
                wallet_opening_indices: merkle_index_comms,
            },
        ))
    }
}

impl<const MAX_BALANCES: usize, const MAX_ORDERS: usize, const MAX_FEES: usize> CommitVerifier
    for ValidCommitmentsWitnessCommitment<MAX_BALANCES, MAX_ORDERS, MAX_FEES>
where
    [(); MAX_BALANCES + MAX_ORDERS + MAX_FEES]: Sized,
{
    type VarType = ValidCommitmentsWitnessVar<MAX_BALANCES, MAX_ORDERS, MAX_FEES>;
    type ErrorType = ();

    fn commit_verifier(&self, verifier: &mut Verifier) -> Result<Self::VarType, Self::ErrorType> {
        let wallet_var = self.wallet.commit_verifier(verifier).unwrap();
        let order_var = self.order.commit_verifier(verifier).unwrap();
        let balance_var = self.balance.commit_verifier(verifier).unwrap();
        let fee_balance_var = self.fee_balance.commit_verifier(verifier).unwrap();
        let fee_var = self.fee.commit_verifier(verifier).unwrap();

        let merkle_opening_vars = self
            .wallet_opening
            .iter()
            .map(|opening_val| verifier.commit(*opening_val))
            .collect_vec();
        let merkle_index_vars = self
            .wallet_opening_indices
            .iter()
            .map(|opening_indices| verifier.commit(*opening_indices))
            .collect_vec();

        Ok(ValidCommitmentsWitnessVar {
            wallet: wallet_var,
            order: order_var,
            balance: balance_var,
            fee_balance: fee_balance_var,
            fee: fee_var,
            wallet_opening: merkle_opening_vars,
            wallet_opening_indices: merkle_index_vars,
        })
    }
}

/// The statement type for VALID COMMITMENTS
#[derive(Clone, Debug)]
pub struct ValidCommitmentsStatement {
    /// The wallet match nullifier of the wallet committed to
    pub nullifier: Scalar,
    /// The global merkle root being proved against
    pub merkle_root: Scalar,
}

impl<const MAX_BALANCES: usize, const MAX_ORDERS: usize, const MAX_FEES: usize> SingleProverCircuit
    for ValidCommitments<MAX_BALANCES, MAX_ORDERS, MAX_FEES>
where
    [(); MAX_BALANCES + MAX_ORDERS + MAX_FEES]: Sized,
{
    type Witness = ValidCommitmentsWitness<MAX_BALANCES, MAX_ORDERS, MAX_FEES>;
    type WitnessCommitment = ValidCommitmentsWitnessCommitment<MAX_BALANCES, MAX_ORDERS, MAX_FEES>;
    type Statement = ValidCommitmentsStatement;

    const BP_GENS_CAPACITY: usize = 32768;

    fn prove(
        witness: Self::Witness,
        statement: Self::Statement,
        mut prover: Prover,
    ) -> Result<(Self::WitnessCommitment, R1CSProof), ProverError> {
        // Commit to the witness
        let mut rng = OsRng {};
        let (witness_var, witness_commit) = witness.commit_prover(&mut rng, &mut prover).unwrap();

        let nullifier_var = prover.commit_public(statement.nullifier);
        let merkle_root_var = prover.commit_public(statement.merkle_root);

        // Apply the constraints
        ValidCommitments::circuit(witness_var, merkle_root_var, nullifier_var, &mut prover)
            .map_err(ProverError::R1CS)?;

        // Prove the statement
        let bp_gens = BulletproofGens::new(Self::BP_GENS_CAPACITY, 1 /* party_capacity */);
        let proof = prover.prove(&bp_gens).map_err(ProverError::R1CS)?;

        Ok((witness_commit, proof))
    }

    fn verify(
        witness_commitment: Self::WitnessCommitment,
        statement: Self::Statement,
        proof: R1CSProof,
        mut verifier: Verifier,
    ) -> Result<(), VerifierError> {
        // Commit to the witness
        let witness_var = witness_commitment.commit_verifier(&mut verifier).unwrap();

        let nullifier_var = verifier.commit_public(statement.nullifier);
        let merkle_root_var = verifier.commit_public(statement.merkle_root);

        // Apply the constraints
        ValidCommitments::circuit(witness_var, merkle_root_var, nullifier_var, &mut verifier)
            .map_err(VerifierError::R1CS)?;

        // Verify the proof
        let bp_gens = BulletproofGens::new(Self::BP_GENS_CAPACITY, 1 /* party_capacity */);
        verifier
            .verify(&proof, &bp_gens)
            .map_err(VerifierError::R1CS)
    }
}

#[cfg(test)]
mod valid_commitments_test {
    use crypto::fields::prime_field_to_scalar;
    use merlin::Transcript;
    use mpc_bulletproof::{
        r1cs::{ConstraintSystem, Prover},
        PedersenGens,
    };
    use num_bigint::BigUint;
    use rand_core::{OsRng, RngCore};

    use crate::{
        test_helpers::bulletproof_prove_and_verify,
        types::{
            balance::Balance,
            order::{Order, OrderSide},
        },
        zk_circuits::test_helpers::{
            compute_wallet_commitment, compute_wallet_match_nullifier, create_wallet_opening,
            SizedWallet, INITIAL_WALLET, MAX_BALANCES, MAX_FEES, MAX_ORDERS,
        },
        CommitProver,
    };

    use super::{ValidCommitments, ValidCommitmentsStatement, ValidCommitmentsWitness};

    const MERKLE_HEIGHT: usize = 3;

    // -----------
    // | Helpers |
    // -----------

    /// Checks whether the given witness and statement satisfy the circuit, without proving or verifying
    fn constraints_satisfied(
        witness: ValidCommitmentsWitness<MAX_BALANCES, MAX_ORDERS, MAX_FEES>,
        statement: ValidCommitmentsStatement,
    ) -> bool {
        // Build a prover
        let mut prover_transcript = Transcript::new("test".as_bytes());
        let pc_gens = PedersenGens::default();
        let mut prover = Prover::new(&pc_gens, &mut prover_transcript);

        // Commit to the witness
        let mut rng = OsRng {};
        let (witness_var, _) = witness.commit_prover(&mut rng, &mut prover).unwrap();

        let nullifier_var = prover.commit_public(statement.nullifier);
        let merkle_root_var = prover.commit_public(statement.merkle_root);

        ValidCommitments::circuit(witness_var, merkle_root_var, nullifier_var, &mut prover)
            .unwrap();

        prover.constraints_satisfied()
    }

    // ---------
    // | Tests |
    // ---------

    /// Tests a valid proof of VALID COMMITMENTS
    #[test]
    fn test_valid_commitments() {
        let wallet: SizedWallet = INITIAL_WALLET.clone();
        let order = wallet.orders[0].to_owned();
        let balance = wallet.balances[0].to_owned();
        let fee_balance = wallet.balances[0].to_owned();
        let fee = wallet.fees[0].to_owned();

        // Create a merkle proof for the wallet
        let mut rng = OsRng {};
        let index = rng.next_u32() % (1 << MERKLE_HEIGHT);
        let (root, opening, opening_indices) =
            create_wallet_opening(&wallet, MERKLE_HEIGHT, index as usize, &mut rng);

        let witness = ValidCommitmentsWitness {
            wallet: wallet.clone(),
            order,
            balance,
            fee_balance,
            fee,
            wallet_opening: opening,
            wallet_opening_indices: opening_indices,
        };
        let statement = ValidCommitmentsStatement {
            nullifier: prime_field_to_scalar(&compute_wallet_match_nullifier(
                &wallet,
                compute_wallet_commitment(&wallet),
            )),
            merkle_root: root,
        };

        let res = bulletproof_prove_and_verify::<
            ValidCommitments<MAX_BALANCES, MAX_ORDERS, MAX_FEES>,
        >(witness, statement);
        assert!(res.is_ok())
    }

    /// Test the case in which the prover gives a balance that is not in the wallet
    #[test]
    fn test_invalid_balance() {
        let wallet: SizedWallet = INITIAL_WALLET.clone();
        let order = wallet.orders[0].to_owned();

        // Invalid, fake balance with a larger balance than the wallet has access to
        let balance = Balance {
            mint: 2u64,
            amount: 20u64,
        };
        let fee_balance = wallet.balances[0].to_owned();
        let fee = wallet.fees[0].to_owned();

        // Create a merkle proof for the wallet
        let mut rng = OsRng {};
        let index = rng.next_u32() % (1 << MERKLE_HEIGHT);
        let (root, opening, opening_indices) =
            create_wallet_opening(&wallet, MERKLE_HEIGHT, index as usize, &mut rng);

        let witness = ValidCommitmentsWitness {
            wallet: wallet.clone(),
            order,
            balance,
            fee_balance,
            fee,
            wallet_opening: opening,
            wallet_opening_indices: opening_indices,
        };
        let statement = ValidCommitmentsStatement {
            nullifier: prime_field_to_scalar(&compute_wallet_match_nullifier(
                &wallet,
                compute_wallet_commitment(&wallet),
            )),
            merkle_root: root,
        };

        assert!(!constraints_satisfied(witness, statement));
    }

    /// Tests the case in which the prover sends a fee balance that is not part of the wallet
    #[test]
    fn test_invalid_fee_balance() {
        let wallet: SizedWallet = INITIAL_WALLET.clone();
        let order = wallet.orders[0].to_owned();

        let balance = wallet.balances[0].to_owned();
        // Invalid, fake balance with a larger balance than the wallet has access to
        let fee_balance = Balance {
            mint: 1,
            amount: 10,
        };
        let fee = wallet.fees[0].to_owned();

        // Create a merkle proof for the wallet
        let mut rng = OsRng {};
        let index = rng.next_u32() % (1 << MERKLE_HEIGHT);
        let (root, opening, opening_indices) =
            create_wallet_opening(&wallet, MERKLE_HEIGHT, index as usize, &mut rng);

        let witness = ValidCommitmentsWitness {
            wallet: wallet.clone(),
            order,
            balance,
            fee_balance,
            fee,
            wallet_opening: opening,
            wallet_opening_indices: opening_indices,
        };
        let statement = ValidCommitmentsStatement {
            nullifier: prime_field_to_scalar(&compute_wallet_match_nullifier(
                &wallet,
                compute_wallet_commitment(&wallet),
            )),
            merkle_root: root,
        };

        assert!(!constraints_satisfied(witness, statement));
    }

    /// Tests the case in which the prover sends an invalid order, not part of the wallet
    #[test]
    fn test_invalid_order() {
        let wallet: SizedWallet = INITIAL_WALLET.clone();
        let order = Order {
            quote_mint: 1,
            base_mint: 3,
            side: OrderSide::Buy,
            price: 10,
            amount: 15,
            timestamp: 0,
        };
        let balance = wallet.balances[0].to_owned();
        let fee_balance = wallet.balances[0].to_owned();
        let fee = wallet.fees[0].to_owned();

        // Create a merkle proof for the wallet
        let mut rng = OsRng {};
        let index = rng.next_u32() % (1 << MERKLE_HEIGHT);
        let (root, opening, opening_indices) =
            create_wallet_opening(&wallet, MERKLE_HEIGHT, index as usize, &mut rng);

        let witness = ValidCommitmentsWitness {
            wallet: wallet.clone(),
            order,
            balance,
            fee_balance,
            fee,
            wallet_opening: opening,
            wallet_opening_indices: opening_indices,
        };
        let statement = ValidCommitmentsStatement {
            nullifier: prime_field_to_scalar(&compute_wallet_match_nullifier(
                &wallet,
                compute_wallet_commitment(&wallet),
            )),
            merkle_root: root,
        };

        assert!(!constraints_satisfied(witness, statement));
    }

    /// Tests with a fee that is not part of the given wallet
    #[test]
    fn test_invalid_fee() {
        let wallet: SizedWallet = INITIAL_WALLET.clone();
        let order = wallet.orders[0].to_owned();
        let balance = wallet.balances[0].to_owned();
        let fee_balance = wallet.balances[0].to_owned();

        // Invalid, prover modified the settle key
        let mut fee = wallet.fees[0].to_owned();
        fee.settle_key = BigUint::from(1729u64);

        // Create a merkle proof for the wallet
        let mut rng = OsRng {};
        let index = rng.next_u32() % (1 << MERKLE_HEIGHT);
        let (root, opening, opening_indices) =
            create_wallet_opening(&wallet, MERKLE_HEIGHT, index as usize, &mut rng);

        let witness = ValidCommitmentsWitness {
            wallet: wallet.clone(),
            order,
            balance,
            fee_balance,
            fee,
            wallet_opening: opening,
            wallet_opening_indices: opening_indices,
        };
        let statement = ValidCommitmentsStatement {
            nullifier: prime_field_to_scalar(&compute_wallet_match_nullifier(
                &wallet,
                compute_wallet_commitment(&wallet),
            )),
            merkle_root: root,
        };

        assert!(!constraints_satisfied(witness, statement));
    }
}
