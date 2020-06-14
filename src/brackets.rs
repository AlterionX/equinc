use num::{
    bigint::BigUint,
    traits::{One, Zero},
};
use std::{
    collections::HashMap,
    iter::Extend,
    ops::{Bound, RangeBounds},
};

use crate::util::{cast_ratio, ApproxRatio, BigUR, UR64};

/// Assumes the list of separators are inclusive.
pub fn multibound_to_opts_iter<I, Iter, T>(i: I) -> impl Iterator<Item = (Option<T>, Option<T>)>
where
    I: IntoIterator<Item = T, IntoIter = Iter>,
    Iter: Clone + Iterator<Item = T>,
{
    let iter = i.into_iter();
    // Enum variant inference forces this to be typed since the assumed type is too stringent.
    let bot_iter = std::iter::once(None).chain(iter.clone().map(Some));
    let top_iter = iter.map(Some).chain(std::iter::once(None));
    bot_iter.zip(top_iter)
}

/// Assumes the list of separators are inclusive.
pub fn multibound_to_bounds_iter<I, Iter, T>(
    i: I,
    // If the boundary belongs to the previous section or next section
    // This should be true by default.
    inclusive_bounds: bool,
) -> impl Iterator<Item = (Bound<T>, Bound<T>)>
where
    I: IntoIterator<Item = T, IntoIter = Iter>,
    Iter: Clone + Iterator<Item = T>,
{
    // Enum variant inference forces this to be typed since the assumed type is too stringent.
    let (bot_map, top_map): (fn(T) -> Bound<T>, fn(T) -> Bound<T>) = if inclusive_bounds {
        (Bound::Included, Bound::Excluded)
    } else {
        (Bound::Excluded, Bound::Included)
    };

    let opt_iter = multibound_to_opts_iter(i);

    let opt_to_bound = move |opt, conv: fn(T) -> Bound<T>| match opt {
        Some(v) => conv(v),
        None => Bound::Unbounded,
    };
    let process_bounds = move |(top_opt, bot_opt)| {
        (
            opt_to_bound(top_opt, top_map),
            opt_to_bound(bot_opt, bot_map),
        )
    };

    opt_iter.map(process_bounds)
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum MaritalStatus {
    Single,
    Joint,
    Separate,
    HeadOfHousehold,
    // TODO California seems to have a "Widower with child" status, so what about other statuses?
}

impl std::str::FromStr for MaritalStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let status = match s.to_ascii_lowercase().as_str() {
            "single" => MaritalStatus::Single,
            "joint" => MaritalStatus::Joint,
            "separate" => MaritalStatus::Separate,
            "head" => MaritalStatus::HeadOfHousehold,
            _ => return Err(format!("Could not parse the marital status {:?}.", s)),
        };
        Ok(status)
    }
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
enum Side {
    LHS,
    RHS,
    Both,
}

#[derive(Debug, Clone)]
pub struct TaxBrackets {
    // n - 1 elements -- missing first
    /// The value is a vec of the bigint that is the inclusive upper bound of the tax bracket.
    /// These values are unique.
    separators: Vec<BigUR>,
    flats: Vec<BigUR>,
    // n elements
    rates: Vec<UR64>,
}

impl TaxBrackets {
    fn base(
        separators: impl Iterator<Item = impl Into<BigUint>>,
        rates: impl Iterator<Item = impl Into<UR64>>,
    ) -> Self {
        let separators = separators.map(Into::into);
        Self::new(separators, rates)
    }

    fn new(
        separators: impl Iterator<Item = impl Into<BigUR>>,
        rates: impl Iterator<Item = impl Into<UR64>>,
    ) -> Self {
        let separators: Vec<_> = separators.map(Into::into).collect();
        let rates: Vec<_> = rates.map(Into::into).collect();
        assert!(separators.len() + 1 == rates.len());
        let flats = {
            let mut flats = Vec::with_capacity(rates.len());
            for bracket in 0..rates.len() {
                let deduction = if bracket == 0 {
                    BigUR::zero()
                } else {
                    let diff = if bracket == 1 {
                        separators[bracket - 1].clone()
                    } else {
                        separators[bracket - 1].clone() - separators[bracket - 2].clone()
                    };
                    flats.last().map_or_else(BigUR::zero, Clone::clone)
                        + diff * cast_ratio(rates[bracket - 1])
                };
                flats.push(deduction);
            }
            flats
        };

        Self {
            separators,
            rates,
            flats,
        }
    }

