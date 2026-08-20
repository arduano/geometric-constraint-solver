// SPDX-License-Identifier: GPL-3.0-or-later

const MAX_REDUCTION_INDEX: f64 = 4_503_599_627_370_496.0;
const FULL_TRIG_RANGE: Interval = Interval {
    lower: -1.0,
    upper: 1.0,
};

pub(super) const QUARTER_PI_INTERVAL: Interval = Interval {
    lower: f64::from_bits(0x3fe9_21fb_5444_2d18),
    upper: f64::from_bits(0x3fe9_21fb_5444_2d19),
};
pub(super) const HALF_PI_INTERVAL: Interval = Interval {
    lower: f64::from_bits(0x3ff9_21fb_5444_2d18),
    upper: f64::from_bits(0x3ff9_21fb_5444_2d19),
};
pub(super) const PI_INTERVAL: Interval = Interval {
    lower: f64::from_bits(0x4009_21fb_5444_2d18),
    upper: f64::from_bits(0x4009_21fb_5444_2d19),
};
pub(super) const TAU_INTERVAL: Interval = Interval {
    lower: f64::from_bits(0x4019_21fb_5444_2d18),
    upper: f64::from_bits(0x4019_21fb_5444_2d19),
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TranscendentalError {
    NonFinite,
    Overflow,
    UndefinedAngle,
    AmbiguousAngle,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Interval {
    pub lower: f64,
    pub upper: f64,
}

impl Interval {
    pub const ZERO: Self = Self {
        lower: 0.0,
        upper: 0.0,
    };
    pub const ONE: Self = Self {
        lower: 1.0,
        upper: 1.0,
    };

    pub fn point(value: f64) -> Self {
        Self {
            lower: value,
            upper: value,
        }
    }

    pub fn hull(first: f64, second: f64) -> Self {
        Self {
            lower: first.min(second),
            upper: first.max(second),
        }
    }

    pub fn checked(lower: f64, upper: f64) -> Option<Self> {
        (lower.is_finite() && upper.is_finite() && lower <= upper).then_some(Self { lower, upper })
    }

    pub fn is_finite(self) -> bool {
        self.lower.is_finite() && self.upper.is_finite() && self.lower <= self.upper
    }

    pub fn contains(self, value: f64) -> bool {
        self.lower <= value && value <= self.upper
    }

    pub fn contains_zero(self) -> bool {
        self.lower <= 0.0 && self.upper >= 0.0
    }

    pub fn excludes_zero(self) -> bool {
        self.upper < 0.0 || self.lower > 0.0
    }

    pub fn midpoint(self) -> f64 {
        self.lower + 0.5 * (self.upper - self.lower)
    }

    pub fn width(self) -> f64 {
        self.upper - self.lower
    }

    pub fn intersection(self, other: Self) -> Option<Self> {
        Self::checked(self.lower.max(other.lower), self.upper.min(other.upper))
    }

    pub fn include(self, other: Self) -> Self {
        Self {
            lower: self.lower.min(other.lower),
            upper: self.upper.max(other.upper),
        }
    }

    pub fn overlaps(self, other: Self) -> bool {
        self.lower <= other.upper && other.lower <= self.upper
    }

    pub fn interior_contains(self, other: Self) -> bool {
        self.lower < other.lower && other.upper < self.upper
    }

    pub fn add(self, other: Self) -> Self {
        Self {
            lower: next_down(self.lower + other.lower),
            upper: next_up(self.upper + other.upper),
        }
    }

    pub fn sub(self, other: Self) -> Self {
        Self {
            lower: next_down(self.lower - other.upper),
            upper: next_up(self.upper - other.lower),
        }
    }

    pub fn neg(self) -> Self {
        Self {
            lower: -self.upper,
            upper: -self.lower,
        }
    }

    pub fn mul(self, other: Self) -> Self {
        let products = [
            self.lower * other.lower,
            self.lower * other.upper,
            self.upper * other.lower,
            self.upper * other.upper,
        ];
        let lower = products.into_iter().fold(f64::INFINITY, f64::min);
        let upper = products.into_iter().fold(f64::NEG_INFINITY, f64::max);
        Self {
            lower: next_down(lower),
            upper: next_up(upper),
        }
    }

    pub fn square(self) -> Self {
        if self.contains_zero() {
            Self {
                lower: 0.0,
                upper: next_up(self.lower.abs().max(self.upper.abs()).powi(2)),
            }
        } else {
            let first = self.lower * self.lower;
            let second = self.upper * self.upper;
            Self {
                lower: next_down(first.min(second)),
                upper: next_up(first.max(second)),
            }
        }
    }

    pub fn powi(self, exponent: u32) -> Self {
        (0..exponent).fold(Self::ONE, |value, _| value.mul(self))
    }

    pub fn reciprocal(self) -> Option<Self> {
        if self.contains_zero() {
            return None;
        }
        Some(Self {
            lower: next_down(1.0 / self.upper),
            upper: next_up(1.0 / self.lower),
        })
    }

    pub fn div(self, other: Self) -> Option<Self> {
        Some(self.mul(other.reciprocal()?))
    }

    pub fn sqrt(self) -> Option<Self> {
        if !self.is_finite() || self.lower < 0.0 {
            return None;
        }
        Some(Self {
            lower: if self.lower == 0.0 {
                0.0
            } else {
                next_down(self.lower.sqrt()).max(0.0)
            },
            upper: next_up(self.upper.sqrt()),
        })
    }

    pub fn scale(self, value: f64) -> Self {
        self.mul(Self::point(value))
    }

    pub fn scalar_product(first: f64, second: f64) -> Self {
        Self::point(first).mul(Self::point(second))
    }

    pub fn sinh(self) -> Result<Self, TranscendentalError> {
        if !self.is_finite() {
            return Err(TranscendentalError::NonFinite);
        }
        let lower = sinh_point(self.lower)?;
        let upper = sinh_point(self.upper)?;
        Self::checked(lower.lower, upper.upper).ok_or(TranscendentalError::Overflow)
    }

    pub fn cosh(self) -> Result<Self, TranscendentalError> {
        if !self.is_finite() {
            return Err(TranscendentalError::NonFinite);
        }
        let maximum = self.lower.abs().max(self.upper.abs());
        let minimum = if self.contains_zero() {
            0.0
        } else {
            self.lower.abs().min(self.upper.abs())
        };
        let (_, lower) = positive_hyperbolic_point(minimum)?;
        let (_, upper) = positive_hyperbolic_point(maximum)?;
        Self::checked(lower.lower.max(1.0), upper.upper).ok_or(TranscendentalError::Overflow)
    }

    pub fn sin(self) -> Result<Self, TranscendentalError> {
        trig_range(self, true)
    }

    pub fn cos(self) -> Result<Self, TranscendentalError> {
        trig_range(self, false)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct Polynomial {
    pub coefficients: Vec<Interval>,
}

impl Polynomial {
    pub fn zero() -> Self {
        Self {
            coefficients: vec![Interval::ZERO],
        }
    }

    pub fn constant(value: Interval) -> Self {
        Self {
            coefficients: vec![value],
        }
    }

    pub fn linear(constant: Interval, slope: Interval) -> Self {
        Self {
            coefficients: vec![constant, slope],
        }
    }

    pub fn from_coefficients(coefficients: Vec<Interval>) -> Self {
        let mut value = Self { coefficients };
        value.trim();
        value
    }

    pub fn degree(&self) -> usize {
        self.coefficients.len().saturating_sub(1)
    }

    pub fn add(&self, other: &Self) -> Self {
        let len = self.coefficients.len().max(other.coefficients.len());
        let mut coefficients = vec![Interval::ZERO; len];
        for (index, coefficient) in coefficients.iter_mut().enumerate() {
            *coefficient = self
                .coefficients
                .get(index)
                .copied()
                .unwrap_or(Interval::ZERO)
                .add(
                    other
                        .coefficients
                        .get(index)
                        .copied()
                        .unwrap_or(Interval::ZERO),
                );
        }
        Self::from_coefficients(coefficients)
    }

    pub fn sub(&self, other: &Self) -> Self {
        self.add(&other.scale(Interval::point(-1.0)))
    }

    pub fn mul(&self, other: &Self) -> Self {
        let mut coefficients =
            vec![Interval::ZERO; self.coefficients.len() + other.coefficients.len() - 1];
        for (first_index, first) in self.coefficients.iter().copied().enumerate() {
            for (second_index, second) in other.coefficients.iter().copied().enumerate() {
                let index = first_index + second_index;
                coefficients[index] = coefficients[index].add(first.mul(second));
            }
        }
        Self::from_coefficients(coefficients)
    }

    pub fn scale(&self, value: Interval) -> Self {
        Self::from_coefficients(
            self.coefficients
                .iter()
                .copied()
                .map(|coefficient| coefficient.mul(value))
                .collect(),
        )
    }

    pub fn derivative(&self) -> Self {
        if self.coefficients.len() <= 1 {
            return Self::zero();
        }
        Self::from_coefficients(
            self.coefficients
                .iter()
                .copied()
                .enumerate()
                .skip(1)
                .map(|(index, coefficient)| coefficient.scale(index_as_f64(index)))
                .collect(),
        )
    }

    pub fn integral(&self) -> Self {
        let mut coefficients = Vec::with_capacity(self.coefficients.len() + 1);
        coefficients.push(Interval::ZERO);
        coefficients.extend(self.coefficients.iter().copied().enumerate().map(
            |(index, coefficient)| {
                coefficient.mul(
                    Interval::ONE
                        .div(Interval::point(index_as_f64(index + 1)))
                        .expect("positive polynomial degree excludes zero"),
                )
            },
        ));
        Self::from_coefficients(coefficients)
    }

    pub fn evaluate(&self, parameter: Interval) -> Interval {
        self.coefficients
            .iter()
            .copied()
            .rev()
            .fold(Interval::ZERO, |value, coefficient| {
                value.mul(parameter).add(coefficient)
            })
    }

    pub fn evaluate_point(&self, parameter: f64) -> Interval {
        self.evaluate(Interval::point(parameter))
    }

    /// Uses the convex hull of interval Bernstein coefficients on the selected interval.
    pub fn bezier_bound(&self, parameter: Interval) -> Interval {
        let composed = self.compose_affine(
            Interval::point(parameter.lower),
            Interval::point(parameter.upper).sub(Interval::point(parameter.lower)),
        );
        let degree = composed.degree();
        if degree == 0 {
            return composed.coefficients[0];
        }
        let mut lower = f64::INFINITY;
        let mut upper = f64::NEG_INFINITY;
        for k in 0..=degree {
            let mut coefficient = Interval::ZERO;
            for i in 0..=k {
                let factor = binomial(k, i)
                    .div(binomial(degree, i))
                    .expect("positive binomial coefficient excludes zero");
                coefficient = coefficient.add(composed.coefficients[i].mul(factor));
            }
            lower = lower.min(coefficient.lower);
            upper = upper.max(coefficient.upper);
        }
        Interval {
            lower: next_down(lower),
            upper: next_up(upper),
        }
    }

    pub fn compose_affine(&self, offset: Interval, scale: Interval) -> Self {
        let affine = Self::linear(offset, scale);
        self.coefficients
            .iter()
            .copied()
            .rev()
            .fold(Self::zero(), |value, coefficient| {
                value.mul(&affine).add(&Self::constant(coefficient))
            })
    }

    fn trim(&mut self) {
        while self.coefficients.len() > 1
            && self.coefficients.last().copied() == Some(Interval::ZERO)
        {
            self.coefficients.pop();
        }
    }
}

pub(super) fn polynomial_from_bernstein(values: &[Interval]) -> Polynomial {
    let degree = values.len() - 1;
    let mut coefficients = vec![Interval::ZERO; values.len()];
    for (index, coefficient) in coefficients.iter_mut().enumerate() {
        let outer = binomial(degree, index);
        for (control, value) in values.iter().copied().take(index + 1).enumerate() {
            let signed_outer = if (index - control).is_multiple_of(2) {
                outer
            } else {
                outer.neg()
            };
            let factor = signed_outer.mul(binomial(index, control));
            *coefficient = coefficient.add(value.mul(factor));
        }
    }
    Polynomial::from_coefficients(coefficients)
}

pub(super) fn cross_interval(first: [Interval; 2], second: [Interval; 2]) -> Interval {
    first[0].mul(second[1]).sub(first[1].mul(second[0]))
}

pub(super) fn next_down(value: f64) -> f64 {
    if value.is_nan() || value == f64::NEG_INFINITY {
        return value;
    }
    if value == 0.0 {
        return -f64::from_bits(1);
    }
    if value > 0.0 {
        f64::from_bits(value.to_bits() - 1)
    } else {
        f64::from_bits(value.to_bits() + 1)
    }
}

pub(super) fn next_up(value: f64) -> f64 {
    if value.is_nan() || value == f64::INFINITY {
        return value;
    }
    if value == 0.0 {
        return f64::from_bits(1);
    }
    if value > 0.0 {
        f64::from_bits(value.to_bits() + 1)
    } else {
        f64::from_bits(value.to_bits() - 1)
    }
}

pub(super) fn atan2_point(y: f64, x: f64) -> Result<Interval, TranscendentalError> {
    if !x.is_finite() || !y.is_finite() {
        return Err(TranscendentalError::NonFinite);
    }
    if x == 0.0 && y == 0.0 {
        return Err(TranscendentalError::UndefinedAngle);
    }
    if y == 0.0 {
        return if x > 0.0 {
            Ok(Interval::point(y))
        } else if y.is_sign_negative() {
            Ok(PI_INTERVAL.neg())
        } else {
            Ok(PI_INTERVAL)
        };
    }
    if x == 0.0 {
        return Ok(if y > 0.0 {
            HALF_PI_INTERVAL
        } else {
            HALF_PI_INTERVAL.neg()
        });
    }

    let x_abs = x.abs();
    let y_abs = y.abs();
    let base = if y_abs <= x_abs {
        atan_nonnegative(positive_ratio(y_abs, x_abs)?)
    } else {
        HALF_PI_INTERVAL.sub(atan_nonnegative(positive_ratio(x_abs, y_abs)?))
    };
    Ok(match (x.is_sign_positive(), y.is_sign_positive()) {
        (true, true) => base,
        (false, true) => PI_INTERVAL.sub(base),
        (false, false) => base.sub(PI_INTERVAL),
        (true, false) => base.neg(),
    })
}

pub(super) fn atan2_box(y: Interval, x: Interval) -> Result<Interval, TranscendentalError> {
    if !x.is_finite() || !y.is_finite() {
        return Err(TranscendentalError::NonFinite);
    }
    if x.contains_zero() && y.contains_zero() {
        return Err(TranscendentalError::UndefinedAngle);
    }
    let lift_negative_axis = x.upper < 0.0 && y.contains_zero();
    let mut result: Option<Interval> = None;
    for x_value in [x.lower, x.upper] {
        for y_value in [y.lower, y.upper] {
            let mut angle = atan2_point(y_value, x_value)?;
            if lift_negative_axis
                && (y_value < 0.0 || (y_value == 0.0 && y_value.is_sign_negative()))
            {
                angle = angle.add(TAU_INTERVAL);
            }
            result = Some(result.map_or(angle, |current| current.include(angle)));
        }
    }
    let result = result.ok_or(TranscendentalError::UndefinedAngle)?;
    if result.width() >= PI_INTERVAL.lower {
        return Err(TranscendentalError::AmbiguousAngle);
    }
    Ok(result)
}

#[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
fn trig_range(interval: Interval, sine: bool) -> Result<Interval, TranscendentalError> {
    if !interval.is_finite() {
        return Err(TranscendentalError::NonFinite);
    }
    let width = interval.upper - interval.lower;
    if !width.is_finite() || width >= TAU_INTERVAL.lower {
        return Ok(FULL_TRIG_RANGE);
    }
    let Some(quotient) = interval.div(HALF_PI_INTERVAL) else {
        return Ok(FULL_TRIG_RANGE);
    };
    if !quotient.is_finite() {
        return Ok(FULL_TRIG_RANGE);
    }
    let candidate_indices = quotient.add(Interval {
        lower: -0.5,
        upper: 0.5,
    });
    let first = candidate_indices.lower.ceil();
    let last = candidate_indices.upper.floor();
    if !first.is_finite()
        || !last.is_finite()
        || first > last
        || first.abs() > MAX_REDUCTION_INDEX
        || last.abs() > MAX_REDUCTION_INDEX
        || last - first > 8.0
    {
        return Ok(FULL_TRIG_RANGE);
    }

    let reduced_domain = Interval {
        lower: -QUARTER_PI_INTERVAL.upper,
        upper: QUARTER_PI_INTERVAL.upper,
    };
    let mut result: Option<Interval> = None;
    let first = first as i64;
    let last = last as i64;
    for index in first..=last {
        let translated = interval.sub(HALF_PI_INTERVAL.scale(index as f64));
        let Some(reduced) = translated.intersection(reduced_domain) else {
            continue;
        };
        let (reduced_sine, reduced_cosine) = trig_kernel(reduced);
        let quadrant = index.rem_euclid(4);
        let value = if sine {
            match quadrant {
                0 => reduced_sine,
                1 => reduced_cosine,
                2 => reduced_sine.neg(),
                _ => reduced_cosine.neg(),
            }
        } else {
            match quadrant {
                0 => reduced_cosine,
                1 => reduced_sine.neg(),
                2 => reduced_cosine.neg(),
                _ => reduced_sine,
            }
        };
        result = Some(result.map_or(value, |current| current.include(value)));
    }
    Ok(result.unwrap_or(FULL_TRIG_RANGE))
}

fn trig_kernel(argument: Interval) -> (Interval, Interval) {
    let square = argument.square();
    let mut sine_polynomial = signed_reciprocal_factorial(17, true);
    for term in (0_u32..8).rev() {
        sine_polynomial = sine_polynomial.mul(square).add(signed_reciprocal_factorial(
            2 * term + 1,
            term.is_multiple_of(2),
        ));
    }
    let mut cosine_polynomial = signed_reciprocal_factorial(18, false);
    for term in (0_u32..9).rev() {
        cosine_polynomial = cosine_polynomial
            .mul(square)
            .add(signed_reciprocal_factorial(
                2 * term,
                term.is_multiple_of(2),
            ));
    }
    let maximum = argument.lower.abs().max(argument.upper.abs());
    let sine_error = positive_power(maximum, 19).mul(reciprocal_factorial(19));
    let cosine_error = positive_power(maximum, 20).mul(reciprocal_factorial(20));
    (
        clamp_unit(add_symmetric_error(
            argument.mul(sine_polynomial),
            sine_error.upper,
        )),
        clamp_unit(add_symmetric_error(cosine_polynomial, cosine_error.upper)),
    )
}

fn positive_hyperbolic_point(value: f64) -> Result<(Interval, Interval), TranscendentalError> {
    if !value.is_finite() || value < 0.0 {
        return Err(TranscendentalError::NonFinite);
    }
    if value == 0.0 {
        return Ok((Interval::point(value), Interval::ONE));
    }
    let mut reduced = value;
    let mut halvings = 0_usize;
    while reduced > 0.5 {
        reduced *= 0.5;
        halvings = halvings
            .checked_add(1)
            .ok_or(TranscendentalError::Overflow)?;
    }
    let (mut sine, mut cosine) = small_positive_hyperbolic(reduced);
    for _ in 0..halvings {
        let doubled_sine = sine.mul(cosine).scale(2.0);
        let doubled_cosine = cosine.square().add(sine.square());
        if !doubled_sine.is_finite() || !doubled_cosine.is_finite() {
            return Err(TranscendentalError::Overflow);
        }
        sine = Interval {
            lower: doubled_sine.lower.max(0.0),
            upper: doubled_sine.upper,
        };
        cosine = Interval {
            lower: doubled_cosine.lower.max(1.0),
            upper: doubled_cosine.upper,
        };
    }
    Ok((sine, cosine))
}

fn sinh_point(value: f64) -> Result<Interval, TranscendentalError> {
    if value == 0.0 {
        return Ok(Interval::point(value));
    }
    let (sine, _) = positive_hyperbolic_point(value.abs())?;
    Ok(if value.is_sign_negative() {
        sine.neg()
    } else {
        sine
    })
}

fn small_positive_hyperbolic(value: f64) -> (Interval, Interval) {
    let argument = Interval::point(value);
    let square = argument.square();
    let mut sine_polynomial = reciprocal_factorial(17);
    for term in (0_u32..8).rev() {
        sine_polynomial = sine_polynomial
            .mul(square)
            .add(reciprocal_factorial(2 * term + 1));
    }
    let mut cosine_polynomial = reciprocal_factorial(18);
    for term in (0_u32..9).rev() {
        cosine_polynomial = cosine_polynomial
            .mul(square)
            .add(reciprocal_factorial(2 * term));
    }
    let tail_factor = Interval::point(1000.0)
        .div(Interval::point(999.0))
        .expect("positive tail denominator excludes zero");
    let sine_error = positive_power(value, 19)
        .mul(reciprocal_factorial(19))
        .mul(tail_factor);
    let cosine_error = positive_power(value, 20)
        .mul(reciprocal_factorial(20))
        .mul(tail_factor);
    let sine = argument.mul(sine_polynomial);
    (
        Interval {
            lower: sine.lower.max(0.0),
            upper: next_up(sine.upper + sine_error.upper),
        },
        Interval {
            lower: cosine_polynomial.lower.max(1.0),
            upper: next_up(cosine_polynomial.upper + cosine_error.upper),
        },
    )
}

fn positive_ratio(numerator: f64, denominator: f64) -> Result<Interval, TranscendentalError> {
    if !numerator.is_finite()
        || !denominator.is_finite()
        || numerator < 0.0
        || denominator <= 0.0
        || numerator > denominator
    {
        return Err(TranscendentalError::NonFinite);
    }
    let quotient = numerator / denominator;
    Ok(Interval {
        lower: next_down(quotient).max(0.0),
        upper: next_up(quotient).min(1.0),
    })
}

fn atan_nonnegative(argument: Interval) -> Interval {
    if argument.upper <= 0.5 {
        return atan_kernel(argument);
    }
    let transformed = argument
        .sub(Interval::ONE)
        .div(argument.add(Interval::ONE))
        .expect("positive atan transform denominator excludes zero");
    QUARTER_PI_INTERVAL.add(atan_kernel(transformed))
}

fn atan_kernel(argument: Interval) -> Interval {
    let square = argument.square();
    let mut polynomial = rational(1, 61);
    for term in (0_u32..30).rev() {
        let denominator = 2 * term + 1;
        let coefficient = if term.is_multiple_of(2) {
            rational(1, denominator)
        } else {
            rational(1, denominator).neg()
        };
        polynomial = polynomial.mul(square).add(coefficient);
    }
    let maximum = argument.lower.abs().max(argument.upper.abs());
    let error = positive_power(maximum, 63).mul(rational(1, 63));
    add_symmetric_error(argument.mul(polynomial), error.upper)
}

fn signed_reciprocal_factorial(degree: u32, positive: bool) -> Interval {
    let value = reciprocal_factorial(degree);
    if positive { value } else { value.neg() }
}

fn reciprocal_factorial(degree: u32) -> Interval {
    (2..=degree).fold(Interval::ONE, |value, factor| {
        value
            .div(Interval::point(f64::from(factor)))
            .expect("positive factorial factor excludes zero")
    })
}

fn rational(numerator: u32, denominator: u32) -> Interval {
    Interval::point(f64::from(numerator))
        .div(Interval::point(f64::from(denominator)))
        .expect("positive rational denominator excludes zero")
}

fn positive_power(value: f64, exponent: u32) -> Interval {
    Interval::point(value).powi(exponent)
}

fn add_symmetric_error(value: Interval, error: f64) -> Interval {
    value.add(Interval {
        lower: -error,
        upper: error,
    })
}

fn clamp_unit(value: Interval) -> Interval {
    Interval {
        lower: value.lower.max(-1.0),
        upper: value.upper.min(1.0),
    }
}

fn binomial(n: usize, k: usize) -> Interval {
    if k > n {
        return Interval::ZERO;
    }
    let k = k.min(n - k);
    (1..=k).fold(Interval::ONE, |value, index| {
        value
            .mul(Interval::point(index_as_f64(n - k + index)))
            .div(Interval::point(index_as_f64(index)))
            .expect("positive integer denominator excludes zero")
    })
}

fn index_as_f64(value: usize) -> f64 {
    f64::from(u32::try_from(value).expect("bounded polynomial degree fits in u32"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn certified_trigonometric_ranges_cover_quadrants_and_fail_wide() {
        for value in [
            -std::f64::consts::TAU,
            -std::f64::consts::FRAC_PI_2,
            -0.25,
            0.0,
            0.25,
            std::f64::consts::FRAC_PI_2,
            std::f64::consts::PI,
            std::f64::consts::TAU,
            1_048_576.0,
        ] {
            let sine = Interval::point(value).sin().unwrap();
            let cosine = Interval::point(value).cos().unwrap();
            assert!(sine.contains(value.sin()), "sin({value}) = {sine:?}");
            assert!(cosine.contains(value.cos()), "cos({value}) = {cosine:?}");
        }
        assert_eq!(Interval::point(f64::MAX).sin().unwrap(), FULL_TRIG_RANGE);
        let extremum = Interval::hull(
            next_down(std::f64::consts::FRAC_PI_2),
            next_up(std::f64::consts::FRAC_PI_2),
        )
        .sin()
        .unwrap();
        assert_eq!(extremum.upper.to_bits(), 1.0_f64.to_bits());
    }

    #[test]
    fn certified_hyperbolic_ranges_cover_finite_values_and_reject_overflow() {
        for value in [-10.0, -0.5, -f64::MIN_POSITIVE, 0.0, 0.5, 10.0, 700.0] {
            let sine = Interval::point(value).sinh().unwrap();
            let cosine = Interval::point(value).cosh().unwrap();
            assert!(sine.contains(value.sinh()), "sinh({value}) = {sine:?}");
            assert!(cosine.contains(value.cosh()), "cosh({value}) = {cosine:?}");
        }
        assert_eq!(
            Interval::point(f64::MAX).cosh(),
            Err(TranscendentalError::Overflow)
        );
    }

    #[test]
    fn certified_atan2_handles_axes_quadrants_and_negative_axis_boxes() {
        for (y, x) in [
            (0.0, 1.0),
            (1.0, 0.0),
            (1.0, 1.0),
            (1.0, -1.0),
            (-1.0, -1.0),
            (-1.0, 1.0),
            (-0.0, -1.0),
        ] {
            let angle = atan2_point(y, x).unwrap();
            assert!(angle.contains(y.atan2(x)), "atan2({y}, {x}) = {angle:?}");
        }
        let lifted = atan2_box(Interval::hull(-0.1, 0.1), Interval::hull(-2.0, -1.0)).unwrap();
        assert!(lifted.lower > 3.0);
        assert!(lifted.upper < 3.3);
        assert_eq!(
            atan2_box(Interval::hull(-1.0, 1.0), Interval::hull(-1.0, 1.0)),
            Err(TranscendentalError::UndefinedAngle)
        );
    }
}
