use currency::Currency;
use num::{rational::Ratio, traits::{Zero, One, ToPrimitive}, integer::Integer, bigint::{BigUint}};
use structopt::StructOpt;
use isocountry::CountryCode;
use maplit::hashmap;
use std::{collections::HashMap, iter::Extend, ops::{Bound, RangeBounds}, cell::RefCell};

mod logger;

type UR64 = Ratio<u64>;
type BigUR = Ratio<BigUint>;

fn cast_ratio<I, O>(input: Ratio<I>) -> Ratio<O>
    where I: Into<O>,
          O: Clone + Integer,
{
    let (numer, denom): (I, I) = input.into();
    Ratio::new(numer.into(), denom.into())
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
enum MaritalStatus {
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

/// Assumes the list of separators are inclusive.
fn multibound_to_opts_iter<I, Iter, T>(i: I) -> impl Iterator<Item = (Option<T>, Option<T>)>
    where I: IntoIterator<Item = T, IntoIter = Iter>,
          Iter: Clone + Iterator<Item = T>,
{
    let iter = i.into_iter();
    // Enum variant inference forces this to be typed since the assumed type is too stringent.
    let bot_iter = std::iter::once(None).chain(iter.clone().map(Some));
    let top_iter = iter.map(Some).chain(std::iter::once(None));
    bot_iter.zip(top_iter)
}

/// Assumes the list of separators are inclusive.
fn multibound_to_bounds_iter<I, Iter, T>(
    i: I,
    // If the boundary belongs to the previous section or next section
    // This should be true by default.
    inclusive_bounds: bool,
) -> impl Iterator<Item = (Bound<T>, Bound<T>)>
    where I: IntoIterator<Item = T, IntoIter = Iter>,
          Iter: Clone + Iterator<Item = T>,
{
    // Enum variant inference forces this to be typed since the assumed type is too stringent.
    let (bot_map, top_map): (fn(T) -> Bound<T>, fn(T) -> Bound<T>) = if inclusive_bounds {
        (Bound::Included, Bound::Excluded)
    } else {
        (Bound::Excluded, Bound::Included)
    };

    let opt_iter = multibound_to_opts_iter(i);

    let opt_to_bound = move |opt, conv: fn(T) -> Bound<T>| {
        match opt {
            Some(v) => conv(v),
            None => Bound::Unbounded,
        }
    };
    let process_bounds = move |(top_opt, bot_opt)| {
        (opt_to_bound(top_opt, top_map), opt_to_bound(bot_opt, bot_map))
    };

    opt_iter.map(process_bounds)
}

#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
enum Side {
    LHS,
    RHS,
    Both,
}

#[derive(Debug, Clone)]
struct TaxBrackets {
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
            let zero = BigUR::zero();
            for bracket in 0..rates.len() {
                let deduction = if bracket == 0 {
                    BigUR::zero()
                } else {
                    let diff = if bracket == 1 {
                        separators[bracket - 1].clone()
                    } else {
                        separators[bracket - 1].clone() - separators[bracket - 2].clone()
                    };
                    flats.last().map_or_else(BigUR::zero, Clone::clone) + diff * cast_ratio(rates[bracket])
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
        let bounds_and_taxation_info = multibound_to_bounds_iter(self.separators.iter(), true)
            .zip(self.taxation_info());
        for (bound, taxation_info) in bounds_and_taxation_info {
            if bound.contains(gross) {
                let start_bound: Bound<&BigUR> = bound.start_bound();
                let amount_over = match start_bound {
                    Bound::Unbounded => gross.clone(),
                    Bound::Included(n) | Bound::Excluded(n) => gross.clone() - n.clone(),
                };
                let (flat, rate) = taxation_info;
                return flat + amount_over * cast_ratio(*rate);
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
        // Map pre tax ranges to post tax ranges
        let separators = self.separators_post_tax();

        let bounds_and_taxation_info = multibound_to_bounds_iter(separators, true)
            .zip(self.taxation_info());
        for (bound, taxation_info) in bounds_and_taxation_info {
            if bound.contains(net) {
                let over_amount = match bound.start_bound() {
                    // No matter if the end bound is present or absent, the value is less than the top bound,
                    // so we simply take the entire income.
                    Bound::Unbounded => net.clone(),
                    // It's contained, so we tax everything in the range
                    Bound::Excluded(n) | Bound::Included(n) => {
                        net.clone() - n.clone()
                    },
                };
                let (flat, rate) = taxation_info;
                let percentage_of_gross = UR64::one() - rate;
                return flat.clone() + over_amount / cast_ratio(percentage_of_gross);
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
            while let (Some(lhs_bracket), Some(rhs_bracket)) = (lhs_brackets_iter.peek(), rhs_brackets_iter.peek()) {
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

            merged_separators.into_iter().map(|(side, b)| (side, b.clone())).unzip()
        };

        let merged_rates = {
            let mut lhs_rates_iter = lhs_rates.iter().peekable();
            let mut rhs_rates_iter = rhs_rates.iter().peekable();

            let mut merged_rates = Vec::with_capacity(merge_order.len() + 1);

            // Start with the base rate
            let mut curr_lhs = *lhs_rates_iter.peek().expect("the lhs tax brackets have at least one bracket.");
            let mut curr_rhs = *rhs_rates_iter.peek().expect("the rhs tax brackets have at least one bracket.");
            merged_rates.push(curr_lhs.clone() + curr_rhs.clone());

            for side in merge_order {
                match side {
                    Side::LHS  => {
                        curr_lhs = lhs_rates_iter.next().expect("Number of tax rates in the lhs to be correct.");
                    },
                    Side::RHS  => {
                        curr_rhs = rhs_rates_iter.next().expect("Number of tax rates in the rhs to be correct.");
                    },
                    Side::Both => {
                        curr_lhs = lhs_rates_iter.next().expect("Number of tax rates in the lhs to be correct.");
                        curr_rhs = rhs_rates_iter.next().expect("Number of tax rates in the rhs to be correct.");
                    },
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
struct TaxSystem(HashMap<MaritalStatus, TaxBrackets>);

impl TaxSystem {
    fn new(brackets_by_status: HashMap<MaritalStatus, (impl IntoIterator<Item = impl Into<BigUint>>, impl IntoIterator<Item = impl Into<UR64>>)>) -> Self {
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

    fn flat(rate: UR64) -> Self {
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

    fn calc_taxes(&self, gross: &BigUR, status: MaritalStatus) -> BigUR {
        self.0.get(&status).map_or_else(BigUR::zero, |b| b.calc_taxes(gross))
    }

    fn calc_net(&self, gross: &BigUR, status: MaritalStatus) -> BigUR {
        self.0.get(&status).map_or_else(|| gross.clone(), |b| b.calc_net(gross))
    }

    fn calc_gross(&self, net: &BigUR, status: MaritalStatus) -> BigUR {
        self.0.get(&status).map_or_else(|| net.clone(), |b| b.calc_gross(net))
    }

    fn merge(mut lhs: TaxSystem, mut rhs: TaxSystem) -> Self {
        let statuses = [
            MaritalStatus::Single,
            MaritalStatus::Separate,
            MaritalStatus::Joint,
            MaritalStatus::HeadOfHousehold,
        ];
        let new_tax_brackets = statuses
            .iter()
            .filter_map(|k| {
                match (lhs.0.remove(k), rhs.0.remove(k)) {
                    (None, None) => None,
                    (None, Some(lone)) | (Some(lone), None) => Some((*k, lone)),
                    (Some(lhs), Some(rhs)) => Some((*k, TaxBrackets::merge(lhs, rhs))),
                }
            });
        Self(new_tax_brackets.collect())
    }
}

// TODO This can become `const` eventually.
fn usa_tax_system() -> TaxSystem {
    let taxes_by_bracket = vec![
        UR64::new(10, 100),
        UR64::new(12, 100),
        UR64::new(22, 100),
        UR64::new(24, 100),
        UR64::new(32, 100),
        UR64::new(35, 100),
        UR64::new(37, 100),
    ];
    let ranges_by_status: HashMap<_, (Vec<u64>, _)> = hashmap! {
        MaritalStatus::Single =>          (vec![ 9_875, 40_125,  85_525, 163_300, 207_350, 518_400], taxes_by_bracket.clone()),
        MaritalStatus::Joint =>           (vec![19_750, 80_250, 171_050, 326_600, 414_700, 622_050], taxes_by_bracket.clone()),
        MaritalStatus::Separate =>        (vec![ 9_875, 40_125,  85_525, 163_300, 207_350, 518_400], taxes_by_bracket.clone()),
        MaritalStatus::HeadOfHousehold => (vec![14_100, 53_700,  85_500, 163_300, 207_350, 518_400], taxes_by_bracket.clone()),
    };

    TaxSystem::new(ranges_by_status)
}

fn country_tax_system(country: &CountryCode) -> Option<TaxSystem> {
    match country {
        CountryCode::USA => Some(usa_tax_system()),
        _ => panic!("Tax rates not implemented for country {:?}.", country),
    }
}

#[non_exhaustive]
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
enum State {
    CA,
    TX,
}

impl State {
    fn tax_system(self) -> Option<TaxSystem> {
        match self {
            Self::CA => {
                let taxes_by_bracket = vec![
                    UR64::new( 1_1 ,  1_000),
                    UR64::new( 2_2 ,  1_000),
                    UR64::new( 4_4 ,  1_000),
                    UR64::new( 6_6 ,  1_000),
                    UR64::new( 8_8 ,  1_000),
                    UR64::new(10_23, 10_000),
                    UR64::new(11_33, 10_000),
                    UR64::new(12_43, 10_000),
                    UR64::new(13_53, 10_000),
                    UR64::new(14_63, 10_000),
                ];
                let brackets_by_status: HashMap<_, (Vec<u64>, _)> = hashmap! {
                    MaritalStatus::Single          => (vec![ 8_809, 20_883, 32_960, 45_753,  57_824, 295_373, 354_445,   590_742, 1_000_000], taxes_by_bracket.clone()),
                    MaritalStatus::Joint           => (vec![17_618, 41_766, 65_920, 91_506, 115_648, 590_746, 708_890, 1_000_000, 1_181_484], taxes_by_bracket.clone()),
                    MaritalStatus::Separate        => (vec![ 8_809, 20_883, 32_960, 45_753,  57_824, 295_373, 354_445,   590_742, 1_000_000], taxes_by_bracket.clone()),
                    MaritalStatus::HeadOfHousehold => (vec![17_629, 41_768, 53_843, 66_636,  78_710, 401_705, 482_047,   803_410, 1_000_000], taxes_by_bracket.clone()),
                };

                Some(TaxSystem::new(brackets_by_status))
            },
            Self::TX => None,
            #[allow(unreachable_patterns)]
            _ => panic!("Tax rates not implemented for state {:?}.", self),
        }
    }
}

impl std::str::FromStr for State {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "CA" | "California" => Ok(State::CA),
            "TX" | "Texas" => Ok(State::TX),
            _ => Err(format!("Could not parse country {:?}", s)),
        }
    }
}

fn city_tax_system<S: AsRef<str>>(city: S) -> Option<TaxSystem> {
    match city.as_ref() {
        "San Francisco" | "SF" => {
            Some(TaxSystem::flat(UR64::new(15, 1000)))
        },
        "Austin" | "AUS" => None,
        _ => panic!("Tax rates not implemented for city {:?}.", city.as_ref()),
    }
}

// TODO Currently USA specific, but perhaps expand later?
// TODO think about iso3166-2
#[derive(Debug)]
struct Location {
    country: CountryCode,
    state: State,
    city: String,
    // TODO cache the final tax bracket
    cached_merged_tax_bracket: RefCell<Option<TaxSystem>>
}

impl Location {
    fn tax_system(&self) -> Option<TaxSystem> {
        let brackets = vec![
            country_tax_system(&self.country),
            self.state.tax_system(),
            city_tax_system(self.city.as_str()),
        ];

        let mut merged = None;
        for (i, brackets) in brackets.into_iter().enumerate() {
            log::debug!("Merging bracket {}", i);
            if let Some(brackets) = brackets {
                if let Some(merged_brackets) = merged {
                    merged = Some(TaxSystem::merge(merged_brackets, brackets));
                } else {
                    merged = Some(brackets);
                }
            }
        }
        merged
    }

    fn calc_net(&self, gross: &BigUR, status: MaritalStatus) -> BigUR {
        self.tax_system().map_or_else(|| gross.clone(), |sys| sys.calc_net(gross, status))
    }

    fn calc_gross(&self, net: &BigUR, status: MaritalStatus) -> BigUR {
        self.tax_system().map_or_else(|| net.clone(), |sys| sys.calc_gross(net, status))
    }
}

impl std::str::FromStr for Location {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<_> = s.split("///").collect();
        let country = match parts[0] {
            "USA" | "United States" | "America" | "US" => Ok(CountryCode::USA),
            _ => Err(format!("Could not parse country {:?}", parts[0])),
        }?;
        Ok(Location {
            country: country,
            state: parts[1].parse()?,
            city: parts[2].to_owned(),
            cached_merged_tax_bracket: RefCell::new(None),
        })
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
enum AnalysisMode {
    PostTax,
    Disposable,
}

impl Default for AnalysisMode {
    fn default() -> Self {
        AnalysisMode::Disposable
    }
}

impl std::str::FromStr for AnalysisMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "post_tax" => Ok(AnalysisMode::PostTax),
            "disposable" => Ok(AnalysisMode::Disposable),
            _ => Err(format!("Failed to understand analysis mode {:?}.", s)),
        }
    }
}

impl std::fmt::Display for AnalysisMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnalysisMode::PostTax => write!(f, "post_tax"),
            AnalysisMode::Disposable => write!(f, "disposable"),
        }
    }
}

#[derive(Debug)]
struct Citizen {
    // TODO consider specific currencies
    income: BigUR,
    status: MaritalStatus,
    home: Location,
}

impl Citizen {
    fn estimate_equivalent_income_at(&self, target: &Location, mode: AnalysisMode) -> BigUR {
        let net = self.home.calc_net(&self.income, self.status);
        let target_pre_tax = target.calc_gross(&net, self.status);

        match mode {
            // Just do taxes, so stop here
            AnalysisMode::PostTax => target_pre_tax,
            AnalysisMode::Disposable => {
                // TODO calculate disposable income
                unimplemented!("Anaysis mode disposable income not yet functional.");
            }
        }
    }
}

#[derive(structopt::StructOpt)]
#[derive(Debug)]
struct Opts {
    source: Location,
    target: Location,
    income: Currency,
    status: MaritalStatus,
    #[structopt(default_value, long)]
    usage: AnalysisMode,
}

fn main() {
    logger::setup().expect("the logger to intialize properly.");

    let opts: Opts = Opts::from_args();
    log::info!("Attempting to process arguments: {:?}", opts);
    let (sign, income) = opts.income.value().clone().to_bytes_le();
    if sign == currency_num::bigint::Sign::Minus {
        panic!("Unexpected negative income. Closing down.");
    }
    let income = BigUint::from_bytes_le(income.as_slice());
    let citizen = Citizen {
        income: BigUR::from_integer(income),
        status: opts.status,
        home: opts.source,
    };
    let target = opts.target;
    let mode = opts.usage;

    let equivalent_income = citizen.estimate_equivalent_income_at(&target, mode);
    log::info!("Equivalent income deduced to be: {}.", equivalent_income);

    // TODO is there a better way to do this?
    let big_100 = BigUint::from(100u8);

    let bills = (equivalent_income.clone().trunc() / big_100.clone()).floor().to_integer();
    let coins = (equivalent_income.clone().trunc() % big_100.clone()).to_integer();
    let rem = equivalent_income.fract();
    println!(r#"Estimated equivalent income at new location:
    raw output: {}
    total: {}.{:02}
    rem fraction of a cent: {}"#,
        equivalent_income,
        bills.to_i32().unwrap_or(0),
        coins.to_i32().unwrap_or(0),
        rem
    );
}
