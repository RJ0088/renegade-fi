//! Groups gadget definitions for non-native field arithmetic

use std::iter;

use crypto::fields::{biguint_to_scalar, scalar_to_biguint};
use curve25519_dalek::scalar::Scalar;
use itertools::Itertools;
use lazy_static::lazy_static;
use mpc_bulletproof::r1cs::{LinearCombination, RandomizableConstraintSystem, Variable};
use num_bigint::BigUint;

/// The number of bits in each word, we use 126 to ensure that
/// multiplications in the base field (dalek `Scalar`s) will not
/// overflow
const WORD_SIZE: usize = 126;

lazy_static! {
    static ref BIGINT_ZERO: BigUint = BigUint::from(0u8);
    static ref BIGINT_2_TO_WORD_SIZE: BigUint = BigUint::from(1u8) << 126;
    static ref BIGINT_WORD_MASK: BigUint = &*BIGINT_2_TO_WORD_SIZE - 1u8;
}

/// Returns the maximum number of words needed to represent an element from
/// a field of the given modulus
fn repr_word_width(modulus: &BigUint) -> usize {
    let word_size_u64 = WORD_SIZE as u64;
    if modulus.bits() % word_size_u64 == 0 {
        (modulus.bits() / word_size_u64) as usize
    } else {
        (modulus.bits() / word_size_u64) as usize + 1
    }
}

/// Reduce the given value to the size of a single word, returning the
/// quotient and remainder
///
/// It is assumed that the value is less than two words in size, so that
/// we can properly constrain the modulus. This check is asserted for
fn div_rem_word<L, CS>(val: L, modulus: &BigUint, cs: &mut CS) -> (Variable, Variable)
where
    L: Into<LinearCombination>,
    CS: RandomizableConstraintSystem,
{
    // Evaluate the underlying linear combination to get a bigint that we can operate on
    let val_lc = val.into();
    let val_bigint = scalar_to_biguint(&cs.eval(&val_lc));

    assert!(
        val_bigint.bits() <= (2 * WORD_SIZE) as u64,
        "value too large for div_rem_word"
    );

    let div_bigint = &val_bigint / modulus;
    let rem_bigint = &val_bigint % modulus;

    let div_var = cs.allocate(Some(biguint_to_scalar(&div_bigint))).unwrap();
    let rem_var = cs.allocate(Some(biguint_to_scalar(&rem_bigint))).unwrap();

    let mod_scalar = biguint_to_scalar(modulus);

    // Constrain the modulus to be correct, i.e. dividend = quotient * divisor + remainder
    cs.constrain(val_lc - (mod_scalar * div_var + rem_var));
    (div_var, rem_var)
}

/// Convert a `BigUint` to a list of scalar words
fn bigint_to_scalar_words(mut val: BigUint) -> Vec<Scalar> {
    let mut words = Vec::new();
    while val.gt(&BIGINT_ZERO) {
        // Compute the next word and shift the input
        let next_word = biguint_to_scalar(&(&val & &*BIGINT_WORD_MASK));
        words.push(next_word);
        val >>= WORD_SIZE;
    }

    words
}

/// Represents an element of a non-native field that has
/// been allocated in a constraint system
///
/// We model the underlying field element as a series of `Scalar`
/// values, denoted "words". The word width here is 126 bits; a
/// Scalar is natively capable of performing arithmetic on elements
/// of F_p where p is slightly larger than 2^252, so using the
/// first 126 bits ensures that arithmetic does not overflow in the
/// base field
#[derive(Clone, Debug)]
pub struct NonNativeElementVar {
    /// The words representing the underlying field
    /// stored in little endian order
    pub(super) words: Vec<Variable>,
    /// The prime-power modulus of the field
    pub(super) field_mod: BigUint,
}

impl NonNativeElementVar {
    /// Create a new value given a set of pre-allocated words
    pub fn new(mut words: Vec<Variable>, field_mod: BigUint) -> Self {
        let field_words = repr_word_width(&field_mod);
        if field_words > words.len() {
            words.append(&mut vec![Variable::Zero(); field_words - words.len()]);
        }
        Self { words, field_mod }
    }

