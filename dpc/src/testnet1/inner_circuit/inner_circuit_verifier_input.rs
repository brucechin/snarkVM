// Copyright (C) 2019-2021 Aleo Systems Inc.
// This file is part of the snarkVM library.

// The snarkVM library is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// The snarkVM library is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with the snarkVM library. If not, see <https://www.gnu.org/licenses/>.

use crate::testnet1::{AleoAmount, BaseDPCComponents, TransactionHash, parameters::SystemParameters};
use snarkvm_algorithms::{
    merkle_tree::MerkleTreeDigest,
    traits::{CommitmentScheme, EncryptionScheme, MerkleParameters, SignatureScheme, CRH},
};
use snarkvm_fields::{ConstraintFieldError, ToConstraintField};

use std::sync::Arc;

#[derive(Derivative)]
#[derivative(Clone(bound = "C: BaseDPCComponents"))]
pub struct InnerCircuitVerifierInput<C: BaseDPCComponents> {
    // Commitment, CRH, and signature parameters
    pub system_parameters: SystemParameters<C>,

    // Ledger parameters and digest
    pub ledger_parameters: Arc<C::MerkleParameters>,
    pub ledger_digest: TransactionHash,

    // Input record serial numbers
    pub old_serial_numbers: Vec<TransactionHash>,

    // Output record commitments
    pub new_commitments: Vec<TransactionHash>,

    // New encrypted record hashes
    pub new_encrypted_record_hashes: Vec<TransactionHash>,

    // Program input commitment and local data root
    pub program_commitment: TransactionHash,
    pub local_data_root: TransactionHash,

    pub memo: [u8; 32],
    pub value_balance: AleoAmount,
    pub network_id: u8,
}

impl<C: BaseDPCComponents> ToConstraintField<C::InnerField> for InnerCircuitVerifierInput<C>
where
    <C::AccountCommitment as CommitmentScheme>::Parameters: ToConstraintField<C::InnerField>,
    <C::AccountCommitment as CommitmentScheme>::Output: ToConstraintField<C::InnerField>,

    <C::AccountEncryption as EncryptionScheme>::Parameters: ToConstraintField<C::InnerField>,

    <C::AccountSignature as SignatureScheme>::Parameters: ToConstraintField<C::InnerField>,
    <C::AccountSignature as SignatureScheme>::PublicKey: ToConstraintField<C::InnerField>,

    <C::RecordCommitment as CommitmentScheme>::Parameters: ToConstraintField<C::InnerField>,
    <C::RecordCommitment as CommitmentScheme>::Output: ToConstraintField<C::InnerField>,

    <C::EncryptedRecordCRH as CRH>::Parameters: ToConstraintField<C::InnerField>,
    <C::EncryptedRecordCRH as CRH>::Output: ToConstraintField<C::InnerField>,

    <C::SerialNumberNonceCRH as CRH>::Parameters: ToConstraintField<C::InnerField>,

    <C::ProgramVerificationKeyCommitment as CommitmentScheme>::Parameters: ToConstraintField<C::InnerField>,
    <C::ProgramVerificationKeyCommitment as CommitmentScheme>::Output: ToConstraintField<C::InnerField>,

    <C::LocalDataCRH as CRH>::Parameters: ToConstraintField<C::InnerField>,
    <C::LocalDataCRH as CRH>::Output: ToConstraintField<C::InnerField>,

    <<C::MerkleParameters as MerkleParameters>::H as CRH>::Parameters: ToConstraintField<C::InnerField>,
    MerkleTreeDigest<C::MerkleParameters>: ToConstraintField<C::InnerField>,
{
    fn to_field_elements(&self) -> Result<Vec<C::InnerField>, ConstraintFieldError> {
        let mut v = Vec::new();

        v.extend_from_slice(
            &self
                .system_parameters
                .account_commitment
                .parameters()
                .to_field_elements()?,
        );
        v.extend_from_slice(
            &<C::AccountEncryption as EncryptionScheme>::parameters(&self.system_parameters.account_encryption)
                .to_field_elements()?,
        );
        v.extend_from_slice(
            &self
                .system_parameters
                .account_signature
                .parameters()
                .to_field_elements()?,
        );
        v.extend_from_slice(
            &self
                .system_parameters
                .record_commitment
                .parameters()
                .to_field_elements()?,
        );
        v.extend_from_slice(
            &self
                .system_parameters
                .encrypted_record_crh
                .parameters()
                .to_field_elements()?,
        );
        v.extend_from_slice(
            &self
                .system_parameters
                .program_verification_key_commitment
                .parameters()
                .to_field_elements()?,
        );
        v.extend_from_slice(&self.system_parameters.local_data_crh.parameters().to_field_elements()?);
        v.extend_from_slice(
            &self
                .system_parameters
                .serial_number_nonce
                .parameters()
                .to_field_elements()?,
        );

        v.extend_from_slice(&self.ledger_parameters.parameters().to_field_elements()?);
        v.extend_from_slice(&self.ledger_digest.to_field_elements()?);

        for sn in &self.old_serial_numbers {
            v.extend_from_slice(&sn.to_field_elements()?);
        }

        for (cm, encrypted_record_hash) in self.new_commitments.iter().zip(&self.new_encrypted_record_hashes) {
            v.extend_from_slice(&cm.to_field_elements()?);
            v.extend_from_slice(&encrypted_record_hash.to_field_elements()?);
        }

        v.extend_from_slice(&self.program_commitment.to_field_elements()?);
        v.extend_from_slice(&ToConstraintField::<C::InnerField>::to_field_elements(&self.memo)?);
        v.extend_from_slice(&ToConstraintField::<C::InnerField>::to_field_elements(
            &[self.network_id][..],
        )?);
        v.extend_from_slice(&self.local_data_root.to_field_elements()?);

        v.extend_from_slice(&ToConstraintField::<C::InnerField>::to_field_elements(
            &self.value_balance.0.to_le_bytes()[..],
        )?);
        Ok(v)
    }
}