    fn taxation_info<'a>(&'a self) -> impl 'a + Clone + Iterator<Item = (&'a BigUR, &'a UR64)> {
        self.flats.iter().zip(self.rates.iter())
    }

    fn calc_taxes(&self, gross: &BigUR) -> BigUR {
        let bounds_and_taxation_info =
            multibound_to_bounds_iter(self.separators.iter(), true).zip(self.taxation_info());
        for (bound, taxation_info) in bounds_and_taxation_info {
            log::info!("Bracket {:?}", bound);
            if bound.contains(gross) {
                let start_bound: Bound<&BigUR> = bound.start_bound();
                let amount_over = match start_bound {
                    Bound::Unbounded => gross.clone(),
                    Bound::Included(n) | Bound::Excluded(n) => gross.clone() - n.clone(),
                };
                let (flat, rate) = taxation_info;
                let taxes = flat + amount_over.clone() * cast_ratio(*rate);
                log::info!("Taxes for {} calced to be {} with rate {} on {} and bump {}.",
                    ApproxRatio(gross.clone()),
                    ApproxRatio(taxes.clone()),
                    ApproxRatio(rate.clone()),
                    ApproxRatio(amount_over.clone()),
                    ApproxRatio(flat.clone()),
                );
                return taxes;
            }
        }
        unreachable!("All bounds should be included by `multibound_to_bounds_iter`");
    }

    fn calc_net(&self, gross: &BigUR) -> BigUR {
        let taxed = self.calc_taxes(gross);
        if &taxed > gross {
            panic!("Tax bracket here causes taxes to exceed gross income!");
        }
        gross.clone() - taxed
    }

    fn separators_post_tax<'a>(&'a self) -> impl 'a + Clone + Iterator<Item = BigUR> {
        assert!(self.separators.len() + 1 == self.flats.len());
        self.separators
            .iter()
            .zip(self.flats.iter().skip(1))
            .map(|(sep, flat)| sep.clone() - flat.clone())
    }

    fn calc_gross(&self, net: &BigUR) -> BigUR {
        log::info!("Net                : {}", ApproxRatio(net.clone()));
        // Map pre tax ranges to post tax ranges
        let separators = self.separators_post_tax();

        let bounds_and_taxation_info =
            multibound_to_bounds_iter(separators, true).zip(self.taxation_info());
        for (bound, taxation_info) in bounds_and_taxation_info {
            log::info!("Bracket {:?}", bound);
            if bound.contains(net) {
                let (over_amount, prev_bucket) = match bound.start_bound() {
                    // No matter if the end bound is present or absent, the value is less than the top bound,
                    // so we simply take the entire income.
                    Bound::Unbounded => (net.clone(), BigUR::zero()),
                    // It's contained, so we tax everything in the range
                    Bound::Excluded(n) | Bound::Included(n) => (net.clone() - n.clone(), n.clone()),
                };
                log::info!("Over amount        : {}", ApproxRatio(over_amount.clone()));
                let (flat, rate) = taxation_info;
                let percentage_of_gross = UR64::one() - rate;
                log::info!("Flat deduction     : {}", ApproxRatio(flat.clone()));
                log::info!("Marginal rate      : {}", ApproxRatio(rate.clone()));
                log::info!("Percentage of gross: {}", ApproxRatio(percentage_of_gross.clone()));
                let gross = flat + prev_bucket + over_amount / cast_ratio(percentage_of_gross);
                log::info!("Gross              : {}", ApproxRatio(gross.clone()));
                return gross;
            }
        }
        unreachable!("All bounds should be included in the for loop.");
    }

    fn merge(lhs: Self, rhs: Self) -> Self {
        let Self {
            separators: lhs_brackets,
            rates: lhs_rates,
            ..
        } = lhs;
        let Self {
            separators: rhs_brackets,
            rates: rhs_rates,
            ..
        } = rhs;

        let (merge_order, merged_separators): (Vec<_>, Vec<_>) = {
            let mut lhs_brackets_iter = lhs_brackets.iter().peekable();
            let mut rhs_brackets_iter = rhs_brackets.iter().peekable();

            let mut merged_separators = Vec::with_capacity(lhs_brackets.len() * 2);
            while let (Some(lhs_bracket), Some(rhs_bracket)) =
                (lhs_brackets_iter.peek(), rhs_brackets_iter.peek())
            {
                if lhs_bracket == rhs_bracket {
                    merged_separators.push((Side::Both, *lhs_bracket));
                    lhs_brackets_iter.next();
                    rhs_brackets_iter.next();
                } else if lhs_bracket < rhs_bracket {
                    merged_separators.push((Side::LHS, *lhs_bracket));
                    lhs_brackets_iter.next();
                } else {
                    merged_separators.push((Side::RHS, *rhs_bracket));
                    rhs_brackets_iter.next();
                }
            }
            // Either lhs or rhs is empty, but it's hard to do this properly, so let's let someone else take care of it.
            merged_separators.extend(lhs_brackets_iter.map(|b| (Side::LHS, b)));
            merged_separators.extend(rhs_brackets_iter.map(|b| (Side::RHS, b)));

            merged_separators
                .into_iter()
                .map(|(side, b)| (side, b.clone()))
                .unzip()
        };

        let merged_rates = {
            let mut lhs_rates_iter = lhs_rates.iter();
            let mut rhs_rates_iter = rhs_rates.iter();

            let mut merged_rates = Vec::with_capacity(merge_order.len() + 1);

            // Start with the base rate
            let mut curr_lhs = *lhs_rates_iter
                .next()
                .expect("the lhs tax brackets have at least one bracket.");
            let mut curr_rhs = *rhs_rates_iter
                .next()
                .expect("the rhs tax brackets have at least one bracket.");
            merged_rates.push(curr_lhs.clone() + curr_rhs.clone());

            for side in merge_order {
                match side {
                    Side::LHS => {
                        curr_lhs = *lhs_rates_iter
                            .next()
                            .expect("Number of tax rates in the lhs to be correct.");
                    }
                    Side::RHS => {
                        curr_rhs = *rhs_rates_iter
                            .next()
                            .expect("Number of tax rates in the rhs to be correct.");
                    }
                    Side::Both => {
                        curr_lhs = *lhs_rates_iter
                            .next()
                            .expect("Number of tax rates in the lhs to be correct.");
                        curr_rhs = *rhs_rates_iter
                            .next()
                            .expect("Number of tax rates in the rhs to be correct.");
                    }
                }
                merged_rates.push(curr_lhs + curr_rhs);
            }
            merged_rates
        };

        // TODO Consider trying to merge flats instead of recalculating with `new`. This is probably hard.
        Self::new(merged_separators.into_iter(), merged_rates.into_iter())
    }
}