    /// Create a new value from a given bigint
    pub fn from_bigint<CS: RandomizableConstraintSystem>(
        mut value: BigUint,
        field_mod: BigUint,
        cs: &mut CS,
    ) -> Self {
        // Ensure that the value is in the field
        value %= &field_mod;

        // Split into words
        let field_words = repr_word_width(&field_mod);
        let mut words = Vec::with_capacity(field_words);
        for _ in 0..field_words {
            // Allocate the next 126 bits in the constraint system
            let next_word = biguint_to_scalar(&(&value & &*BIGINT_WORD_MASK));
            let word_var = cs.allocate(Some(next_word)).unwrap();
            words.push(word_var);

            value >>= WORD_SIZE;
        }

        Self { words, field_mod }
    }

    /// Construct a `NonNativeElementVar` from a bigint without reducing modulo the
    /// field modulus
    ///
    /// Here, `word_width` is the number of words that should be used to represent the
    /// resulting allocated non-native field element.
    pub fn from_bigint_unreduced<CS: RandomizableConstraintSystem>(
        value: BigUint,
        word_width: usize,
        field_mod: BigUint,
        cs: &mut CS,
    ) -> Self {
        // Ensure that the allocated word width is large enough for the underlying value
        assert!(
            repr_word_width(&value) <= word_width,
            "specified word width too narrow {:?} < {:?}",
            word_width,
            repr_word_width(&value)
        );

        let mut words = bigint_to_scalar_words(value);
        words.append(&mut vec![Scalar::zero(); word_width - words.len()]);

        let allocated_words = words
            .iter()
            .map(|word| cs.allocate(Some(*word)).unwrap())
            .collect_vec();

        Self {
            words: allocated_words,
            field_mod,
        }
    }

    /// Evalute the non-native variable in the given constraint system, and return the
    /// result as a bigint
    pub fn as_bigint<CS: RandomizableConstraintSystem>(&self, cs: &CS) -> BigUint {
        let mut res = BigUint::from(0u8);
        for word in self.words.iter().rev().cloned() {
            // Evaluate the underlying scalar representation of the word
            let word_bigint = scalar_to_biguint(&cs.eval(&word.into()));
            res = (res << WORD_SIZE) + word_bigint
        }

        res
    }

    /// Constrain two non-native field elements to equal one another
    pub fn constrain_equal<CS: RandomizableConstraintSystem>(lhs: &Self, rhs: &Self, cs: &mut CS) {
        // Pad the inputs to both be of the length of the longer input
        let max_len = lhs.words.len().max(rhs.words.len());
        let left_hand_words = lhs.words.iter().chain(iter::repeat(&Variable::Zero()));
        let right_hand_words = rhs.words.iter().chain(iter::repeat(&Variable::Zero()));

        // Compare each word in the non-native element
        for (lhs_word, rhs_word) in left_hand_words.zip(right_hand_words).take(max_len) {
            cs.constrain(*lhs_word - *rhs_word);
        }
    }

    /// Reduce the given element modulo its field
    pub fn reduce<CS: RandomizableConstraintSystem>(&mut self, cs: &mut CS) {
        // Convert to bigint for reduction
        let self_bigint = self.as_bigint(cs);
        let div_bigint = &self_bigint / &self.field_mod;
        let mod_bigint = &self_bigint % &self.field_mod;

        // Explicitly compute the representation width of the division result in the constraint system
        // We do this because the value is taken unreduced; so that verifier cannot infer the width
        // from the field modulus, and does not have access to the underlying value to determine its
        // width otherwise
        let field_modulus_word_width = repr_word_width(&self.field_mod);
        let div_word_width = self.words.len() + 1 - field_modulus_word_width;

        let div_nonnative = NonNativeElementVar::from_bigint_unreduced(
            div_bigint,
            div_word_width,
            self.field_mod.clone(),
            cs,
        );

        let mod_nonnative =
            NonNativeElementVar::from_bigint(mod_bigint, self.field_mod.clone(), cs);

        // Constrain the values to be a correct modulus
        let div_mod_mul = Self::mul_bigint_unreduced(&div_nonnative, &self.field_mod, cs);
        let reconstructed = Self::add_unreduced(&div_mod_mul, &mod_nonnative, cs);

        Self::constrain_equal(self, &reconstructed, cs);

        // Finally, update self to the correct modulus
        self.words = mod_nonnative.words;
    }

