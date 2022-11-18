//! Groups the base type and derived types for the `Fee` entity
use crate::{
    bigint_to_scalar,
    errors::{MpcError, TypeConversionError},
    mpc::SharedFabric,
    Allocate, CommitProver, CommitSharedProver, CommitVerifier,
};
use curve25519_dalek::{ristretto::CompressedRistretto, scalar::Scalar};
use itertools::Itertools;
use mpc_bulletproof::{
    r1cs::{Prover, Variable, Verifier},
    r1cs_mpc::{MpcProver, MpcVariable},
};
use mpc_ristretto::{
    authenticated_ristretto::AuthenticatedCompressedRistretto,
    authenticated_scalar::AuthenticatedScalar, beaver::SharedValueSource, network::MpcNetwork,
};
use num_bigint::BigInt;
use rand_core::{CryptoRng, RngCore};

/// Represents a fee-tuple in the state, i.e. a commitment to pay a relayer for a given
/// match
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Fee {
    /// The public settle key of the cluster collecting fees
    pub settle_key: BigInt,
    /// The mint (ERC-20 Address) of the token used to pay gas
    pub gas_addr: BigInt,
    /// The amount of the mint token to use for gas
    pub gas_token_amount: u64,
    /// The percentage fee that the cluster may take upon match
    /// For now this is encoded as a u64, which represents a
    /// fixed point rational under the hood
    pub percentage_fee: u64,
}

impl TryFrom<&[u64]> for Fee {
    type Error = TypeConversionError;

    fn try_from(values: &[u64]) -> Result<Self, Self::Error> {
        if values.len() != 4 {
            return Err(TypeConversionError(format!(
                "expected array of length 4, got {:?}",
                values.len()
            )));
        }

        Ok(Self {
            settle_key: BigInt::from(values[0]),
            gas_addr: BigInt::from(values[1]),
            gas_token_amount: values[2],
            percentage_fee: values[3],
        })
    }
}

impl From<&Fee> for Vec<u64> {
    fn from(fee: &Fee) -> Self {
        vec![
            fee.settle_key.clone().try_into().unwrap(),
            fee.gas_addr.clone().try_into().unwrap(),
            fee.gas_token_amount,
            fee.percentage_fee,
        ]
    }
}

/// A fee with values allocated in a single-prover constraint system
#[derive(Clone, Debug)]
pub struct FeeVar {
    /// The public settle key of the cluster collecting fees
    pub settle_key: Variable,
    /// The mint (ERC-20 Address) of the token used to pay gas
    pub gas_addr: Variable,
    /// The amount of the mint token to use for gas
    pub gas_token_amount: Variable,
    /// The percentage fee that the cluster may take upon match
    /// For now this is encoded as a u64, which represents a
    /// fixed point rational under the hood
    pub percentage_fee: Variable,
}

impl From<FeeVar> for Vec<Variable> {
    fn from(fee: FeeVar) -> Self {
        vec![
            fee.settle_key,
            fee.gas_addr,
            fee.gas_token_amount,
            fee.percentage_fee,
        ]
    }
}

impl CommitProver for Fee {
    type VarType = FeeVar;
    type CommitType = CommittedFee;
    type ErrorType = (); // Does not error

    fn commit_prover<R: RngCore + CryptoRng>(
        &self,
        rng: &mut R,
        prover: &mut Prover,
    ) -> Result<(Self::VarType, Self::CommitType), Self::ErrorType> {
        let (settle_comm, settle_var) =
            prover.commit(bigint_to_scalar(&self.settle_key), Scalar::random(rng));
        let (addr_comm, addr_var) =
            prover.commit(bigint_to_scalar(&self.gas_addr), Scalar::random(rng));
        let (amount_comm, amount_var) =
            prover.commit(Scalar::from(self.gas_token_amount), Scalar::random(rng));
        let (percent_comm, percent_var) =
            prover.commit(Scalar::from(self.percentage_fee), Scalar::random(rng));

        Ok((
            FeeVar {
                settle_key: settle_var,
                gas_addr: addr_var,
                gas_token_amount: amount_var,
                percentage_fee: percent_var,
            },
            CommittedFee {
                settle_key: settle_comm,
                gas_addr: addr_comm,
                gas_token_amount: amount_comm,
                percentage_fee: percent_comm,
            },
        ))
    }
}

