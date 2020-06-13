use currency::Currency;
use num::{rational::Ratio, traits::{Zero, One, ToPrimitive}, integer::Integer, bigint::{BigUint}};
use structopt::StructOpt;
use isocountry::CountryCode;
use maplit::hashmap;
use std::{collections::HashMap, iter::Extend, ops::{Bound, RangeBounds}, cell::RefCell};

pub type UR64 = Ratio<u64>;
pub type BigUR = Ratio<BigUint>;

pub fn cast_ratio<I, O>(input: Ratio<I>) -> Ratio<O>
    where I: Into<O>,
          O: Clone + Integer,
{
    let (numer, denom): (I, I) = input.into();
    Ratio::new(numer.into(), denom.into())
}
