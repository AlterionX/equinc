use currency::Currency;
use num::{rational::Ratio, traits::{Zero, One, ToPrimitive}, integer::Integer, bigint::{BigUint}};
use structopt::StructOpt;
use isocountry::CountryCode;
use maplit::hashmap;
use std::{collections::HashMap, iter::Extend, ops::{Bound, RangeBounds}, cell::RefCell};

use crate::util::BigUR;
use crate::brackets::{MaritalStatus, TaxSystem};
use crate::loc::Location;
use crate::cfg::AnalysisMode;

#[derive(Debug)]
pub struct Citizen {
    // TODO consider specific currencies
    pub income: BigUR,
    pub status: MaritalStatus,
    pub home: Location,
}

impl Citizen {
    pub fn estimate_equivalent_income_at(&self, target: &Location, mode: AnalysisMode) -> BigUR {
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
