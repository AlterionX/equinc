use num::{bigint::BigUint, traits::ToPrimitive};
use structopt::StructOpt;

mod brackets;
mod cfg;
mod citizen;
mod loc;
mod logger;
mod util;

use cfg::Opts;
use citizen::Citizen;
use util::{BigUR, ApproxRatio};

fn main() {
    logger::setup().expect("the logger to intialize properly.");

    let opts = Opts::from_args();
    log::info!("Attempting to process arguments: {:?}", opts);
    let (sign, income) = opts.income.value().clone().to_bytes_le();
    if sign == currency_num::bigint::Sign::Minus {
        panic!("Unexpected negative income. Terminating.");
    }
    let (sign, monthly_expenses) = opts.monthly_expenses.value().clone().to_bytes_le();
    if sign == currency_num::bigint::Sign::Minus {
        panic!("Unexpected negative expenses. Terminating.");
    }
    let income = BigUint::from_bytes_le(income.as_slice());
    let monthly_expenses = BigUint::from_bytes_le(monthly_expenses.as_slice());

    let citizen = Citizen {
        income: BigUR::from_integer(income) / BigUint::from(100u8),
        monthly_expenses: BigUR::from_integer(monthly_expenses) / BigUint::from(100u8),
        status: opts.status,
        home: opts.source,
    };
    log::debug!("Citizen created: {:?}", citizen);
    let target = opts.target;
    let mode = opts.usage;

    let equivalent_income = citizen.estimate_equivalent_income_at(&target, mode);
    log::info!("Equivalent income deduced to be: {}.", equivalent_income);

    // TODO allow for other symbols.
    let currency_symbol = '$';
    println!("Total earned   : {}{}", currency_symbol, ApproxRatio(citizen.income.clone()));
    println!("Taxes at home  : {}{}", currency_symbol, ApproxRatio(citizen.calc_taxes()));
    println!("Taxes at target: {}{}", currency_symbol, ApproxRatio(citizen.calc_taxes_at(&target)));

    println!(
        r#"Estimated equivalent income at new location:
    raw output: {}
    total: {sym}{}"#,
        equivalent_income,
        ApproxRatio(equivalent_income.clone()),
        sym = currency_symbol,
    );
}