    /// Add together two non-native field elements
    pub fn add<CS: RandomizableConstraintSystem>(lhs: &Self, rhs: &Self, cs: &mut CS) -> Self {
        let mut new_elem = Self::add_unreduced(lhs, rhs, cs);
        new_elem.reduce(cs);

        new_elem
    }

    /// Add together two non-native field elements without reducing the sum
    fn add_unreduced<CS: RandomizableConstraintSystem>(
        lhs: &Self,
        rhs: &Self,
        cs: &mut CS,
    ) -> Self {
        // Ensure that both non-native elements are of the same field
        assert_eq!(
            lhs.field_mod, rhs.field_mod,
            "elements from different fields"
        );

        // Pad both left and right hand side to the same length
        let max_word_width = lhs.words.len().max(rhs.words.len());
        let lhs_word_iter = lhs.words.iter().chain(iter::repeat(&Variable::Zero()));
        let rhs_word_iter = rhs.words.iter().chain(iter::repeat(&Variable::Zero()));

        // Add word by word with carry
        let mut carry = Variable::Zero();
        let mut new_words = Vec::with_capacity(max_word_width + 1);
        for (lhs_word, rhs_word) in lhs_word_iter.zip(rhs_word_iter).take(max_word_width) {
            // Compute the word-wise sum and reduce to fit into a single word
            let word_res = *lhs_word + *rhs_word + carry;
            let div_rem = div_rem_word(word_res.clone(), &BIGINT_2_TO_WORD_SIZE, cs);

            carry = div_rem.0;
            new_words.push(div_rem.1);
        }
        new_words.push(carry);

        // Collect this into a new non-native element and reduce it
        NonNativeElementVar {
            words: new_words,
            field_mod: lhs.field_mod.clone(),
        }
    }

    /// Add together a non-native field element and a bigint
    pub fn add_bigint<CS: RandomizableConstraintSystem>(
        lhs: &Self,
        rhs: &BigUint,
        cs: &mut CS,
    ) -> Self {
        let mut res = Self::add_bigint_unreduced(lhs, rhs, cs);
        res.reduce(cs);
        res
    }

    /// Add together a non-native field element and a bigint without reducing the sum
    fn add_bigint_unreduced<CS: RandomizableConstraintSystem>(
        lhs: &Self,
        rhs: &BigUint,
        cs: &mut CS,
    ) -> Self {
        // Convert the rhs to a list of words
        let rhs_words = bigint_to_scalar_words(rhs.clone());

        // Resize the lhs and rhs word iterators to be of equal size
        let max_len = rhs_words.len().max(lhs.words.len());
        let lhs_word_iterator = lhs
            .words
            .iter()
            .cloned()
            .chain(iter::repeat(Variable::Zero()));
        let rhs_word_iterator = rhs_words
            .iter()
            .cloned()
            .chain(iter::repeat(Scalar::zero()));

        // Add the two non-native elements word-wise
        let mut carry = Variable::Zero();
        let mut new_words = Vec::with_capacity(max_len + 1);
        for (lhs_word, rhs_word) in lhs_word_iterator.zip(rhs_word_iterator).take(max_len) {
            let word_res = lhs_word + rhs_word + carry;
            let div_rem = div_rem_word(word_res, &BIGINT_2_TO_WORD_SIZE, cs);

            new_words.push(div_rem.1);
            carry = div_rem.0;
        }
        new_words.push(carry);

        Self {
            words: new_words,
            field_mod: lhs.field_mod.clone(),
        }
    }

    /// Multiply together two non-native field elements
    pub fn mul<CS: RandomizableConstraintSystem>(lhs: &Self, rhs: &Self, cs: &mut CS) -> Self {
        let mut res = Self::mul_unreduced(lhs, rhs, cs);
        res.reduce(cs);
        res
    }

