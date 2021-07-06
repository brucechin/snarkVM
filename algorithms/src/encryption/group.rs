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

use crate::{encryption::GroupEncryptionParameters, errors::EncryptionError, traits::EncryptionScheme};
use snarkvm_curves::traits::{AffineCurve, Group, ProjectiveCurve};
use snarkvm_fields::{Field, One, PrimeField, Zero};
use snarkvm_utilities::{
    bytes_to_bits,
    errors::SerializationError,
    rand::UniformRand,
    serialize::*,
    to_bytes,
    FromBytes,
    ToBytes,
};

use digest::Digest;
use itertools::Itertools;
use rand::Rng;
use std::{
    io::{Read, Result as IoResult, Write},
    marker::PhantomData,
};

#[derive(Derivative, CanonicalSerialize, CanonicalDeserialize)]
#[derivative(
    Copy(bound = "G: Group"),
    Clone(bound = "G: Group"),
    PartialEq(bound = "G: Group"),
    Eq(bound = "G: Group"),
    Debug(bound = "G: Group"),
    Hash(bound = "G: Group")
)]
pub struct GroupEncryptionPublicKey<G: Group + ProjectiveCurve + CanonicalSerialize + CanonicalDeserialize>(pub G);

impl<G: Group + ProjectiveCurve + CanonicalSerialize + CanonicalDeserialize> ToBytes for GroupEncryptionPublicKey<G> {
    /// Writes the x-coordinate of the encryption public key.
    #[inline]
    fn write<W: Write>(&self, mut writer: W) -> IoResult<()> {
        let affine = self.0.into_affine();
        let x_coordinate = affine.to_x_coordinate();
        x_coordinate.write(&mut writer)
    }
}

impl<G: Group + ProjectiveCurve + CanonicalSerialize + CanonicalDeserialize> FromBytes for GroupEncryptionPublicKey<G> {
    /// Reads the x-coordinate of the encryption public key.
    #[inline]
    fn read<R: Read>(mut reader: R) -> IoResult<Self> {
        let x_coordinate = <G::Affine as AffineCurve>::BaseField::read(&mut reader)?;

        if let Some(element) = <G as ProjectiveCurve>::Affine::from_x_coordinate(x_coordinate, true) {
            if element.is_in_correct_subgroup_assuming_on_curve() {
                return Ok(Self(element.into_projective()));
            }
        }

        if let Some(element) = <G as ProjectiveCurve>::Affine::from_x_coordinate(x_coordinate, false) {
            if element.is_in_correct_subgroup_assuming_on_curve() {
                return Ok(Self(element.into_projective()));
            }
        }

        Err(EncryptionError::Message("Failed to read encryption public key".into()).into())
    }
}

impl<G: Group + ProjectiveCurve + CanonicalSerialize + CanonicalDeserialize> Default for GroupEncryptionPublicKey<G> {
    fn default() -> Self {
        Self(G::default())
    }
}

#[derive(Derivative)]
#[derivative(
    Clone(bound = "G: Group, SG: Group, D: Digest"),
    Debug(bound = "G: Group, SG: Group, D: Digest"),
    PartialEq(bound = "G: Group, SG: Group, D: Digest"),
    Eq(bound = "G: Group, SG: Group, D: Digest")
)]
pub struct GroupEncryption<G: Group + ProjectiveCurve, SG: Group, D: Digest> {
    pub parameters: GroupEncryptionParameters<G>,
    pub _signature_group: PhantomData<SG>,
    pub _hash: PhantomData<D>,
}