/// A fee that has been committed to in a single-prover constraint system
#[derive(Clone, Debug)]
pub struct CommittedFee {
    /// The public settle key of the cluster collecting fees
    pub settle_key: CompressedRistretto,
    /// The mint (ERC-20 Address) of the token used to pay gas
    pub gas_addr: CompressedRistretto,
    /// The amount of the mint token to use for gas
    pub gas_token_amount: CompressedRistretto,
    /// The percentage fee that the cluster may take upon match
    /// For now this is encoded as a u64, which represents a
    /// fixed point rational under the hood
    pub percentage_fee: CompressedRistretto,
}

impl CommitVerifier for CommittedFee {
    type VarType = FeeVar;
    type ErrorType = (); // Does not error

    fn commit_verifier(&self, verifier: &mut Verifier) -> Result<Self::VarType, Self::ErrorType> {
        let settle_var = verifier.commit(self.settle_key);
        let addr_var = verifier.commit(self.gas_addr);
        let amount_var = verifier.commit(self.gas_token_amount);
        let percentage_var = verifier.commit(self.percentage_fee);

        Ok(FeeVar {
            settle_key: settle_var,
            gas_addr: addr_var,
            gas_token_amount: amount_var,
            percentage_fee: percentage_var,
        })
    }
}

/// A fee with values that have been allocated in an MPC network
#[derive(Clone, Debug)]
pub struct AuthenticatedFee<N: MpcNetwork + Send, S: SharedValueSource<Scalar>> {
    /// The public settle key of the cluster collecting fees
    pub settle_key: AuthenticatedScalar<N, S>,
    /// The mint (ERC-20 Address) of the token used to pay gas
    pub gas_addr: AuthenticatedScalar<N, S>,
    /// The amount of the mint token to use for gas
    pub gas_token_amount: AuthenticatedScalar<N, S>,
    /// The percentage fee that the cluster may take upon match
    /// For now this is encoded as a u64, which represents a
    /// fixed point rational under the hood
    pub percentage_fee: AuthenticatedScalar<N, S>,
}

impl<N: MpcNetwork + Send, S: SharedValueSource<Scalar>> From<AuthenticatedFee<N, S>>
    for Vec<AuthenticatedScalar<N, S>>
{
    fn from(fee: AuthenticatedFee<N, S>) -> Self {
        vec![
            fee.settle_key,
            fee.gas_addr,
            fee.gas_token_amount,
            fee.percentage_fee,
        ]
    }
}

impl<N: MpcNetwork + Send, S: SharedValueSource<Scalar>> From<&[AuthenticatedScalar<N, S>]>
    for AuthenticatedFee<N, S>
{
    fn from(values: &[AuthenticatedScalar<N, S>]) -> Self {
        Self {
            settle_key: values[0].to_owned(),
            gas_addr: values[1].to_owned(),
            gas_token_amount: values[2].to_owned(),
            percentage_fee: values[3].to_owned(),
        }
    }
}

impl<N: MpcNetwork + Send, S: SharedValueSource<Scalar>> Allocate<N, S> for Fee {
    type SharedType = AuthenticatedFee<N, S>;
    type ErrorType = MpcError;

    fn allocate(
        &self,
        owning_party: u64,
        fabric: SharedFabric<N, S>,
    ) -> Result<Self::SharedType, Self::ErrorType> {
        let shared_values = fabric
            .borrow_fabric()
            .batch_allocate_private_scalars(
                owning_party,
                &[
                    bigint_to_scalar(&self.settle_key),
                    bigint_to_scalar(&self.gas_addr),
                    Scalar::from(self.gas_token_amount),
                    Scalar::from(self.percentage_fee),
                ],
            )
            .map_err(|err| MpcError::SharingError(err.to_string()))?;

        Ok(AuthenticatedFee {
            settle_key: shared_values[0].to_owned(),
            gas_addr: shared_values[1].to_owned(),
            gas_token_amount: shared_values[2].to_owned(),
            percentage_fee: shared_values[3].to_owned(),
        })
    }
}

/// Represents a fee that has been allocated in an MPC network and committed to in
/// a multi-prover constraint system
#[derive(Clone, Debug)]
pub struct AuthenticatedFeeVar<N: MpcNetwork + Send, S: SharedValueSource<Scalar>> {
    /// The public settle key of the cluster collecting fees
    pub settle_key: MpcVariable<N, S>,
    /// The mint (ERC-20 Address) of the token used to pay gas
    pub gas_addr: MpcVariable<N, S>,
    /// The amount of the mint token to use for gas
    pub gas_token_amount: MpcVariable<N, S>,
    /// The percentage fee that the cluster may take upon match
    /// For now this is encoded as a u64, which represents a
    /// fixed point rational under the hood
    pub percentage_fee: MpcVariable<N, S>,
}

