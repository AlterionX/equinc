use currency::Currency;
use num::{rational::Ratio, traits::{Zero, One, ToPrimitive}, integer::Integer, bigint::{BigUint}};
use structopt::StructOpt;
use isocountry::CountryCode;
use maplit::hashmap;
use std::{collections::HashMap, iter::Extend, ops::{Bound, RangeBounds}, cell::RefCell};

mod logger;
mod util;
mod brackets;
mod loc;
mod cfg;
mod citizen;

use util::BigUR;
use cfg::Opts;
use citizen::Citizen;

fn main() {
    logger::setup().expect("the logger to intialize properly.");

    let opts = Opts::from_args();
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