// TODO check if taxation is bijective. I think it is, but not sure.
#[derive(Debug, Clone)]
pub struct TaxSystem(HashMap<MaritalStatus, TaxBrackets>);

impl TaxSystem {
    pub fn new(
        brackets_by_status: HashMap<
            MaritalStatus,
            (
                impl IntoIterator<Item = impl Into<BigUint>>,
                impl IntoIterator<Item = impl Into<UR64>>,
            ),
        >,
    ) -> Self {
        let brackets = brackets_by_status
            .into_iter()
            .map(|(k, v)| {
                let (separators, rates) = v;
                let bracket = TaxBrackets::base(separators.into_iter(), rates.into_iter());
                (k, bracket)
            })
            .collect();
        Self(brackets)
    }

    pub fn flat(rate: UR64) -> Self {
        let statuses = [
            MaritalStatus::Single,
            MaritalStatus::Separate,
            MaritalStatus::Joint,
            MaritalStatus::HeadOfHousehold,
        ];
        let tax_brackets = TaxBrackets::base(Vec::<u64>::new().into_iter(), vec![rate].into_iter());
        let map = statuses.iter().map(|k| (*k, tax_brackets.clone()));
        Self(map.collect())
    }

    pub fn calc_taxes(&self, gross: &BigUR, status: MaritalStatus) -> BigUR {
        self.0
            .get(&status)
            .map_or_else(BigUR::zero, |b| b.calc_taxes(gross))
    }

    pub fn calc_net(&self, gross: &BigUR, status: MaritalStatus) -> BigUR {
        self.0
            .get(&status)
            .map_or_else(|| gross.clone(), |b| b.calc_net(gross))
    }

    pub fn calc_gross(&self, net: &BigUR, status: MaritalStatus) -> BigUR {
        self.0
            .get(&status)
            .map_or_else(|| net.clone(), |b| b.calc_gross(net))
    }

    pub fn merge(mut lhs: TaxSystem, mut rhs: TaxSystem) -> Self {
        let statuses = [
            MaritalStatus::Single,
            MaritalStatus::Separate,
            MaritalStatus::Joint,
            MaritalStatus::HeadOfHousehold,
        ];
        let new_tax_brackets =
            statuses
                .iter()
                .filter_map(|k| match (lhs.0.remove(k), rhs.0.remove(k)) {
                    (None, None) => None,
                    (None, Some(lone)) | (Some(lone), None) => Some((*k, lone)),
                    (Some(lhs), Some(rhs)) => Some((*k, TaxBrackets::merge(lhs, rhs))),
                });
        Self(new_tax_brackets.collect())
    }
}
