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

use snarkvm_algorithms::{errors::CRHError, traits::CRH};
use snarkvm_dpc::{testnet2::instantiated::Components, traits::DPCComponents};
use snarkvm_utilities::ToBytes;

use std::path::PathBuf;

mod utils;
use utils::store;

pub fn setup<C: DPCComponents>() -> Result<Vec<u8>, CRHError> {
    let program_vk_crh = <C::ProgramVerificationKeyCRH as CRH>::setup("ProgramVerificationKeyCRH");
    let program_vk_crh_parameters_bytes = program_vk_crh.to_bytes_le()?;

    let size = program_vk_crh_parameters_bytes.len();
    println!("program_vk_crh.params\n\tsize - {}", size);
    Ok(program_vk_crh_parameters_bytes)
}

pub fn main() {
    let bytes = setup::<Components>().unwrap();
    let filename = PathBuf::from("program_vk_crh.params");
    let sumname = PathBuf::from("program_vk_crh.checksum");
    store(&filename, &sumname, &bytes).unwrap();
}
