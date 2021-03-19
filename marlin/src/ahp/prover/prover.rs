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
    ahp::{
        indexer::{Circuit, CircuitInfo, Matrix},
        prover::ProverConstraintSystem,
        verifier::{VerifierFirstMessage, VerifierSecondMessage},
        AHPError,
        AHPForR1CS,
        UnnormalizedBivariateLagrangePoly,
    },
    prover::{state::ProverState, ProverMessage},
    ToString,
    Vec,
};
use snarkvm_algorithms::{
    cfg_into_iter,
    cfg_iter,
    cfg_iter_mut,
    fft::{EvaluationDomain, Evaluations as EvaluationsOnDomain},
};
use snarkvm_fields::{batch_inversion, Field, PrimeField};
use snarkvm_r1cs::errors::SynthesisError;

use snarkvm_polycommit::{LabeledPolynomial, Polynomial};
use snarkvm_r1cs::ConstraintSynthesizer;

use rand_core::RngCore;

#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// The first set of prover oracles.
pub struct ProverFirstOracles<F: Field> {
    /// The LDE of `w`.
    pub w: LabeledPolynomial<F>,
    /// The LDE of `Az`.
    pub z_a: LabeledPolynomial<F>,
    /// The LDE of `Bz`.
    pub z_b: LabeledPolynomial<F>,
    /// The sum-check hiding polynomial.
    pub mask_poly: LabeledPolynomial<F>,
}

impl<F: Field> ProverFirstOracles<F> {
    /// Iterate over the polynomials output by the prover in the first round.
    pub fn iter(&self) -> impl Iterator<Item = &LabeledPolynomial<F>> {
        vec![&self.w, &self.z_a, &self.z_b, &self.mask_poly].into_iter()
    }
}

/// The second set of prover oracles.
pub struct ProverSecondOracles<F: Field> {
    /// The polynomial `t` that is produced in the first round.
    pub t: LabeledPolynomial<F>,
    /// The polynomial `g` resulting from the first sumcheck.
    pub g_1: LabeledPolynomial<F>,
    /// The polynomial `h` resulting from the first sumcheck.
    pub h_1: LabeledPolynomial<F>,
}

impl<F: Field> ProverSecondOracles<F> {
    /// Iterate over the polynomials output by the prover in the second round.
    pub fn iter(&self) -> impl Iterator<Item = &LabeledPolynomial<F>> {
        vec![&self.t, &self.g_1, &self.h_1].into_iter()
    }
}

/// The third set of prover oracles.
pub struct ProverThirdOracles<F: Field> {
    /// The polynomial `g` resulting from the second sumcheck.
    pub g_2: LabeledPolynomial<F>,
    /// The polynomial `h` resulting from the second sumcheck.
    pub h_2: LabeledPolynomial<F>,
}

impl<F: Field> ProverThirdOracles<F> {
    /// Iterate over the polynomials output by the prover in the third round.
    pub fn iter(&self) -> impl Iterator<Item = &LabeledPolynomial<F>> {
        vec![&self.g_2, &self.h_2].into_iter()
    }
}

