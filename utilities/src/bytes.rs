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

use crate::{
    error,
    io::{Read, Result as IoResult, Write},
    Vec,
};

pub trait ToBytes {
    /// Serializes `self` into `writer`.
    fn write<W: Write>(&self, writer: W) -> IoResult<()>;

    fn to_bytes(&self) -> IoResult<Vec<u8>> {
        let mut out = vec![];
        self.write(&mut out)?;
        Ok(out)
    }

    fn to_array<const N: usize>(&self) -> IoResult<[u8; N]> {
        let mut out = [0u8; N];
        let mut out_slice = &mut out[..];
        self.write(&mut out_slice)?; // todo: check for bugs
        if !out_slice.is_empty() {
            return Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "did not receive enough bytes"))
        }
        Ok(out)
    }
}

pub trait FromBytes: Sized {
    /// Reads `Self` from `reader`.
    fn read<R: Read>(reader: R) -> IoResult<Self>;

    fn from_bytes(mut bytes: &[u8]) -> IoResult<Self> {
        Self::read(&mut bytes)
    }
}

impl<const N: usize> ToBytes for [u8; N] {
    #[inline]
    fn write<W: Write>(&self, mut writer: W) -> IoResult<()> {
        writer.write_all(self)
    }
}

impl<const N: usize> FromBytes for [u8; N] {
    #[inline]
    fn read<R: Read>(mut reader: R) -> IoResult<Self> {
        let mut arr = [0u8; N];
        reader.read_exact(&mut arr)?;
        Ok(arr)
    }
}

impl<const N: usize> ToBytes for [u16; N] {
    #[inline]
    fn write<W: Write>(&self, mut writer: W) -> IoResult<()> {
        for num in self {
            writer.write_all(&num.to_le_bytes())?;
        }
        Ok(())
    }
}

impl<const N: usize> FromBytes for [u16; N] {
    #[inline]
    fn read<R: Read>(mut reader: R) -> IoResult<Self> {
        let mut res = [0u16; N];
        for num in res.iter_mut() {
            let mut bytes = [0u8; 2];
            reader.read_exact(&mut bytes)?;
            *num = u16::from_le_bytes(bytes);
        }
        Ok(res)
    }
}

impl<const N: usize> ToBytes for [u32; N] {
    #[inline]
    fn write<W: Write>(&self, mut writer: W) -> IoResult<()> {
        for num in self {
            writer.write_all(&num.to_le_bytes())?;
        }
        Ok(())
    }
}

impl<const N: usize> FromBytes for [u32; N] {
    #[inline]
    fn read<R: Read>(mut reader: R) -> IoResult<Self> {
        let mut res = [0u32; N];
        for num in res.iter_mut() {
            let mut bytes = [0u8; 4];
            reader.read_exact(&mut bytes)?;
            *num = u32::from_le_bytes(bytes);
        }
        Ok(res)
    }
}

impl<const N: usize> ToBytes for [u64; N] {
    #[inline]
    fn write<W: Write>(&self, mut writer: W) -> IoResult<()> {
        for num in self {
            writer.write_all(&num.to_le_bytes())?;
        }
        Ok(())
    }
}

impl<const N: usize> FromBytes for [u64; N] {
    #[inline]
    fn read<R: Read>(mut reader: R) -> IoResult<Self> {
        let mut res = [0u64; N];
        for num in res.iter_mut() {
            let mut bytes = [0u8; 8];
            reader.read_exact(&mut bytes)?;
            *num = u64::from_le_bytes(bytes);
        }
        Ok(res)
    }
}

impl<L: ToBytes, R: ToBytes> ToBytes for (L, R) {
    fn write<W: Write>(&self, mut writer: W) -> IoResult<()> {
        self.0.write(&mut writer)?;
        self.1.write(&mut writer)?;
        Ok(())
    }
}

/// Takes as input a sequence of structs, and converts them to a series of
/// bytes. All traits that implement `Bytes` can be automatically converted to
/// bytes in this manner.
#[macro_export]
macro_rules! to_bytes {
    ($($x:expr),*) => ({
        let mut buf = $crate::vec![];
        {$crate::push_to_vec!(buf, $($x),*)}.map(|_| buf)
    });
}

#[macro_export]
macro_rules! push_to_vec {
    ($buf:expr, $y:expr, $($x:expr),*) => ({
        {
            ToBytes::write(&$y, &mut $buf)
        }.and({$crate::push_to_vec!($buf, $($x),*)})
    });

    ($buf:expr, $x:expr) => ({
        ToBytes::write(&$x, &mut $buf)
    })
}

impl ToBytes for u8 {
    #[inline]
    fn write<W: Write>(&self, mut writer: W) -> IoResult<()> {
        writer.write_all(&[*self])
    }
}

impl FromBytes for u8 {
    #[inline]
    fn read<R: Read>(mut reader: R) -> IoResult<Self> {
        let mut byte = [0u8];
        reader.read_exact(&mut byte)?;
        Ok(byte[0])
    }
}

impl ToBytes for u16 {
    #[inline]
    fn write<W: Write>(&self, mut writer: W) -> IoResult<()> {
        writer.write_all(&self.to_le_bytes())
    }
}

impl FromBytes for u16 {
    #[inline]
    fn read<R: Read>(mut reader: R) -> IoResult<Self> {
        let mut bytes = [0u8; 2];
        reader.read_exact(&mut bytes)?;
        Ok(u16::from_le_bytes(bytes))
    }
}

impl ToBytes for u32 {
    #[inline]
    fn write<W: Write>(&self, mut writer: W) -> IoResult<()> {
        writer.write_all(&self.to_le_bytes())
    }
}