impl<G: Group + ProjectiveCurve, SG: Group + CanonicalSerialize + CanonicalDeserialize, D: Digest + Send + Sync>
    EncryptionScheme for GroupEncryption<G, SG, D>
{
    type BlindingExponent = <G as Group>::ScalarField;
    type Parameters = GroupEncryptionParameters<G>;
    type PrivateKey = <G as Group>::ScalarField;
    type PublicKey = GroupEncryptionPublicKey<G>;
    type Randomness = <G as Group>::ScalarField;
    type Text = G;

    fn setup<R: Rng>(rng: &mut R) -> Self {
        Self {
            parameters: <Self as EncryptionScheme>::Parameters::setup(
                rng,
                <Self as EncryptionScheme>::PrivateKey::size_in_bits(),
            ),
            _signature_group: PhantomData,
            _hash: PhantomData,
        }
    }

    fn generate_private_key<R: Rng>(&self, rng: &mut R) -> <Self as EncryptionScheme>::PrivateKey {
        let keygen_time = start_timer!(|| "GroupEncryption::generate_private_key");
        let private_key = <G as Group>::ScalarField::rand(rng);
        end_timer!(keygen_time);

        private_key
    }

    fn generate_public_key(
        &self,
        private_key: &<Self as EncryptionScheme>::PrivateKey,
    ) -> Result<<Self as EncryptionScheme>::PublicKey, EncryptionError> {
        let keygen_time = start_timer!(|| "GroupEncryption::generate_public_key");

        let mut public_key = G::zero();
        for (bit, base_power) in bytes_to_bits(&to_bytes![private_key]?).zip_eq(&self.parameters.generator_powers) {
            if bit {
                public_key += base_power;
            }
        }
        end_timer!(keygen_time);

        Ok(GroupEncryptionPublicKey(public_key))
    }

    fn generate_randomness<R: Rng>(
        &self,
        public_key: &<Self as EncryptionScheme>::PublicKey,
        rng: &mut R,
    ) -> Result<Self::Randomness, EncryptionError> {
        let mut y = Self::Randomness::zero();
        let mut z_bytes = vec![];

        while Self::Randomness::read(&z_bytes[..]).is_err() {
            y = Self::Randomness::rand(rng);

            let affine = public_key.0.mul(y).into_affine();
            debug_assert!(affine.is_in_correct_subgroup_assuming_on_curve());
            z_bytes = to_bytes![affine.to_x_coordinate()]?;
        }

        Ok(y)
    }

    fn generate_blinding_exponents(
        &self,
        public_key: &<Self as EncryptionScheme>::PublicKey,
        randomness: &Self::Randomness,
        message_length: usize,
    ) -> Result<Vec<Self::BlindingExponent>, EncryptionError> {
        let record_view_key = public_key.0.mul(*randomness);

        let affine = record_view_key.into_affine();
        debug_assert!(affine.is_in_correct_subgroup_assuming_on_curve());
        let z_bytes = to_bytes![affine.to_x_coordinate()]?;

        let z = Self::Randomness::read(&z_bytes[..])?;

        let one = Self::Randomness::one();
        let mut i = Self::Randomness::one();

        let mut blinding_exponents = vec![];
        for _ in 0..message_length {
            // 1 [/] (z [+] i)
            match (z + i).inverse() {
                Some(val) => blinding_exponents.push(val),
                None => return Err(EncryptionError::MissingInverse),
            };

            i += one;
        }

        Ok(blinding_exponents)
    }

    fn encrypt(
        &self,
        public_key: &<Self as EncryptionScheme>::PublicKey,
        randomness: &Self::Randomness,
        message: &[Self::Text],
    ) -> Result<Vec<Vec<u8>>, EncryptionError> {
        let record_view_key = public_key.0.mul(*randomness);

        let mut c_0 = G::zero();
        for (bit, base_power) in bytes_to_bits(&to_bytes![randomness]?).zip_eq(&self.parameters.generator_powers) {
            if bit {
                c_0 += base_power;
            }
        }
        let mut ciphertext = vec![c_0.to_bytes()?];

        let one = Self::Randomness::one();
        let mut i = Self::Randomness::one();

        let blinding_exponents = self.generate_blinding_exponents(public_key, randomness, message.len())?;

        for (m_i, blinding_exp) in message.iter().zip_eq(blinding_exponents) {
            // h_i <- 1 [/] (z [+] i) * record_view_key
            let h_i = record_view_key.mul(blinding_exp);

            // c_i <- h_i + m_i
            let c_i = h_i + m_i;

            ciphertext.push(c_i.to_bytes()?);
            i += one;
        }

        Ok(ciphertext)
    }

    fn decrypt<B: AsRef<[u8]>>(
        &self,
        private_key: &<Self as EncryptionScheme>::PrivateKey,
        ciphertext: &[B],
    ) -> Result<Vec<Self::Text>, EncryptionError> {
        assert!(!ciphertext.is_empty());
        let c_0 = Self::Text::from_bytes(ciphertext[0].as_ref())?;

        let record_view_key = c_0.mul(*private_key);

        let affine = record_view_key.into_affine();
        debug_assert!(affine.is_in_correct_subgroup_assuming_on_curve());
        let z_bytes = to_bytes![affine.to_x_coordinate()]?;

        let z = Self::Randomness::read(&z_bytes[..])?;

        let one = Self::Randomness::one();
        let mut plaintext = Vec::with_capacity(ciphertext.len().saturating_sub(1));
        let mut i = Self::Randomness::one();

        for c_i in ciphertext.iter().skip(1) {
            // h_i <- 1 [/] (z [+] i) * record_view_key
            let h_i = match &(z + i).inverse() {
                Some(val) => record_view_key.mul(*val),
                None => return Err(EncryptionError::MissingInverse),
            };

            // m_i <- c_i - h_i
            let m_i = Self::Text::from_bytes(c_i.as_ref())? - h_i;

            plaintext.push(m_i);
            i += one;
        }

        Ok(plaintext)
    }

    fn parameters(&self) -> &<Self as EncryptionScheme>::Parameters {
        &self.parameters
    }

    fn private_key_size_in_bits() -> usize {
        <Self as EncryptionScheme>::PrivateKey::size_in_bits()
    }
}

impl<G: Group + ProjectiveCurve, SG: Group, D: Digest> From<GroupEncryptionParameters<G>>
    for GroupEncryption<G, SG, D>
{
    fn from(parameters: GroupEncryptionParameters<G>) -> Self {
        Self {
            parameters,
            _signature_group: PhantomData,
            _hash: PhantomData,
        }
    }
}
