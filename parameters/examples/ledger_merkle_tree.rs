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

use snarkvm_algorithms::{errors::MerkleError, traits::MerkleParameters};
use snarkvm_dpc::testnet2::{instantiated::Components, Testnet2Components as Testnet1Components};
use snarkvm_utilities::ToBytes;

use std::path::PathBuf;

mod utils;
use utils::store;

pub fn setup<C: Testnet1Components>() -> Result<Vec<u8>, MerkleError> {
    let ledger_merkle_tree_parameters = <C::MerkleParameters as MerkleParameters>::setup("MerkleParameters");
    let ledger_merkle_tree_parameters_bytes = ledger_merkle_tree_parameters.crh().to_bytes_le()?;

    let size = ledger_merkle_tree_parameters_bytes.len();
    println!("ledger_merkle_tree.params\n\tsize - {}", size);
    Ok(ledger_merkle_tree_parameters_bytes)
}

pub fn main() {
    let bytes = setup::<Components>().unwrap();
    let filename = PathBuf::from("ledger_merkle_tree.params");
    let sumname = PathBuf::from("ledger_merkle_tree.checksum");
    store(&filename, &sumname, &bytes).unwrap();
}
