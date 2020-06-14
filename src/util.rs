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

pub struct ApproxRatio<T>(pub Ratio<T>);

impl<T: Clone + std::fmt::Display + Integer + From<u8>> std::fmt::Display for ApproxRatio<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let r = self.0.clone();
        let ratio_100: Ratio<T> = Ratio::from_integer(100u8.into());
        let trunc = r.to_integer();
        let fract = (r.fract() * ratio_100.clone() % ratio_100).to_integer();
        write!(f, "{}.{:02} (approx)", trunc, fract)
    }
}