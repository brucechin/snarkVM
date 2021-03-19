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

use crate::ahp::{
    indexer::CircuitInfo,
    verifier::{VerifierFirstMessage, VerifierSecondMessage, VerifierState},
    AHPError,
    AHPForR1CS,
};
use snarkvm_algorithms::fft::EvaluationDomain;
use snarkvm_fields::PrimeField;
use snarkvm_r1cs::errors::SynthesisError;

use snarkvm_polycommit::QuerySet;

use rand_core::RngCore;

impl<F: PrimeField> AHPForR1CS<F> {
    /// Output the first message and next round state.
    pub fn verifier_first_round<R: RngCore>(
        index_info: CircuitInfo<F>,
        rng: &mut R,
    ) -> Result<(VerifierFirstMessage<F>, VerifierState<F>), AHPError> {
        // Check that the R1CS is a square matrix.
        if index_info.num_constraints != index_info.num_variables {
            return Err(AHPError::NonSquareMatrix);
        }

        let domain_h =
            EvaluationDomain::new(index_info.num_constraints).ok_or(SynthesisError::PolynomialDegreeTooLarge)?;

        let domain_k =
            EvaluationDomain::new(index_info.num_non_zero).ok_or(SynthesisError::PolynomialDegreeTooLarge)?;

        let alpha = domain_h.sample_element_outside_domain(rng);
        let eta_a = F::rand(rng);
        let eta_b = F::rand(rng);
        let eta_c = F::rand(rng);

        let message = VerifierFirstMessage {
            alpha,
            eta_a,
            eta_b,
            eta_c,
        };

        let new_state = VerifierState {
            domain_h,
            domain_k,
            first_round_message: Some(message),
            second_round_message: None,
            gamma: None,
        };

        Ok((message, new_state))
    }

    /// Output the second message and next round state.
    pub fn verifier_second_round<R: RngCore>(
        mut state: VerifierState<F>,
        rng: &mut R,
    ) -> (VerifierSecondMessage<F>, VerifierState<F>) {
        let beta = state.domain_h.sample_element_outside_domain(rng);
        let msg = VerifierSecondMessage { beta };
        state.second_round_message = Some(msg);

        (msg, state)
    }

    /// Output the third message and next round state.
    pub fn verifier_third_round<R: RngCore>(mut state: VerifierState<F>, rng: &mut R) -> VerifierState<F> {
        state.gamma = Some(F::rand(rng));
        state
    }

    /// Output the query state and next round state.
    pub fn verifier_query_set<'a, 'b, R: RngCore>(
        state: VerifierState<F>,
        _: &'a mut R,
        with_vanishing: bool,
    ) -> (QuerySet<'b, F>, VerifierState<F>) {
        let alpha = state.first_round_message.unwrap().alpha;
        let beta = state.second_round_message.unwrap().beta;

        let gamma = state.gamma.unwrap();

        let mut query_set = QuerySet::new();
        // For the first linear combination
        // Outer sumcheck test:
        //   s(beta) + r(alpha, beta) * (sum_M eta_M z_M(beta)) - t(beta) * z(beta)
        // = h_1(beta) * v_H(beta) + beta * g_1(beta)
        //
        // LinearCombination::new(
        //      outer_sumcheck
        //      vec![
        //          (F::one(), "mask_poly"),
        //          (r_alpha_beta * (eta_a + eta_c * z_b_at_beta), z_a),
        //          (-t_at_beta * v_X_at_beta, w),
        //          (-v_H_at_beta, h_1),
        //          (-beta * g_1_at_beta, LCTerm::One)
        //          (r_alpha_beta * eta_b * z_a_at_beta, LCTerm::One),
        //          (-t_at_beta * x_poly_at_beta, LCTerm::One),
        //      ],
        //  )
        //  LinearCombination::new("z_b", vec![(F::one(), z_b)])
        //  LinearCombination::new("g_1", vec![(F::one(), g_1)], rhs::new(g_1_at_beta))
        //  LinearCombination::new("t", vec![(F::one(), t)])
        query_set.insert(("g_1".into(), beta));
        query_set.insert(("z_b".into(), beta));
        query_set.insert(("t".into(), beta));
        query_set.insert(("outer_sumcheck".into(), beta));

        // For the second linear combination
        // v_K_at_beta_3 * h_3 - a + v_3 * (beta_3 * 1/beta_3^(D - d_3) * g'_3 + sigma_3/k_size) = 0;
        //
        //
        // LinearCombination::new("g_2", vec![(F::one(), g_2)]);
        //
        // LinearCombination::new(
        //     "a_denom".into(),
        //     vec![
        //         (alpha * beta, LCTerm::One),
        //         (alpha, "a_row"),
        //         (beta, "a_col"),
        //         (F::one(), "a_row_col"),
        // ]);
        // LinearCombination::new(
        //     "b-denom".into(),
        //     vec![
        //         (alpha * beta, LCTerm::One),
        //         (alpha, "b_row"),
        //         (beta, "b_col"),
        //         (F::one(), "b_row_col"),
        // ]);
        // LinearCombination::new(
        //     "c_denom".into(),
        //     vec![
        //         (alpha * beta, LCTerm::one()),
        //         (alpha, "c_row"),
        //         (beta, "c_col"),
        //         (F::one(), "c_row_col"),
        // ]);
        //
        // LinearCombination::new(
        //     "a_poly".into(),
        //     vec![
        //          (eta_a * b_denom_at_gamma * c_denom_at_gamma, "a_val".into()),
        //          (eta_b * a_denom_at_gamma * c_denom_at_gamma, "b_val".into()),
        //          (eta_c * b_denom_at_gamma * a_denom_at_gamma, "c_val".into()),
        //     ],
        // )
        //
        // let v_H_at_alpha = domain_h.evaluate_vanishing_polynomial(alpha);
        // let v_H_at_beta = domain_h.evaluate_vanishing_polynomial(beta);
        // let v_K_at_gamma = domain_k.evaluate_vanishing_polynomial(gamma);
        //
        // let a_poly_lc *= v_H_at_alpha * v_H_at_beta;
        // let b_lc = LinearCombination::new("b_poly", vec![(a_denom_at_gamma * b_denom_at_gamma * c_denom_at_gamma, "one")]);
        // let h_lc = LinearCombination::new("b_poly", vec![(v_K_at_gamma, "h_2")]);
        //
        // // This LC is the only one that is evaluated:
        // let inner_sumcheck = a_poly_lc - (b_lc * (gamma * &g_2_at_gamma + &(t_at_beta / &k_size))) - h_lc
        // main_lc.set_label("inner_sumcheck");
        query_set.insert(("g_2".into(), gamma));
        query_set.insert(("a_denom".into(), gamma));
        query_set.insert(("b_denom".into(), gamma));
        query_set.insert(("c_denom".into(), gamma));
        query_set.insert(("inner_sumcheck".into(), gamma));

        if with_vanishing {
            query_set.insert(("vanishing_poly_h_alpha".into(), alpha));
            query_set.insert(("vanishing_poly_h_beta".into(), beta));
            query_set.insert(("vanishing_poly_k_gamma".into(), gamma));
        }

        (query_set, state)
    }
}
