use crate::brackets::MaritalStatus;
use crate::cfg::AnalysisMode;
use crate::loc::Location;
use crate::util::BigUR;

#[derive(Debug)]
pub struct Citizen {
    // TODO consider specific currencies
    pub income: BigUR,
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
