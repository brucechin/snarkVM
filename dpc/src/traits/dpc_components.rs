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

use snarkvm_algorithms::traits::{CommitmentScheme, EncryptionScheme, SignatureScheme, CRH, PRF};
use snarkvm_curves::{MontgomeryParameters, PairingEngine, ProjectiveCurve, TwistedEdwardsParameters};
use snarkvm_fields::{PrimeField, ToConstraintField};
use snarkvm_gadgets::{
    traits::algorithms::{CRHGadget, CommitmentGadget, EncryptionGadget, PRFGadget, SignatureGadget},
    CompressedGroupGadget,
};

pub trait DPCComponents: 'static + Sized {
    const NETWORK_ID: u8;

    const NUM_INPUT_RECORDS: usize;
    const NUM_OUTPUT_RECORDS: usize;
    const NUM_TOTAL_RECORDS: usize = Self::NUM_INPUT_RECORDS + Self::NUM_OUTPUT_RECORDS;

    type InnerCurve: PairingEngine;
    type OuterCurve: PairingEngine;

    type InnerScalarField: PrimeField;
    type OuterScalarField: PrimeField;

    /// Commitment scheme for account contents. Invoked only over `Self::InnerScalarField`.
    type AccountCommitment: CommitmentScheme + ToConstraintField<Self::InnerScalarField>;
    type AccountCommitmentGadget: CommitmentGadget<Self::AccountCommitment, Self::InnerScalarField>;

    /// Encryption scheme for account records. Invoked only over `Self::InnerScalarField`.
    type AccountEncryption: EncryptionScheme + ToConstraintField<Self::InnerScalarField>;
    type AccountEncryptionGadget: EncryptionGadget<Self::AccountEncryption, Self::InnerScalarField>;

    /// Signature scheme for delegated compute. Invoked only over `Self::InnerScalarField`.
    type AccountSignature: SignatureScheme + ToConstraintField<Self::InnerScalarField>;
    type AccountSignatureGadget: SignatureGadget<Self::AccountSignature, Self::InnerScalarField>;

    /// CRH for the encrypted record. Invoked only over `Self::InnerScalarField`.
    type EncryptedRecordCRH: CRH + ToConstraintField<Self::InnerScalarField>;
    type EncryptedRecordCRHGadget: CRHGadget<Self::EncryptedRecordCRH, Self::InnerScalarField>;

    /// Group and Model Parameters for record encryption
    type EncryptionGroup: ProjectiveCurve;
    type EncryptionGroupGadget: CompressedGroupGadget<Self::EncryptionGroup, Self::InnerScalarField>;
    type EncryptionParameters: MontgomeryParameters + TwistedEdwardsParameters;

    /// CRH for hash of the `Self::InnerSNARK` verification keys.
    /// This is invoked only on the larger curve.
    type InnerCircuitIDCRH: CRH;
    type InnerCircuitIDCRHGadget: CRHGadget<Self::InnerCircuitIDCRH, Self::OuterScalarField>;

    /// CRH and commitment scheme for committing to program input. Invoked inside
    /// `Self::InnerSNARK` and every program SNARK.
    type LocalDataCRH: CRH + ToConstraintField<Self::InnerScalarField>;
    type LocalDataCRHGadget: CRHGadget<Self::LocalDataCRH, Self::InnerScalarField>;

    type LocalDataCommitment: CommitmentScheme + ToConstraintField<Self::InnerScalarField>;
    type LocalDataCommitmentGadget: CommitmentGadget<Self::LocalDataCommitment, Self::InnerScalarField>;

    /// CRH for hashes of birth and death verification keys.
    /// This is invoked only on the larger curve.
    type ProgramVerificationKeyCRH: CRH;
    type ProgramVerificationKeyCRHGadget: CRHGadget<Self::ProgramVerificationKeyCRH, Self::OuterScalarField>;

    /// Commitment scheme for committing to hashes of birth and death verification keys.
    type ProgramVerificationKeyCommitment: CommitmentScheme + ToConstraintField<Self::InnerScalarField>;
    /// Used to commit to hashes of verification keys on the smaller curve and to decommit hashes
    /// of verification keys on the larger curve
    type ProgramVerificationKeyCommitmentGadget: CommitmentGadget<Self::ProgramVerificationKeyCommitment, Self::InnerScalarField>
        + CommitmentGadget<Self::ProgramVerificationKeyCommitment, Self::OuterScalarField>;

    /// PRF for computing serial numbers. Invoked only over `Self::InnerScalarField`.
    type PRF: PRF;
    type PRFGadget: PRFGadget<Self::PRF, Self::InnerScalarField>;

    /// Commitment scheme for record contents. Invoked only over `Self::InnerScalarField`.
    type RecordCommitment: CommitmentScheme + ToConstraintField<Self::InnerScalarField>;
    type RecordCommitmentGadget: CommitmentGadget<Self::RecordCommitment, Self::InnerScalarField>;

    /// CRH for computing the serial number nonce. Invoked only over `Self::InnerScalarField`.
    type SerialNumberNonceCRH: CRH + ToConstraintField<Self::InnerScalarField>;
    type SerialNumberNonceCRHGadget: CRHGadget<Self::SerialNumberNonceCRH, Self::InnerScalarField>;

    fn account_commitment() -> &'static Self::AccountCommitment;

    fn account_encryption() -> &'static Self::AccountEncryption;

    fn account_signature() -> &'static Self::AccountSignature;

    fn encrypted_record_crh() -> &'static Self::EncryptedRecordCRH;
}