impl<F: PrimeField> AHPForR1CS<F> {
    /// Initialize the AHP prover.
    pub fn prover_init<'a, C: ConstraintSynthesizer<F>>(
        index: &'a Circuit<F>,
        circuit: &C,
    ) -> Result<ProverState<'a, F>, AHPError> {
        let init_time = start_timer!(|| "AHP::Prover::Init");

        let constraint_time = start_timer!(|| "Generating constraints and witnesses");
        let mut pcs = ProverConstraintSystem::new();
        circuit.generate_constraints(&mut pcs)?;
        end_timer!(constraint_time);

        let padding_time = start_timer!(|| "Padding matrices to make them square");
        crate::ahp::matrices::pad_input_for_indexer_and_prover(&mut pcs);
        pcs.make_matrices_square();
        end_timer!(padding_time);

        let num_non_zero = index.index_info.num_non_zero;

        let ProverConstraintSystem {
            public_variables: padded_public_variables,
            private_variables,
            num_constraints,
            num_public_variables,
            num_private_variables,
            ..
        } = pcs;

        if index.index_info.num_constraints != num_constraints
            || index.index_info.num_variables != (num_public_variables + num_private_variables)
        {
            return Err(AHPError::InstanceDoesNotMatchIndex);
        }

        if !Self::formatted_public_input_is_admissible(&padded_public_variables) {
            return Err(AHPError::InvalidPublicInputLength);
        }

        // Perform matrix multiplications.
        let inner_product = |row: &[(F, usize)]| {
            let mut result = F::zero();

            for &(ref coefficient, i) in row {
                // Fetch the variable.
                let variable = match i < num_public_variables {
                    true => padded_public_variables[i],
                    false => private_variables[i - num_public_variables],
                };

                result += &(if coefficient.is_one() {
                    variable
                } else {
                    variable * coefficient
                });
            }

            result
        };

        let eval_z_a_time = start_timer!(|| "Evaluating z_A");
        let z_a = index.a.iter().map(|row| inner_product(row)).collect();
        end_timer!(eval_z_a_time);

        let eval_z_b_time = start_timer!(|| "Evaluating z_B");
        let z_b = index.b.iter().map(|row| inner_product(row)).collect();
        end_timer!(eval_z_b_time);

        let zk_bound = 1; // One query is sufficient for our desired soundness

        let domain_h = EvaluationDomain::new(num_constraints).ok_or(SynthesisError::PolynomialDegreeTooLarge)?;

        let domain_k = EvaluationDomain::new(num_non_zero).ok_or(SynthesisError::PolynomialDegreeTooLarge)?;

        let domain_x = EvaluationDomain::new(num_public_variables).ok_or(SynthesisError::PolynomialDegreeTooLarge)?;

        end_timer!(init_time);

        Ok(ProverState {
            padded_public_variables,
            private_variables,
            z_a: Some(z_a),
            z_b: Some(z_b),
            w_poly: None,
            mz_polys: None,
            zk_bound,
            index,
            verifier_first_message: None,
            mask_poly: None,
            domain_h,
            domain_k,
            domain_x,
        })
    }

    /// Output the first round message and the next state.
    #[allow(clippy::type_complexity)]
    pub fn prover_first_round<'a, R: RngCore>(
        mut state: ProverState<'a, F>,
        rng: &mut R,
        hiding: bool,
    ) -> Result<(ProverMessage<F>, ProverFirstOracles<F>, ProverState<'a, F>), AHPError> {
        let round_time = start_timer!(|| "AHP::Prover::FirstRound");
        let domain_h = state.domain_h;
        let zk_bound = state.zk_bound;

        let v_H = domain_h.vanishing_polynomial().into();

        let x_time = start_timer!(|| "Computing x polynomial and evals");
        let domain_x = state.domain_x;
        let x_poly =
            EvaluationsOnDomain::from_vec_and_domain(state.padded_public_variables.clone(), domain_x).interpolate();
        let x_evals = domain_h.fft(&x_poly);
        end_timer!(x_time);

        let ratio = domain_h.size() / domain_x.size();

        let mut w_extended = state.private_variables.clone();
        w_extended.extend(vec![
            F::zero();
            domain_h.size() - domain_x.size() - state.private_variables.len()
        ]);

        let w_poly_time = start_timer!(|| "Computing w polynomial");
        let w_poly_evals = cfg_into_iter!(0..domain_h.size())
            .map(|k| {
                if k % ratio == 0 {
                    F::zero()
                } else {
                    w_extended[k - (k / ratio) - 1] - &x_evals[k]
                }
            })
            .collect();

        let w_poly = &EvaluationsOnDomain::from_vec_and_domain(w_poly_evals, domain_h).interpolate()
            + &(&Polynomial::from_coefficients_slice(&[F::rand(rng)]) * &v_H);
        let (w_poly, remainder) = w_poly.divide_by_vanishing_poly(domain_x).unwrap();
        assert!(remainder.is_zero());
        end_timer!(w_poly_time);

        let z_a_poly_time = start_timer!(|| "Computing z_A polynomial");
        let z_a = state.z_a.clone().unwrap();
        let z_a_poly = &EvaluationsOnDomain::from_vec_and_domain(z_a, domain_h).interpolate()
            + &(&Polynomial::from_coefficients_slice(&[F::rand(rng)]) * &v_H);
        end_timer!(z_a_poly_time);

        let z_b_poly_time = start_timer!(|| "Computing z_B polynomial");
        let z_b = state.z_b.clone().unwrap();
        let z_b_poly = &EvaluationsOnDomain::from_vec_and_domain(z_b, domain_h).interpolate()
            + &(&Polynomial::from_coefficients_slice(&[F::rand(rng)]) * &v_H);
        end_timer!(z_b_poly_time);

        let mask_poly_time = start_timer!(|| "Computing mask polynomial");
        let mask_poly_degree = 3 * domain_h.size() + 2 * zk_bound - 3;
        let mut mask_poly = Polynomial::rand(mask_poly_degree, rng);
        let scaled_sigma_1 = (mask_poly.divide_by_vanishing_poly(domain_h).unwrap().1)[0];
        mask_poly[0] -= &scaled_sigma_1;
        end_timer!(mask_poly_time);

        let msg = ProverMessage::default();

        assert!(w_poly.degree() < domain_h.size() - domain_x.size() + zk_bound);
        assert!(z_a_poly.degree() < domain_h.size() + zk_bound);
        assert!(z_b_poly.degree() < domain_h.size() + zk_bound);
        assert!(mask_poly.degree() <= 3 * domain_h.size() + 2 * zk_bound - 3);

        let (w, z_a, z_b) = if hiding {
            (
                LabeledPolynomial::new("w".to_string(), w_poly, None, Some(1)),
                LabeledPolynomial::new("z_a".to_string(), z_a_poly, None, Some(1)),
                LabeledPolynomial::new("z_b".to_string(), z_b_poly, None, Some(1)),
            )
        } else {
            (
                LabeledPolynomial::new("w".to_string(), w_poly, None, None),
                LabeledPolynomial::new("z_a".to_string(), z_a_poly, None, None),
                LabeledPolynomial::new("z_b".to_string(), z_b_poly, None, None),
            )
        };

        let mask_poly = LabeledPolynomial::new_owned("mask_poly".to_string(), mask_poly, None, None);

        let oracles = ProverFirstOracles {
            w: w.clone(),
            z_a: z_a.clone(),
            z_b: z_b.clone(),
            mask_poly: mask_poly.clone(),
        };

        state.w_poly = Some(w);
        state.mz_polys = Some((z_a, z_b));
        state.mask_poly = Some(mask_poly);
        end_timer!(round_time);

        Ok((msg, oracles, state))
    }

    fn calculate_t<'a>(
        matrices: impl Iterator<Item = &'a Matrix<F>>,
        matrix_randomizers: &[F],
        input_domain: EvaluationDomain<F>,
        domain_h: EvaluationDomain<F>,
        r_alpha_x_on_h: Vec<F>,
    ) -> Polynomial<F> {
        let mut t_evals_on_h = vec![F::zero(); domain_h.size()];
        for (matrix, eta) in matrices.zip(matrix_randomizers) {
            for (r, row) in matrix.iter().enumerate() {
                for (coeff, c) in row.iter() {
                    let index = domain_h.reindex_by_subdomain(input_domain, *c);
                    t_evals_on_h[index] += &(*eta * coeff * &r_alpha_x_on_h[r]);
                }
            }
        }
        EvaluationsOnDomain::from_vec_and_domain(t_evals_on_h, domain_h).interpolate()
    }

    /// Output the number of oracles sent by the prover in the first round.
    pub fn prover_num_first_round_oracles() -> usize {
        4
    }

    /// Output the degree bounds of oracles in the first round.
    pub fn prover_first_round_degree_bounds(_info: &CircuitInfo<F>) -> impl Iterator<Item = Option<usize>> {
        vec![None; 4].into_iter()
    }

    /// Output the second round message and the next state.
    pub fn prover_second_round<'a, R: RngCore>(
        verifier_message: &VerifierFirstMessage<F>,
        mut state: ProverState<'a, F>,
        _r: &mut R,
        hiding: bool,
    ) -> (ProverMessage<F>, ProverSecondOracles<F>, ProverState<'a, F>) {
        let round_time = start_timer!(|| "AHP::Prover::SecondRound");

        let domain_h = state.domain_h;
        let zk_bound = state.zk_bound;

        let mask_poly = state
            .mask_poly
            .as_ref()
            .expect("ProverState should include mask_poly when prover_second_round is called");

        let VerifierFirstMessage {
            alpha,
            eta_a,
            eta_b,
            eta_c,
        } = *verifier_message;

        let summed_z_m_poly_time = start_timer!(|| "Compute z_m poly");
        let (z_a_poly, z_b_poly) = state.mz_polys.as_ref().unwrap();
        let z_c_poly = z_a_poly.polynomial() * z_b_poly.polynomial();

        let mut summed_z_m_coeffs = z_c_poly.coeffs;
        // Note: Can't combine these two loops, because z_c_poly has 2x the degree
        // of z_a_poly and z_b_poly, so the second loop gets truncated due to
        // the `zip`s.
        cfg_iter_mut!(summed_z_m_coeffs).for_each(|c| *c *= &eta_c);
        cfg_iter_mut!(summed_z_m_coeffs)
            .zip(&z_a_poly.polynomial().coeffs)
            .zip(&z_b_poly.polynomial().coeffs)
            .for_each(|((c, a), b)| *c += &(eta_a * a + &(eta_b * b)));

        let summed_z_m = Polynomial::from_coefficients_vec(summed_z_m_coeffs);
        end_timer!(summed_z_m_poly_time);

        let r_alpha_x_evals_time = start_timer!(|| "Compute r_alpha_x evals");
        let r_alpha_x_evals = domain_h.batch_eval_unnormalized_bivariate_lagrange_poly_with_diff_inputs(alpha);
        end_timer!(r_alpha_x_evals_time);

        let r_alpha_poly_time = start_timer!(|| "Compute r_alpha_x poly");
        let r_alpha_poly = Polynomial::from_coefficients_vec(domain_h.ifft(&r_alpha_x_evals));
        end_timer!(r_alpha_poly_time);

        let t_poly_time = start_timer!(|| "Compute t poly");
        let t_poly = Self::calculate_t(
            vec![&state.index.a, &state.index.b, &state.index.c].into_iter(),
            &[eta_a, eta_b, eta_c],
            state.domain_x,
            state.domain_h,
            r_alpha_x_evals,
        );
        end_timer!(t_poly_time);

        let z_poly_time = start_timer!(|| "Compute z poly");

        let domain_x = EvaluationDomain::new(state.padded_public_variables.len())
            .ok_or(SynthesisError::PolynomialDegreeTooLarge)
            .unwrap();
        let x_poly =
            EvaluationsOnDomain::from_vec_and_domain(state.padded_public_variables.clone(), domain_x).interpolate();
        let w_poly = state.w_poly.as_ref().unwrap();
        let mut z_poly = w_poly.polynomial().mul_by_vanishing_poly(domain_x);
        cfg_iter_mut!(z_poly.coeffs)
            .zip(&x_poly.coeffs)
            .for_each(|(z, x)| *z += x);
        assert!(z_poly.degree() < domain_h.size() + zk_bound);

        end_timer!(z_poly_time);

        let q_1_time = start_timer!(|| "Compute q_1 poly");

        let mul_domain_size = *[
            mask_poly.len(),
            r_alpha_poly.coeffs.len() + summed_z_m.coeffs.len(),
            t_poly.coeffs.len() + z_poly.len(),
        ]
        .iter()
        .max()
        .unwrap();
        let mul_domain =
            EvaluationDomain::new(mul_domain_size).expect("field is not smooth enough to construct domain");
        let mut r_alpha_evals = r_alpha_poly.evaluate_over_domain_by_ref(mul_domain);
        let summed_z_m_evals = summed_z_m.evaluate_over_domain_by_ref(mul_domain);
        let z_poly_evals = z_poly.evaluate_over_domain_by_ref(mul_domain);
        let t_poly_m_evals = t_poly.evaluate_over_domain_by_ref(mul_domain);

        cfg_iter_mut!(r_alpha_evals.evaluations)
            .zip(&summed_z_m_evals.evaluations)
            .zip(&z_poly_evals.evaluations)
            .zip(&t_poly_m_evals.evaluations)
            .for_each(|(((a, b), &c), d)| {
                *a *= &b;
                *a -= &(c * d);
            });
        let rhs = r_alpha_evals.interpolate();
        let q_1 = mask_poly.polynomial() + &rhs;
        end_timer!(q_1_time);

        let sumcheck_time = start_timer!(|| "Compute sumcheck h and g polys");
        let (h_1, x_g_1) = q_1.divide_by_vanishing_poly(domain_h).unwrap();
        let g_1 = Polynomial::from_coefficients_slice(&x_g_1.coeffs[1..]);
        end_timer!(sumcheck_time);

        let msg = ProverMessage::default();

        assert!(g_1.degree() <= domain_h.size() - 2);
        assert!(h_1.degree() <= 2 * domain_h.size() + 2 * zk_bound - 2);

        let oracles = if hiding {
            ProverSecondOracles {
                t: LabeledPolynomial::new("t".into(), t_poly, None, None),
                g_1: LabeledPolynomial::new("g_1".into(), g_1, Some(domain_h.size() - 2), Some(1)),
                h_1: LabeledPolynomial::new("h_1".into(), h_1, None, None),
            }
        } else {
            ProverSecondOracles {
                t: LabeledPolynomial::new("t".into(), t_poly, None, None),
                g_1: LabeledPolynomial::new("g_1".into(), g_1, Some(domain_h.size() - 2), None),
                h_1: LabeledPolynomial::new("h_1".into(), h_1, None, None),
            }
        };

        state.w_poly = None;
        state.verifier_first_message = Some(*verifier_message);
        end_timer!(round_time);

        (msg, oracles, state)
    }

    /// Output the number of oracles sent by the prover in the second round.
    pub fn prover_num_second_round_oracles() -> usize {
        3
    }

    /// Output the degree bounds of oracles in the second round.
    pub fn prover_second_round_degree_bounds(info: &CircuitInfo<F>) -> impl Iterator<Item = Option<usize>> {
        let h_domain_size = EvaluationDomain::<F>::compute_size_of_domain(info.num_constraints).unwrap();

        vec![None, Some(h_domain_size - 2), None].into_iter()
    }

    /// Output the third round message and the next state.
    pub fn prover_third_round<'a, R: RngCore>(
        verifier_message: &VerifierSecondMessage<F>,
        prover_state: ProverState<'a, F>,
        _r: &mut R,
    ) -> Result<(ProverMessage<F>, ProverThirdOracles<F>), AHPError> {
        let round_time = start_timer!(|| "AHP::Prover::ThirdRound");

        let ProverState {
            index,
            verifier_first_message,
            domain_h,
            domain_k,
            ..
        } = prover_state;

        let VerifierFirstMessage {
            eta_a,
            eta_b,
            eta_c,
            alpha,
        } = verifier_first_message
            .expect("ProverState should include verifier_first_msg when prover_third_round is called");

        let beta = verifier_message.beta;

        let v_H_at_alpha = domain_h.evaluate_vanishing_polynomial(alpha);
        let v_H_at_beta = domain_h.evaluate_vanishing_polynomial(beta);

        let (a_star, b_star, c_star) = (&index.a_star_arith, &index.b_star_arith, &index.c_star_arith);

        let f_evals_time = start_timer!(|| "Computing f evals on K");
        let mut f_vals_on_K = Vec::with_capacity(domain_k.size());
        let mut inverses_a = Vec::with_capacity(domain_k.size());
        let mut inverses_b = Vec::with_capacity(domain_k.size());
        let mut inverses_c = Vec::with_capacity(domain_k.size());

        for i in 0..domain_k.size() {
            inverses_a.push((beta - &a_star.evals_on_K.row[i]) * &(alpha - &a_star.evals_on_K.col[i]));
            inverses_b.push((beta - &b_star.evals_on_K.row[i]) * &(alpha - &b_star.evals_on_K.col[i]));
            inverses_c.push((beta - &c_star.evals_on_K.row[i]) * &(alpha - &c_star.evals_on_K.col[i]));
        }
        batch_inversion(&mut inverses_a);
        batch_inversion(&mut inverses_b);
        batch_inversion(&mut inverses_c);

        for i in 0..domain_k.size() {
            let t = eta_a * &a_star.evals_on_K.val[i] * &inverses_a[i]
                + &(eta_b * &b_star.evals_on_K.val[i] * &inverses_b[i])
                + &(eta_c * &c_star.evals_on_K.val[i] * &inverses_c[i]);
            let f_at_kappa = v_H_at_beta * &v_H_at_alpha * &t;
            f_vals_on_K.push(f_at_kappa);
        }
        end_timer!(f_evals_time);

        let f_poly_time = start_timer!(|| "Computing f poly");
        let f = EvaluationsOnDomain::from_vec_and_domain(f_vals_on_K, domain_k).interpolate();
        end_timer!(f_poly_time);

        let g_2 = Polynomial::from_coefficients_slice(&f.coeffs[1..]);

        let domain_b =
            EvaluationDomain::<F>::new(3 * domain_k.size() - 3).ok_or(SynthesisError::PolynomialDegreeTooLarge)?;

        let denom_eval_time = start_timer!(|| "Computing denominator evals on B");
        let a_denom: Vec<_> = cfg_iter!(a_star.evals_on_B.row.evaluations)
            .zip(&a_star.evals_on_B.col.evaluations)
            .zip(&a_star.row_col_evals_on_B.evaluations)
            .map(|((&r, c), r_c)| beta * &alpha - &(r * &alpha) - &(beta * c) + r_c)
            .collect();

        let b_denom: Vec<_> = cfg_iter!(b_star.evals_on_B.row.evaluations)
            .zip(&b_star.evals_on_B.col.evaluations)
            .zip(&b_star.row_col_evals_on_B.evaluations)
            .map(|((&r, c), r_c)| beta * &alpha - &(r * &alpha) - &(beta * c) + r_c)
            .collect();

        let c_denom: Vec<_> = cfg_iter!(c_star.evals_on_B.row.evaluations)
            .zip(&c_star.evals_on_B.col.evaluations)
            .zip(&c_star.row_col_evals_on_B.evaluations)
            .map(|((&r, c), r_c)| beta * &alpha - &(r * &alpha) - &(beta * c) + r_c)
            .collect();
        end_timer!(denom_eval_time);

        let a_evals_time = start_timer!(|| "Computing a evals on B");
        let a_poly_on_B = cfg_into_iter!(0..domain_b.size())
            .map(|i| {
                let t = eta_a * &a_star.evals_on_B.val.evaluations[i] * &b_denom[i] * &c_denom[i]
                    + &(eta_b * &b_star.evals_on_B.val.evaluations[i] * &a_denom[i] * &c_denom[i])
                    + &(eta_c * &c_star.evals_on_B.val.evaluations[i] * &a_denom[i] * &b_denom[i]);
                v_H_at_beta * &v_H_at_alpha * &t
            })
            .collect();
        end_timer!(a_evals_time);

        let a_poly_time = start_timer!(|| "Computing a poly");
        let a_poly = EvaluationsOnDomain::from_vec_and_domain(a_poly_on_B, domain_b).interpolate();
        end_timer!(a_poly_time);

        let b_evals_time = start_timer!(|| "Computing b evals on B");
        let b_poly_on_B = cfg_into_iter!(0..domain_b.size())
            .map(|i| a_denom[i] * &b_denom[i] * &c_denom[i])
            .collect();
        end_timer!(b_evals_time);

        let b_poly_time = start_timer!(|| "Computing b poly");
        let b_poly = EvaluationsOnDomain::from_vec_and_domain(b_poly_on_B, domain_b).interpolate();
        end_timer!(b_poly_time);

        let h_2_poly_time = start_timer!(|| "Computing sumcheck h poly");
        let h_2 = (&a_poly - &(&b_poly * &f))
            .divide_by_vanishing_poly(domain_k)
            .unwrap()
            .0;
        end_timer!(h_2_poly_time);

        let msg = ProverMessage::default();

        assert!(g_2.degree() <= domain_k.size() - 2);
        let oracles = ProverThirdOracles {
            g_2: LabeledPolynomial::new_owned("g_2".to_string(), g_2, Some(domain_k.size() - 2), None),
            h_2: LabeledPolynomial::new_owned("h_2".to_string(), h_2, None, None),
        };
        end_timer!(round_time);

        Ok((msg, oracles))
    }

    /// Output the number of oracles sent by the prover in the third round.
    pub fn prover_num_third_round_oracles() -> usize {
        3
    }

    /// Output the degree bounds of oracles in the third round.
    pub fn prover_third_round_degree_bounds(info: &CircuitInfo<F>) -> impl Iterator<Item = Option<usize>> {
        let num_non_zero = info.num_non_zero;
        let k_size = EvaluationDomain::<F>::compute_size_of_domain(num_non_zero).unwrap();

        vec![Some(k_size - 2), None].into_iter()
    }
}