    /// Multiply together two non-native field elements without reducing the product
    fn mul_unreduced<CS: RandomizableConstraintSystem>(
        lhs: &Self,
        rhs: &Self,
        cs: &mut CS,
    ) -> Self {
        assert_eq!(
            lhs.field_mod, rhs.field_mod,
            "elements from different fields"
        );
        let n_result_words = lhs.words.len() + rhs.words.len();

        // Both lhs and rhs are represented as:
        //  x = x_1 + 2^WORD_SIZE * x_2 + ... + 2^(num_words * WORD_SIZE) * x_num_words
        // To multiply the values, we take the direct product of each pair of terms
        // between `lhs` and `rhs`, storing both the term and the carry from reducing
        // each term in one of the buckets below; depending on the shift (2^k) applied
        // to the result
        //
        // The maximum shift is 2^{2 * num_words} as (2^k - 1)(2^k - 1) = 2^2k - 2^{k+1} - 1 < 2^2k
        let mut terms = vec![Vec::new(); n_result_words];
        let mut carries = vec![Vec::new(); n_result_words + 1];

        for (lhs_index, lhs_word) in lhs.words.iter().enumerate() {
            for (rhs_index, rhs_word) in rhs.words.iter().enumerate() {
                // Compute the term and reduce it modulo the field
                let (_, _, term_direct_product) =
                    cs.multiply((*lhs_word).into(), (*rhs_word).into());
                let (term_carry, term) =
                    div_rem_word(term_direct_product, &BIGINT_2_TO_WORD_SIZE, cs);

                // Place the term and the carry in the shift bin corresponding to the value k such that
                // this term is prefixed with 2^k in the expanded representation described above
                let shift_index = lhs_index + rhs_index;
                terms[shift_index].push(term);
                carries[shift_index + 1].push(term_carry);
            }
        }

        // Now reduce each term into a single word
        let mut carry = Variable::Zero();
        let mut res_words = Vec::with_capacity(n_result_words);
        for word_index in 0..n_result_words {
            // Sum all the terms and carries at the given word index
            let mut summed_word: LinearCombination = carry.into();
            for word_term in terms[word_index].iter().chain(carries[word_index].iter()) {
                summed_word += *word_term;
            }

            // Reduce this sum and add any carry to the next term's carries
            let div_rem_res = div_rem_word(summed_word, &BIGINT_2_TO_WORD_SIZE, cs);
            carry = div_rem_res.0;
            res_words.push(div_rem_res.1);
        }
        res_words.push(carry);

        Self {
            words: res_words,
            field_mod: lhs.field_mod.clone(),
        }
    }

    /// Multiply together a non-native field element and a bigint
    pub fn mul_bigint<CS: RandomizableConstraintSystem>(
        lhs: &Self,
        rhs: &BigUint,
        cs: &mut CS,
    ) -> Self {
        let mut res = Self::mul_bigint_unreduced(lhs, rhs, cs);
        res.reduce(cs);
        res
    }

    /// Multiply together a non-native field element and a bigint without reducing to the field modulus
    fn mul_bigint_unreduced<CS: RandomizableConstraintSystem>(
        lhs: &Self,
        rhs: &BigUint,
        cs: &mut CS,
    ) -> Self {
        // Split the BigUint into words
        let rhs_words = bigint_to_scalar_words(rhs.clone());
        let n_result_words = rhs_words.len() + lhs.words.len();

        // Both lhs and rhs are represented as:
        //  x = x_1 + 2^WORD_SIZE * x_2 + ... + 2^(num_words * WORD_SIZE) * x_num_words
        // To multiply the values, we take the direct product of each pair of terms
        // between `lhs` and `rhs`, storing both the term and the carry from reducing
        // each term in one of the buckets below; depending on the shift (2^k) applied
        // to the result
        //
        // The maximum shift is 2^{2 * num_words} as (2^k - 1)(2^k - 1) = 2^2k - 2^{k+1} - 1 < 2^2k
        let mut terms = vec![Vec::new(); n_result_words];
        let mut carries = vec![Vec::new(); n_result_words];

        for (lhs_index, lhs_word) in lhs.words.iter().enumerate() {
            for (rhs_index, rhs_word) in rhs_words.iter().enumerate() {
                // Compute the term and reduce it modulo the field
                let term_direct_product = *lhs_word * *rhs_word;
                let (term_carry, term) =
                    div_rem_word(term_direct_product, &BIGINT_2_TO_WORD_SIZE, cs);

                // Place the term and the carry in the shift bin corresponding to the value k such that
                // this term is prefixed with 2^k in the expanded representation described above
                let shift_index = lhs_index + rhs_index;
                terms[shift_index].push(term);
                carries[shift_index + 1].push(term_carry);
            }
        }

        // Now reduce each term into a single word
        let mut carry = Variable::Zero();
        let mut res_words = Vec::with_capacity(n_result_words);
        for word_index in 0..n_result_words {
            // Sum all the terms and carries at the given word index
            let mut summed_word: LinearCombination = carry.into();
            for word_term in terms[word_index].iter().chain(carries[word_index].iter()) {
                summed_word += *word_term;
            }

            // Reduce this sum and add any carry to the next term's carries
            let div_rem_res = div_rem_word(summed_word, &BIGINT_2_TO_WORD_SIZE, cs);
            carry = div_rem_res.0;
            res_words.push(div_rem_res.1);
        }

        Self {
            words: res_words,
            field_mod: lhs.field_mod.clone(),
        }
    }
}