impl FromBytes for u32 {
    #[inline]
    fn read<R: Read>(mut reader: R) -> IoResult<Self> {
        let mut bytes = [0u8; 4];
        reader.read_exact(&mut bytes)?;
        Ok(u32::from_le_bytes(bytes))
    }
}

impl ToBytes for u64 {
    #[inline]
    fn write<W: Write>(&self, mut writer: W) -> IoResult<()> {
        writer.write_all(&self.to_le_bytes())
    }
}

impl FromBytes for u64 {
    #[inline]
    fn read<R: Read>(mut reader: R) -> IoResult<Self> {
        let mut bytes = [0u8; 8];
        reader.read_exact(&mut bytes)?;
        Ok(u64::from_le_bytes(bytes))
    }
}

impl ToBytes for u128 {
    #[inline]
    fn write<W: Write>(&self, mut writer: W) -> IoResult<()> {
        writer.write_all(&self.to_le_bytes())
    }
}

impl FromBytes for u128 {
    #[inline]
    fn read<R: Read>(mut reader: R) -> IoResult<Self> {
        let mut bytes = [0u8; 16];
        reader.read_exact(&mut bytes)?;
        Ok(u128::from_le_bytes(bytes))
    }
}

impl ToBytes for i64 {
    #[inline]
    fn write<W: Write>(&self, mut writer: W) -> IoResult<()> {
        writer.write_all(&self.to_le_bytes())
    }
}

impl FromBytes for i64 {
    #[inline]
    fn read<R: Read>(mut reader: R) -> IoResult<Self> {
        let mut bytes = [0u8; 8];
        reader.read_exact(&mut bytes)?;
        Ok(i64::from_le_bytes(bytes))
    }
}

impl ToBytes for () {
    #[inline]
    fn write<W: Write>(&self, _writer: W) -> IoResult<()> {
        Ok(())
    }
}

impl FromBytes for () {
    #[inline]
    fn read<R: Read>(_bytes: R) -> IoResult<Self> {
        Ok(())
    }
}

impl ToBytes for bool {
    #[inline]
    fn write<W: Write>(&self, writer: W) -> IoResult<()> {
        u8::write(&(*self as u8), writer)
    }
}

impl FromBytes for bool {
    #[inline]
    fn read<R: Read>(reader: R) -> IoResult<Self> {
        match u8::read(reader) {
            Ok(0) => Ok(false),
            Ok(1) => Ok(true),
            Ok(_) => Err(error("FromBytes::read failed")),
            Err(err) => Err(err),
        }
    }
}

impl<T: ToBytes> ToBytes for Vec<T> {
    #[inline]
    fn write<W: Write>(&self, mut writer: W) -> IoResult<()> {
        for item in self {
            item.write(&mut writer)?;
        }
        Ok(())
    }
}

impl<'a, T: 'a + ToBytes> ToBytes for &'a [T] {
    #[inline]
    fn write<W: Write>(&self, mut writer: W) -> IoResult<()> {
        for item in *self {
            item.write(&mut writer)?;
        }
        Ok(())
    }
}

impl<'a, T: 'a + ToBytes> ToBytes for &'a T {
    #[inline]
    fn write<W: Write>(&self, mut writer: W) -> IoResult<()> {
        (*self).write(&mut writer)
    }
}

pub fn bytes_to_bits(bytes: &[u8]) -> impl Iterator<Item = bool> + '_ {
    bytes
        .iter()
        .map(|byte| (0..8).map(move |i| (*byte >> i) & 1 == 1))
        .flatten()
}

pub fn bits_to_bytes(bits: &[bool]) -> Vec<u8> {
    // Pad the bits if it not a correct size
    let mut bits = std::borrow::Cow::from(bits);
    if bits.len() % 8 != 0 {
        let current_length = bits.len();
        bits.to_mut().resize(current_length + 8 - (current_length % 8), false);
    }

    let mut bytes = Vec::with_capacity(bits.len() / 8);
    for bits in bits.chunks(8) {
        let mut result = 0u8;
        for (i, bit) in bits.iter().enumerate() {
            let bit_value = *bit as u8;
            result += bit_value << i as u8;
        }
        bytes.push(result);
    }
    bytes
}

#[cfg(test)]
mod test {
    use super::{bits_to_bytes, bytes_to_bits, ToBytes};
    use crate::Vec;

    use rand::{Rng, SeedableRng};
    use rand_xorshift::XorShiftRng;

    const ITERATIONS: usize = 1000;

    #[test]
    fn test_macro_empty() {
        let array: Vec<u8> = vec![];
        let bytes: Vec<u8> = to_bytes![array].unwrap();
        assert_eq!(&bytes, &array);
        assert_eq!(bytes.len(), 0);
    }

    #[test]
    fn test_macro() {
        let array1 = [1u8; 32];
        let array2 = [2u8; 16];
        let array3 = [3u8; 8];
        let bytes = to_bytes![array1, array2, array3].unwrap();
        assert_eq!(bytes.len(), 56);

        let mut actual_bytes = Vec::new();
        actual_bytes.extend_from_slice(&array1);
        actual_bytes.extend_from_slice(&array2);
        actual_bytes.extend_from_slice(&array3);
        assert_eq!(bytes, actual_bytes);
    }

    #[test]
    fn test_bits_to_bytes() {
        let mut rng = XorShiftRng::seed_from_u64(1231275789u64);

        for _ in 0..ITERATIONS {
            let given_bytes: [u8; 32] = rng.gen();

            let bits = bytes_to_bits(&given_bytes).collect::<Vec<_>>();
            let recovered_bytes = bits_to_bytes(&bits);

            assert_eq!(given_bytes.to_vec(), recovered_bytes);
        }
    }
}
