use num::BigUint;

use crate::brackets::MaritalStatus;
use crate::cfg::AnalysisMode;
use crate::loc::Location;
use crate::util::BigUR;

#[derive(Debug)]
pub struct Citizen {
    // TODO consider specific currencies
    pub income: BigUR,
    pub monthly_expenses: BigUR,
    pub status: MaritalStatus,
    pub home: Location,
}

impl Citizen {
    pub fn calc_taxes(&self) -> BigUR {
        self.home.calc_taxes(&self.income, self.status)
    }

    pub fn calc_taxes_at(&self, loc: &Location) -> BigUR {
        loc.calc_taxes(&self.income, self.status)
    }

    pub fn estimate_equivalent_income_at(&self, target: &Location, mode: AnalysisMode) -> BigUR {
        let net = self.home.calc_net(&self.income, self.status);

        match mode {
            // Just do taxes, so stop here
            AnalysisMode::PostTax => target.calc_gross(&net, self.status),
            AnalysisMode::Disposable => {
                // TODO calculate disposable income
                let annual_expenses =
                    BigUR::from_integer(BigUint::from(12u8)) * self.monthly_expenses.clone();
                if annual_expenses > net {
                    panic!("Annual expenses are higher than income. Please watch your spending!");
                }
                let disposable = net - annual_expenses.clone();
                let ratio = target.get_living_costs_factor() / self.home.get_living_costs_factor();
                let target_net = disposable + annual_expenses * ratio;
                target.calc_gross(&target_net, self.status)
            }
        }
    }
}