#[cfg(test)]
mod nonnative_tests {
    use curve25519_dalek::{ristretto::CompressedRistretto, scalar::Scalar};
    use itertools::Itertools;
    use merlin::Transcript;
    use mpc_bulletproof::{
        r1cs::{Prover, R1CSProof, Variable, Verifier},
        BulletproofGens, PedersenGens,
    };
    use num_bigint::BigUint;
    use rand_core::{CryptoRng, OsRng, RngCore};

    use crate::{
        errors::{ProverError, VerifierError},
        test_helpers::bulletproof_prove_and_verify,
        CommitProver, CommitVerifier, SingleProverCircuit,
    };

    use super::{bigint_to_scalar_words, NonNativeElementVar};

    // -------------
    // | Constants |
    // -------------

    /// The seed for the prover/verifier transcripts
    const TRANSCRIPT_SEED: &str = "test";

    // -----------
    // | Helpers |
    // -----------

    /// Samples a random 512-bit bigint
    fn random_biguint<R: RngCore + CryptoRng>(rng: &mut R) -> BigUint {
        let bytes = &mut [0u8; 32];
        rng.fill_bytes(bytes);
        BigUint::from_bytes_le(bytes)
    }

    // ------------
    // | Circuits |
    // ------------

    /// A witness type for a fan-in 2, fan-out 1 operator
    #[derive(Clone, Debug)]
    pub struct FanIn2Witness {
        /// The left hand side of the operator
        lhs: BigUint,
        /// The right hand side of the operator
        rhs: BigUint,
        /// The field modulus that these operands are defined over
        field_mod: BigUint,
    }

    impl CommitProver for FanIn2Witness {
        type VarType = FanIn2WitnessVar;
        type CommitType = FanIn2WitnessCommitment;
        type ErrorType = ();

        fn commit_prover<R: RngCore + CryptoRng>(
            &self,
            rng: &mut R,
            prover: &mut Prover,
        ) -> Result<(Self::VarType, Self::CommitType), Self::ErrorType> {
            // Split the bigint into words
            let lhs_words = bigint_to_scalar_words(self.lhs.clone());
            let (lhs_comm, lhs_var): (Vec<CompressedRistretto>, Vec<Variable>) = lhs_words
                .iter()
                .map(|word| prover.commit(*word, Scalar::random(rng)))
                .unzip();

            let lhs_var = NonNativeElementVar::new(lhs_var, self.field_mod.clone());

            let rhs_words = bigint_to_scalar_words(self.rhs.clone());
            let (rhs_comm, rhs_var): (Vec<CompressedRistretto>, Vec<Variable>) = rhs_words
                .iter()
                .map(|word| prover.commit(*word, Scalar::random(rng)))
                .unzip();

            let rhs_var = NonNativeElementVar::new(rhs_var, self.field_mod.clone());

            Ok((
                FanIn2WitnessVar {
                    lhs: lhs_var,
                    rhs: rhs_var,
                },
                FanIn2WitnessCommitment {
                    lhs: lhs_comm,
                    rhs: rhs_comm,
                    field_mod: self.field_mod.clone(),
                },
            ))
        }
    }

    /// A constraint-system allocated fan-in 2 witness
    #[derive(Clone, Debug)]
    pub struct FanIn2WitnessVar {
        /// The left hand side of the operator
        lhs: NonNativeElementVar,
        /// The right hand side of the operator
        rhs: NonNativeElementVar,
    }

