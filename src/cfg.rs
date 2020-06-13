use currency::Currency;
use num::{rational::Ratio, traits::{Zero, One, ToPrimitive}, integer::Integer, bigint::{BigUint}};
use structopt::StructOpt;
use isocountry::CountryCode;
use maplit::hashmap;
use std::{collections::HashMap, iter::Extend, ops::{Bound, RangeBounds}, cell::RefCell};

use crate::{brackets::MaritalStatus, loc::Location};

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum AnalysisMode {
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

#[derive(structopt::StructOpt)]
#[derive(Debug)]
pub struct Opts {
    pub source: Location,
    pub target: Location,
    pub income: Currency,
    pub status: MaritalStatus,
    #[structopt(default_value, long)]
    pub usage: AnalysisMode,
}
