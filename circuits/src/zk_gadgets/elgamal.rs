//! Implements the ZK gadgetry for ElGamal encryption

use curve25519_dalek::ristretto::CompressedRistretto;
use itertools::Itertools;
use mpc_bulletproof::{
    r1cs::{Prover, R1CSProof, RandomizableConstraintSystem, Verifier},
    BulletproofGens,
};
use num_bigint::BigUint;
use rand_core::OsRng;

use crate::{
    errors::{ProverError, VerifierError},
    CommitProver, CommitVerifier, SingleProverCircuit,
};

use super::{
    edwards::{EdwardsPoint, TwistedEdwardsCurve},
    nonnative::{FieldMod, NonNativeElementVar},
};

/// A gadget that constrains ElGamal encryption under a known public key
pub struct ElGamalGadget<const SCALAR_BITS: usize> {}

impl<const SCALAR_BITS: usize> ElGamalGadget<SCALAR_BITS> {
    /// Constrain the decryption of a given ciphertext to equal the expected result
    pub fn encrypt<CS: RandomizableConstraintSystem>(
        randomness: NonNativeElementVar,
        cleartext: EdwardsPoint,
        public_key: EdwardsPoint,
        curve_basepoint: EdwardsPoint,
        curve: &TwistedEdwardsCurve,
        cs: &mut CS,
    ) -> (EdwardsPoint, EdwardsPoint) {
        // Multiply the randomness with the basepoint to allow the receiver to
        // recover the encryption key
        let randomness_times_basepoint =
            curve.scalar_mul::<SCALAR_BITS, _>(&randomness, &curve_basepoint, cs);
        // Multiply the randomness with the public key, and use it to blind the cleartext
        let randomness_times_public_key =
            curve.scalar_mul::<SCALAR_BITS, _>(&randomness, &public_key, cs);
        let ciphertext = curve.add_points(&cleartext, &randomness_times_public_key, cs);

        (ciphertext, randomness_times_basepoint)
    }
}

/// A witness to the statement of valid encryption
#[derive(Clone, Debug)]
pub struct ElGamalWitness {
    /// The x coordinate of the cleartext
    cleartext_x: BigUint,
    /// The y coordinate of the cleartext
    cleartext_y: BigUint,
    /// The modulus that the field is defined over
    field_mod: FieldMod,
    /// The randomness used to blind the cleartext
    randomness: BigUint,
}

/// The statement parameterization of a correct ElGamal encryption circuit
#[derive(Clone, Debug)]
pub struct ElGamalStatement {
    /// The first point in the expected ciphertext resulting from the encryption
    expected_ciphertext_1: (BigUint, BigUint),
    /// The second point in the expected ciphertext resulting from the encryption
    expected_ciphertext_2: (BigUint, BigUint),
    /// The public key used for encryption
    public_key: (BigUint, BigUint),
    /// The curve basepoint
    basepoint: (BigUint, BigUint),
    /// A parameterization of a twisted Edwards curve
    curve: TwistedEdwardsCurve,
    /// The modulus of the field that the operation is defined over
    field_mod: FieldMod,
}

/// An ElGamal witness that has been allocated in a constraint system
#[derive(Clone, Debug)]
pub struct ElGamalWitnessVar {
    /// The cleartext point mapped onto an Edwards curve
    cleartext_point: EdwardsPoint,
    /// The randomness used to blind the cleartext
    randomness: NonNativeElementVar,
}

impl CommitProver for ElGamalWitness {
    type VarType = ElGamalWitnessVar;
    type CommitType = ElGamalWitnessCommitment;
    type ErrorType = ();

    fn commit_prover<R: rand_core::RngCore + rand_core::CryptoRng>(
        &self,
        rng: &mut R,
        prover: &mut Prover,
    ) -> Result<(Self::VarType, Self::CommitType), Self::ErrorType> {
        // Commit to the witness
        let (cleartext_point, x_comm, y_comm) = EdwardsPoint::commit_witness(
            self.cleartext_x.to_owned(),
            self.cleartext_y.to_owned(),
            self.field_mod.to_owned(),
            rng,
            prover,
        );

        let (randomness_var, randomness_commitment) = NonNativeElementVar::commit_witness(
            self.randomness.to_owned(),
            self.field_mod.to_owned(),
            rng,
            prover,
        );

        Ok((
            ElGamalWitnessVar {
                cleartext_point,
                randomness: randomness_var,
            },
            ElGamalWitnessCommitment {
                cleartext_x_commit: x_comm,
                cleartext_y_commit: y_comm,
                randomness_commit: randomness_commitment,
                field_mod: self.field_mod.to_owned(),
            },
        ))
    }
}