    /// A commitment to a fan-in 2 witness
    #[derive(Clone, Debug)]
    pub struct FanIn2WitnessCommitment {
        /// The left hand side of the operator
        lhs: Vec<CompressedRistretto>,
        /// The right hand side of the operator
        rhs: Vec<CompressedRistretto>,
        /// The modulus of the field
        field_mod: BigUint,
    }

    impl CommitVerifier for FanIn2WitnessCommitment {
        type VarType = FanIn2WitnessVar;
        type ErrorType = ();

        fn commit_verifier(
            &self,
            verifier: &mut Verifier,
        ) -> Result<Self::VarType, Self::ErrorType> {
            // Commit to the words in the lhs and rhs vars, then reform them into
            // allocated non-native field elements
            let lhs_vars = self
                .lhs
                .iter()
                .map(|comm| verifier.commit(*comm))
                .collect_vec();
            let lhs = NonNativeElementVar::new(lhs_vars, self.field_mod.clone());

            let rhs_vars = self
                .rhs
                .iter()
                .map(|comm| verifier.commit(*comm))
                .collect_vec();
            let rhs = NonNativeElementVar::new(rhs_vars, self.field_mod.clone());

            Ok(FanIn2WitnessVar { lhs, rhs })
        }
    }

    pub struct AdderCircuit {}
    impl SingleProverCircuit for AdderCircuit {
        type Witness = FanIn2Witness;
        type Statement = BigUint;
        type WitnessCommitment = FanIn2WitnessCommitment;

        const BP_GENS_CAPACITY: usize = 64;

        fn prove(
            witness: Self::Witness,
            statement: Self::Statement,
            mut prover: Prover,
        ) -> Result<(Self::WitnessCommitment, R1CSProof), ProverError> {
            // Commit to the witness
            let mut rng = OsRng {};
            let (witness_var, wintess_comm) = witness.commit_prover(&mut rng, &mut prover).unwrap();

            // Commit to the statement variable
            let expected_words = bigint_to_scalar_words(statement);
            let (_, statement_word_vars): (Vec<_>, Vec<Variable>) = expected_words
                .iter()
                .map(|word| prover.commit_public(*word))
                .unzip();
            let expected_nonnative =
                NonNativeElementVar::new(statement_word_vars, witness.field_mod);

            // Add the two witness values
            let addition_result =
                NonNativeElementVar::add(&witness_var.lhs, &witness_var.rhs, &mut prover);

            NonNativeElementVar::constrain_equal(
                &addition_result,
                &expected_nonnative,
                &mut prover,
            );

            // Prove the statement
            let bp_gens = BulletproofGens::new(Self::BP_GENS_CAPACITY, 1 /* party_capacity */);
            let proof = prover.prove(&bp_gens).map_err(ProverError::R1CS)?;

            Ok((wintess_comm, proof))
        }

        fn verify(
            witness_commitment: Self::WitnessCommitment,
            statement: Self::Statement,
            proof: R1CSProof,
            mut verifier: Verifier,
        ) -> Result<(), VerifierError> {
            // Commit to the witness
            let witness_var = witness_commitment.commit_verifier(&mut verifier).unwrap();

            // Commit to the statement variable
            let expected_words = bigint_to_scalar_words(statement);
            let statement_word_vars = expected_words
                .iter()
                .map(|word| verifier.commit_public(*word))
                .collect_vec();
            let expected_nonnative =
                NonNativeElementVar::new(statement_word_vars, witness_commitment.field_mod);

            // Add the two witness values
            let addition_result =
                NonNativeElementVar::add(&witness_var.lhs, &witness_var.rhs, &mut verifier);

            NonNativeElementVar::constrain_equal(
                &addition_result,
                &expected_nonnative,
                &mut verifier,
            );

            // Verify the proof
            let bp_gens = BulletproofGens::new(Self::BP_GENS_CAPACITY, 1 /* party_capacity */);
            verifier
                .verify(&proof, &bp_gens)
                .map_err(VerifierError::R1CS)
        }
    }

    #[derive(Clone, Debug)]
    pub struct MulCircuit {}
    impl SingleProverCircuit for MulCircuit {
        type Statement = BigUint;
        type Witness = FanIn2Witness;
        type WitnessCommitment = FanIn2WitnessCommitment;

