pub use isocountry::CountryCode;
use maplit::hashmap;
use num::traits::Zero;
use std::{cell::RefCell, collections::HashMap};

use crate::brackets::{MaritalStatus, TaxSystem};
use crate::util::*;

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

pub fn country_tax_system(country: &CountryCode) -> Option<TaxSystem> {
    match country {
        CountryCode::USA => Some(usa_tax_system()),
        _ => panic!("Tax rates not implemented for country {:?}.", country),
    }
}

#[non_exhaustive]
#[derive(Debug, Copy, Clone, Hash, Eq, PartialEq)]
pub enum State {
    CA,
    TX,
}

impl State {
    fn tax_system(self) -> Option<TaxSystem> {
        match self {
            Self::CA => {
                let taxes_by_bracket = vec![
                    UR64::new(1_1, 1_000),
                    UR64::new(2_2, 1_000),
                    UR64::new(4_4, 1_000),
                    UR64::new(6_6, 1_000),
                    UR64::new(8_8, 1_000),
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
            }
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
        "San Francisco" | "SF" => Some(TaxSystem::flat(UR64::new(15, 1000))),
        "Austin" | "AUS" => None,
        _ => panic!("Tax rates not implemented for city {:?}.", city.as_ref()),
    }
}

// TODO Currently USA specific, but perhaps expand later?
// TODO think about iso3166-2
#[derive(Debug)]
pub struct Location {
    pub country: CountryCode,
    pub state: State,
    pub city: String,
    // TODO cache the final tax bracket
    cached_merged_tax_bracket: RefCell<Option<TaxSystem>>,
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

    pub fn calc_taxes(&self, gross: &BigUR, status: MaritalStatus) -> BigUR {
        self.tax_system()
            .map_or_else(|| BigUR::zero(), |sys| sys.calc_taxes(gross, status))
    }

    pub fn calc_net(&self, gross: &BigUR, status: MaritalStatus) -> BigUR {
        self.tax_system()
            .map_or_else(|| gross.clone(), |sys| sys.calc_net(gross, status))
    }

    pub fn calc_gross(&self, net: &BigUR, status: MaritalStatus) -> BigUR {
        self.tax_system()
            .map_or_else(|| net.clone(), |sys| sys.calc_gross(net, status))
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
