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

use snarkvm_fields::Field;
use snarkvm_r1cs::errors::SynthesisError;

use snarkvm_r1cs::{ConstraintSynthesizer, ConstraintSystem};

#[derive(Copy, Clone)]
struct Circuit<F: Field> {
    a: Option<F>,
    b: Option<F>,
    num_constraints: usize,
    num_variables: usize,
}

impl<ConstraintF: Field> ConstraintSynthesizer<ConstraintF> for Circuit<ConstraintF> {
    fn generate_constraints<CS: ConstraintSystem<ConstraintF>>(&self, cs: &mut CS) -> Result<(), SynthesisError> {
        let a = cs.alloc(|| "a", || self.a.ok_or(SynthesisError::AssignmentMissing))?;
        let b = cs.alloc(|| "b", || self.b.ok_or(SynthesisError::AssignmentMissing))?;
        let c = cs.alloc_input(
            || "c",
            || {
                let mut a = self.a.ok_or(SynthesisError::AssignmentMissing)?;
                let b = self.b.ok_or(SynthesisError::AssignmentMissing)?;

                a.mul_assign(&b);
                Ok(a)
            },
        )?;

        for i in 0..(self.num_variables - 3) {
            let _ = cs.alloc(
                || format!("var {}", i),
                || self.a.ok_or(SynthesisError::AssignmentMissing),
            )?;
        }

        for i in 0..self.num_constraints {
            cs.enforce(|| format!("constraint {}", i), |lc| lc + a, |lc| lc + b, |lc| lc + c);
        }
        Ok(())
    }
}

mod marlin {
    use super::*;
    use crate::{
        fiat_shamir::{poseidon::PoseidonSponge, FiatShamirAlgebraicSpongeRng},
        marlin::{MarlinCore, MarlinDefaultConfig},
    };
    use snarkvm_curves::bls12_377::{Bls12_377, Fq, Fr};
    use snarkvm_polycommit::{marlin_pc::MarlinKZG10, sonic_pc::SonicKZG10};
    use snarkvm_utilities::rand::{test_rng, UniformRand};

    use blake2::Blake2s;
    use core::ops::MulAssign;

    type FS = FiatShamirAlgebraicSpongeRng<Fr, Fq, PoseidonSponge<Fq>>;

    type MultiPC = MarlinKZG10<Bls12_377>;
    type MarlinInst = MarlinCore<Fr, Fq, MultiPC, FS, MarlinDefaultConfig, Blake2s>;

    type MultiPCSonic = SonicKZG10<Bls12_377>;
    type MarlinSonicInst = MarlinCore<Fr, Fq, MultiPCSonic, FS, MarlinDefaultConfig, Blake2s>;

    macro_rules! impl_marlin_test {
        ($test_struct: ident, $marlin_inst: tt) => {
            struct $test_struct {}
            impl $test_struct {
                pub(crate) fn test_circuit(num_constraints: usize, num_variables: usize) {
                    let rng = &mut test_rng();

                    let universal_srs = $marlin_inst::universal_setup(100, 25, 100, rng).unwrap();

                    for _ in 0..100 {
                        let a = Fr::rand(rng);
                        let b = Fr::rand(rng);
                        let mut c = a;
                        c.mul_assign(&b);

                        let circ = Circuit {
                            a: Some(a),
                            b: Some(b),
                            num_constraints,
                            num_variables,
                        };

                        let (index_pk, index_vk) = $marlin_inst::circuit_setup(&universal_srs, &circ).unwrap();
                        println!("Called index");

                        let proof = $marlin_inst::prove(&index_pk, &circ, rng).unwrap();
                        println!("Called prover");

                        assert!($marlin_inst::verify(&index_vk, &[c], &proof).unwrap());
                        println!("Called verifier");
                        println!("\nShould not verify (i.e. verifier messages should print below):");
                        assert!(!$marlin_inst::verify(&index_vk, &[a], &proof).unwrap());
                    }
                }
            }
        };
    }

    impl_marlin_test!(MarlinPCTest, MarlinInst);
    impl_marlin_test!(SonicPCTest, MarlinSonicInst);

    #[test]
    fn prove_and_verify_with_tall_matrix_big() {
        let num_constraints = 100;
        let num_variables = 25;

        MarlinPCTest::test_circuit(num_constraints, num_variables);
        SonicPCTest::test_circuit(num_constraints, num_variables);
    }

    #[test]
    fn prove_and_verify_with_tall_matrix_small() {
        let num_constraints = 26;
        let num_variables = 25;

        MarlinPCTest::test_circuit(num_constraints, num_variables);
        SonicPCTest::test_circuit(num_constraints, num_variables);
    }

    #[test]
    fn prove_and_verify_with_squat_matrix_big() {
        let num_constraints = 25;
        let num_variables = 100;

        MarlinPCTest::test_circuit(num_constraints, num_variables);
        SonicPCTest::test_circuit(num_constraints, num_variables);
    }

    #[test]
    fn prove_and_verify_with_squat_matrix_small() {
        let num_constraints = 25;
        let num_variables = 26;

        MarlinPCTest::test_circuit(num_constraints, num_variables);
        SonicPCTest::test_circuit(num_constraints, num_variables);
    }

    #[test]
    fn prove_and_verify_with_square_matrix() {
        let num_constraints = 25;
        let num_variables = 25;

        MarlinPCTest::test_circuit(num_constraints, num_variables);
        SonicPCTest::test_circuit(num_constraints, num_variables);
    }
}