        const BP_GENS_CAPACITY: usize = 128;

        fn prove(
            witness: Self::Witness,
            statement: Self::Statement,
            mut prover: Prover,
        ) -> Result<(Self::WitnessCommitment, R1CSProof), ProverError> {
            // Commit to the witness
            let mut rng = OsRng {};
            let (witness_var, wintess_comm) = witness.commit_prover(&mut rng, &mut prover).unwrap();

            // Commit to the statement variable
            let expected_words = bigint_to_scalar_words(statement);
            let (_, statement_word_vars): (Vec<_>, Vec<Variable>) = expected_words
                .iter()
                .map(|word| prover.commit_public(*word))
                .unzip();
            let expected_nonnative =
                NonNativeElementVar::new(statement_word_vars, witness.field_mod);

            // Add the two witness values
            let mul_result =
                NonNativeElementVar::mul(&witness_var.lhs, &witness_var.rhs, &mut prover);
            NonNativeElementVar::constrain_equal(&mul_result, &expected_nonnative, &mut prover);

            // Prove the statement
            let bp_gens = BulletproofGens::new(Self::BP_GENS_CAPACITY, 1 /* party_capacity */);
            let proof = prover.prove(&bp_gens).map_err(ProverError::R1CS)?;

            Ok((wintess_comm, proof))
        }

        fn verify(
            witness_commitment: Self::WitnessCommitment,
            statement: Self::Statement,
            proof: R1CSProof,
            mut verifier: Verifier,
        ) -> Result<(), VerifierError> {
            // Commit to the witness
            let witness_var = witness_commitment.commit_verifier(&mut verifier).unwrap();

            // Commit to the statement variable
            let expected_words = bigint_to_scalar_words(statement);
            let statement_word_vars = expected_words
                .iter()
                .map(|word| verifier.commit_public(*word))
                .collect_vec();
            let expected_nonnative =
                NonNativeElementVar::new(statement_word_vars, witness_commitment.field_mod);

            // Add the two witness values
            let mul_result =
                NonNativeElementVar::mul(&witness_var.lhs, &witness_var.rhs, &mut verifier);
            NonNativeElementVar::constrain_equal(&mul_result, &expected_nonnative, &mut verifier);

            // Verify the proof
            let bp_gens = BulletproofGens::new(Self::BP_GENS_CAPACITY, 1 /* party_capacity */);
            verifier
                .verify(&proof, &bp_gens)
                .map_err(VerifierError::R1CS)
        }
    }

    // ---------
    // | Tests |
    // ---------

    /// Tests converting to and from a biguint
    #[test]
    fn test_to_from_biguint() {
        let n_tests = 100;
        let mut rng = OsRng {};

        let mut prover_transcript = Transcript::new(TRANSCRIPT_SEED.as_bytes());
        let pc_gens = PedersenGens::default();
        let mut prover = Prover::new(&pc_gens, &mut prover_transcript);

        for _ in 0..n_tests {
            // Sample a random biguint and field modulus, convert to and from
            // non-native, and assert equality
            let random_elem = random_biguint(&mut rng);
            let random_mod = random_biguint(&mut rng);
            let expected_bigint = &random_elem % &random_mod;

            let nonnative_elem =
                NonNativeElementVar::from_bigint(random_elem, random_mod, &mut prover);
            assert_eq!(nonnative_elem.as_bigint(&prover), expected_bigint);
        }
    }

    /// Tests reducing a non-native field element modulo its field
    #[test]
    fn test_reduce() {
        let n_tests = 100;
        let mut rng = OsRng {};

        let mut prover_transcript = Transcript::new(TRANSCRIPT_SEED.as_bytes());
        let pc_gens = PedersenGens::default();
        let mut prover = Prover::new(&pc_gens, &mut prover_transcript);

        for _ in 0..n_tests {
            // Sample a random value, and a random modulus
            let random_val = random_biguint(&mut rng);
            let random_mod = random_biguint(&mut rng);

            let expected = &random_val % &random_mod;

            let words = bigint_to_scalar_words(random_val);
            let allocated_words = words
                .iter()
                .map(|word| prover.commit_public(*word).1)
                .collect_vec();

            let mut val = NonNativeElementVar::new(allocated_words, random_mod);
            val.reduce(&mut prover);

            // Evaluate the value in the constraint system and ensure it is as expected
            let reduced_val_bigint = val.as_bigint(&prover);
            assert_eq!(reduced_val_bigint, expected);
        }
    }