/// A commitment to an ElGamal witness
#[derive(Clone, Debug)]
pub struct ElGamalWitnessCommitment {
    /// The commitment result of the x coordinate of the cleartext
    cleartext_x_commit: Vec<CompressedRistretto>,
    /// The commitment result of the y coordinate of the cleartext
    cleartext_y_commit: Vec<CompressedRistretto>,
    /// The commitment to the randomness used to blind the cleartext
    randomness_commit: Vec<CompressedRistretto>,
    /// The modulus that the field is defined over
    field_mod: FieldMod,
}

impl CommitVerifier for ElGamalWitnessCommitment {
    type VarType = ElGamalWitnessVar;
    type ErrorType = ();

    fn commit_verifier(&self, verifier: &mut Verifier) -> Result<Self::VarType, Self::ErrorType> {
        // Allocate the commitment to the point's words in the verifier's constraint system
        let cleartext_x_vars = self
            .cleartext_x_commit
            .iter()
            .map(|var| verifier.commit(*var).into())
            .collect_vec();
        let cleartext_y_vars = self
            .cleartext_y_commit
            .iter()
            .map(|var| verifier.commit(*var).into())
            .collect_vec();
        let nonnative_x = NonNativeElementVar::new(cleartext_x_vars, self.field_mod.to_owned());
        let nonnative_y = NonNativeElementVar::new(cleartext_y_vars, self.field_mod.to_owned());

        let cleartext_point = EdwardsPoint::new(nonnative_x, nonnative_y);

        // Commit to the randomness
        let randomness_vars = self
            .randomness_commit
            .iter()
            .map(|var| verifier.commit(*var).into())
            .collect_vec();
        let randomness_nonnative =
            NonNativeElementVar::new(randomness_vars, self.field_mod.to_owned());

        Ok(ElGamalWitnessVar {
            cleartext_point,
            randomness: randomness_nonnative,
        })
    }
}

impl<const SCALAR_BITS: usize> SingleProverCircuit for ElGamalGadget<SCALAR_BITS> {
    type Witness = ElGamalWitness;
    type Statement = ElGamalStatement;
    type WitnessCommitment = ElGamalWitnessCommitment;

    const BP_GENS_CAPACITY: usize = 32768;

    fn prove(
        witness: Self::Witness,
        statement: Self::Statement,
        mut prover: Prover,
    ) -> Result<(Self::WitnessCommitment, R1CSProof), ProverError> {
        // Commit to the witness
        let mut rng = OsRng {};
        let (witness_var, witness_comm) = witness.commit_prover(&mut rng, &mut prover).unwrap();

        // Commit to the statement variables
        let expected_ciphertext1 = EdwardsPoint::commit_public(
            statement.expected_ciphertext_1.0,
            statement.expected_ciphertext_1.1,
            statement.field_mod.to_owned(),
            &mut prover,
        );

        let expected_ciphertext2 = EdwardsPoint::commit_public(
            statement.expected_ciphertext_2.0,
            statement.expected_ciphertext_2.1,
            statement.field_mod.to_owned(),
            &mut prover,
        );

        let public_key = EdwardsPoint::commit_public(
            statement.public_key.0,
            statement.public_key.1,
            statement.field_mod.to_owned(),
            &mut prover,
        );

        let basepoint = EdwardsPoint::commit_public(
            statement.basepoint.0,
            statement.basepoint.1,
            statement.field_mod.to_owned(),
            &mut prover,
        );

        // Apply the constraints
        let ciphertext = Self::encrypt(
            witness_var.randomness,
            witness_var.cleartext_point,
            public_key,
            basepoint,
            &statement.curve,
            &mut prover,
        );

        EdwardsPoint::constrain_equal(&ciphertext.0, &expected_ciphertext1, &mut prover);
        EdwardsPoint::constrain_equal(&ciphertext.1, &expected_ciphertext2, &mut prover);

        // Prove the statement
        let bp_gens = BulletproofGens::new(Self::BP_GENS_CAPACITY, 1 /* party_capacity */);
        let proof = prover.prove(&bp_gens).map_err(ProverError::R1CS)?;

        Ok((witness_comm, proof))
    }

