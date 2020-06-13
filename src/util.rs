use num::{bigint::BigUint, integer::Integer, rational::Ratio};

pub type UR64 = Ratio<u64>;
pub type BigUR = Ratio<BigUint>;

pub fn cast_ratio<I, O>(input: Ratio<I>) -> Ratio<O>
where
    I: Into<O>,
    O: Clone + Integer,
{
    let (numer, denom): (I, I) = input.into();
    Ratio::new(numer.into(), denom.into())
}