impl<N: MpcNetwork + Send, S: SharedValueSource<Scalar>> From<AuthenticatedFeeVar<N, S>>
    for Vec<MpcVariable<N, S>>
{
    fn from(fee: AuthenticatedFeeVar<N, S>) -> Self {
        vec![
            fee.settle_key,
            fee.gas_addr,
            fee.gas_token_amount,
            fee.percentage_fee,
        ]
    }
}

impl<N: MpcNetwork + Send, S: SharedValueSource<Scalar>> CommitSharedProver<N, S> for Fee {
    type SharedVarType = AuthenticatedFeeVar<N, S>;
    type CommitType = AuthenticatedCommittedFee<N, S>;
    type ErrorType = MpcError;

    fn commit<R: RngCore + CryptoRng>(
        &self,
        owning_party: u64,
        rng: &mut R,
        prover: &mut MpcProver<N, S>,
    ) -> Result<(Self::SharedVarType, Self::CommitType), Self::ErrorType> {
        let blinders = (0..4).map(|_| Scalar::random(rng)).collect_vec();
        let (shared_comm, shared_vars) = prover
            .batch_commit(
                owning_party,
                &[
                    bigint_to_scalar(&self.settle_key),
                    bigint_to_scalar(&self.gas_addr),
                    Scalar::from(self.gas_token_amount),
                    Scalar::from(self.percentage_fee),
                ],
                &blinders,
            )
            .map_err(|err| MpcError::SharingError(err.to_string()))?;

        Ok((
            AuthenticatedFeeVar {
                settle_key: shared_vars[0].to_owned(),
                gas_addr: shared_vars[1].to_owned(),
                gas_token_amount: shared_vars[2].to_owned(),
                percentage_fee: shared_vars[3].to_owned(),
            },
            // TODO: implement clone for AuthenticatedCompressedRistretto
            AuthenticatedCommittedFee {
                settle_key: shared_comm[0].to_owned(),
                gas_addr: shared_comm[1].to_owned(),
                gas_token_amount: shared_comm[2].to_owned(),
                percentage_fee: shared_comm[3].to_owned(),
            },
        ))
    }
}

/// Represents a fee that has been committed to in a multi-prover constraint system
#[derive(Clone, Debug)]
pub struct AuthenticatedCommittedFee<N: MpcNetwork + Send, S: SharedValueSource<Scalar>> {
    /// The public settle key of the cluster collecting fees
    pub settle_key: AuthenticatedCompressedRistretto<N, S>,
    /// The mint (ERC-20 Address) of the token used to pay gas
    pub gas_addr: AuthenticatedCompressedRistretto<N, S>,
    /// The amount of the mint token to use for gas
    pub gas_token_amount: AuthenticatedCompressedRistretto<N, S>,
    /// The percentage fee that the cluster may take upon match
    /// For now this is encoded as a u64, which represents a
    /// fixed point rational under the hood
    pub percentage_fee: AuthenticatedCompressedRistretto<N, S>,
}

impl<N: MpcNetwork + Send, S: SharedValueSource<Scalar>> From<AuthenticatedCommittedFee<N, S>>
    for Vec<AuthenticatedCompressedRistretto<N, S>>
{
    fn from(commit: AuthenticatedCommittedFee<N, S>) -> Self {
        vec![
            commit.settle_key,
            commit.gas_addr,
            commit.gas_token_amount,
            commit.percentage_fee,
        ]
    }
}

impl<N: MpcNetwork + Send, S: SharedValueSource<Scalar>> CommitVerifier
    for AuthenticatedCommittedFee<N, S>
{
    type VarType = FeeVar;
    type ErrorType = MpcError;

    fn commit_verifier(&self, verifier: &mut Verifier) -> Result<Self::VarType, Self::ErrorType> {
        let opened_values = AuthenticatedCompressedRistretto::batch_open_and_authenticate(&[
            self.settle_key.clone(),
            self.gas_addr.clone(),
            self.gas_token_amount.clone(),
            self.percentage_fee.clone(),
        ])
        .map_err(|err| MpcError::SharingError(err.to_string()))?;

        let settle_var = verifier.commit(opened_values[0].value());
        let addr_var = verifier.commit(opened_values[1].value());
        let amount_var = verifier.commit(opened_values[2].value());
        let percentage_var = verifier.commit(opened_values[3].value());

        Ok(FeeVar {
            settle_key: settle_var,
            gas_addr: addr_var,
            gas_token_amount: amount_var,
            percentage_fee: percentage_var,
        })
    }
}