    fn verify(
        witness_commitment: Self::WitnessCommitment,
        statement: Self::Statement,
        proof: R1CSProof,
        mut verifier: Verifier,
    ) -> Result<(), VerifierError> {
        // Commit to the witness
        let witness_var = witness_commitment.commit_verifier(&mut verifier).unwrap();

        // Commit to the statement variables
        let expected_ciphertext1 = EdwardsPoint::commit_public(
            statement.expected_ciphertext_1.0,
            statement.expected_ciphertext_1.1,
            statement.field_mod.to_owned(),
            &mut verifier,
        );

        let expected_ciphertext2 = EdwardsPoint::commit_public(
            statement.expected_ciphertext_2.0,
            statement.expected_ciphertext_2.1,
            statement.field_mod.to_owned(),
            &mut verifier,
        );

        let public_key = EdwardsPoint::commit_public(
            statement.public_key.0,
            statement.public_key.1,
            statement.field_mod.to_owned(),
            &mut verifier,
        );

        let basepoint = EdwardsPoint::commit_public(
            statement.basepoint.0,
            statement.basepoint.1,
            statement.field_mod.to_owned(),
            &mut verifier,
        );

        // Apply the constraints
        let ciphertext = Self::encrypt(
            witness_var.randomness,
            witness_var.cleartext_point,
            public_key,
            basepoint,
            &statement.curve,
            &mut verifier,
        );

        EdwardsPoint::constrain_equal(&ciphertext.0, &expected_ciphertext1, &mut verifier);
        EdwardsPoint::constrain_equal(&ciphertext.1, &expected_ciphertext2, &mut verifier);

        // Verify the proof
        let bp_gens = BulletproofGens::new(Self::BP_GENS_CAPACITY, 1 /* party_capacity */);
        verifier
            .verify(&proof, &bp_gens)
            .map_err(VerifierError::R1CS)
    }
}

#[cfg(test)]
mod elgamal_tests {
    use ark_crypto_primitives::encryption::{
        elgamal::{ElGamal, Parameters, Randomness},
        AsymmetricEncryptionScheme,
    };
    use ark_ec::twisted_edwards::TECurveConfig;
    use ark_ed25519::{EdwardsAffine, EdwardsParameters, EdwardsProjective, Fr as EdwardsScalar};
    use num_bigint::BigUint;
    use rand::rngs::OsRng;
    use rand_core::OsRng as CoreOsRng;

    use crate::{
        test_helpers::bulletproof_prove_and_verify,
        zk_gadgets::{
            edwards::edwards_tests::{
                create_ed25519_repr, ed25519_random_felt, ed25519_random_point,
            },
            nonnative::FieldMod,
        },
    };

    use super::{ElGamalGadget, ElGamalStatement, ElGamalWitness};

    /// A type alias for the Arkworks native ElGamal gadget over ed25519
    type ArkworksElGamal = ElGamal<EdwardsProjective>;

    /// Test the encryption circuit
    #[test]
    #[ignore = "too expensive to run in CI"]
    fn test_encryption_circuit() {
        // Setup a random plaintext and randomness
        let mut rng1 = OsRng {};
        let mut rng2 = CoreOsRng {};
        let plaintext = ed25519_random_point(&mut rng2);

        // Sample a small (bitlength) randomness to shrink test complexity
        let randomness = ed25519_random_felt(&mut rng2) % BigUint::from(1u8 << 3);

        // Use the curve25519 field modulus
        let field_mod = FieldMod::from_modulus((BigUint::from(1u8) << 255) - 19u8);

        let encryption_params = Parameters {
            generator: EdwardsParameters::GENERATOR,
        };
        let (pub_key, _): (EdwardsAffine, _) =
            ArkworksElGamal::keygen(&encryption_params, &mut rng1).unwrap();

        // Encrypt the random plaintext via Arkworks
        // Arkworks reverses the order of the ciphertext in our gadget, bind them in reverse order
        let arkworks_randomness = EdwardsScalar::from(randomness.clone());
        let (ciphertext2, ciphertext1): (EdwardsAffine, EdwardsAffine) = ArkworksElGamal::encrypt(
            &encryption_params,
            &pub_key,
            &plaintext,
            &Randomness(arkworks_randomness),
        )
        .unwrap();

        // Now use the expected result to prove the ElGamal valid encryption statement above
        let witness = ElGamalWitness {
            cleartext_x: plaintext.x.into(),
            cleartext_y: plaintext.y.into(),
            field_mod: field_mod.clone(),
            randomness,
        };

        let ed25519_basepoint: (BigUint, BigUint) = (
            EdwardsParameters::GENERATOR.x.into(),
            EdwardsParameters::GENERATOR.y.into(),
        );
        let statement = ElGamalStatement {
            expected_ciphertext_1: (ciphertext1.x.into(), ciphertext1.y.into()),
            expected_ciphertext_2: (ciphertext2.x.into(), ciphertext2.y.into()),
            public_key: (pub_key.x.into(), pub_key.y.into()),
            basepoint: ed25519_basepoint,
            curve: create_ed25519_repr(),
            field_mod,
        };

        let res =
            bulletproof_prove_and_verify::<ElGamalGadget<3 /* SCALAR_BITS */>>(witness, statement);
        assert!(res.is_ok());
    }
}