    /// Tests the addition functionality inside an addition circuit
    #[test]
    fn test_add_circuit() {
        let n_tests = 10;
        let mut rng = OsRng {};

        for _ in 0..n_tests {
            // Sample two random elements, compute their sum, then prover the AdderCircuit
            // statement
            let random_elem1 = random_biguint(&mut rng);
            let random_elem2 = random_biguint(&mut rng);
            let random_mod = random_biguint(&mut rng);
            let expected_bigint = (&random_elem1 + &random_elem2) % &random_mod;

            let witness = FanIn2Witness {
                lhs: random_elem1,
                rhs: random_elem2,
                field_mod: random_mod,
            };

            let statement = expected_bigint;

            // Prove and verify a valid member of the relation
            let res = bulletproof_prove_and_verify::<AdderCircuit>(witness, statement);
            assert!(res.is_ok());
        }
    }

    /// Tests adding a non-native field element with a biguint
    #[test]
    fn test_add_biguint() {
        let n_tests = 100;
        let mut rng = OsRng {};

        let mut prover_transcript = Transcript::new(TRANSCRIPT_SEED.as_bytes());
        let pc_gens = PedersenGens::default();
        let mut prover = Prover::new(&pc_gens, &mut prover_transcript);

        for _ in 0..n_tests {
            // Sample two random elements and a modulus, allocate one in the constraint
            // system and add the other directly as a biguint
            let random_elem1 = random_biguint(&mut rng);
            let random_elem2 = random_biguint(&mut rng);
            let random_mod = random_biguint(&mut rng);
            let expected_bigint = (&random_elem1 + &random_elem2) % &random_mod;

            let nonnative = NonNativeElementVar::from_bigint(random_elem1, random_mod, &mut prover);
            let res = NonNativeElementVar::add_bigint(&nonnative, &random_elem2, &mut prover);

            let res_bigint = res.as_bigint(&prover);
            assert_eq!(res_bigint, expected_bigint);
        }
    }

    /// Tests multiplying two non-native field elements together
    #[test]
    fn test_mul_circuit() {
        let n_tests = 10;
        let mut rng = OsRng {};

        for _ in 0..n_tests {
            // Sample two random elements, compute their sum, then prover the AdderCircuit
            // statement
            let random_elem1 = random_biguint(&mut rng);
            let random_elem2 = random_biguint(&mut rng);
            let random_mod = random_biguint(&mut rng);
            let expected_bigint = (&random_elem1 * &random_elem2) % &random_mod;

            let witness = FanIn2Witness {
                lhs: random_elem1,
                rhs: random_elem2,
                field_mod: random_mod,
            };

            let statement = expected_bigint;

            // Prove and verify a valid member of the relation
            let res = bulletproof_prove_and_verify::<MulCircuit>(witness, statement);
            assert!(res.is_ok());
        }
    }

    /// Tests multiplying a non-native field element with a bigint
    #[test]
    fn test_mul_bigint() {
        let n_tests = 100;
        let mut rng = OsRng {};

        let mut prover_transcript = Transcript::new(TRANSCRIPT_SEED.as_bytes());
        let pc_gens = PedersenGens::default();
        let mut prover = Prover::new(&pc_gens, &mut prover_transcript);

        for _ in 0..n_tests {
            // Sample two random elements and a modulus, allocate one in the constraint
            // system and add the other directly as a biguint
            let random_elem1 = random_biguint(&mut rng);
            let random_elem2 = random_biguint(&mut rng);
            let random_mod = random_biguint(&mut rng);
            let expected_bigint = (&random_elem1 * &random_elem2) % &random_mod;

            let nonnative = NonNativeElementVar::from_bigint(random_elem1, random_mod, &mut prover);
            let res = NonNativeElementVar::mul_bigint(&nonnative, &random_elem2, &mut prover);

            let res_bigint = res.as_bigint(&prover);
            assert_eq!(res_bigint, expected_bigint);
        }
    }
}