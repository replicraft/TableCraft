//! Closed statistical aggregates and their documented minimum population sizes.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Statistic {
    SampleVariance,
    PopulationVariance,
    SampleDeviation,
    PopulationDeviation,
    DomainPopulationDeviation,
}
impl Statistic {
    pub(crate) fn denominator(self, count: u32) -> Option<u32> {
        match self {
            Self::SampleVariance | Self::SampleDeviation if count >= 2 => Some(count - 1),
            // Microsoft's Var/VarP reference specifies at least two records for
            // both functions; StDevP alone permits a singleton population.
            Self::PopulationVariance | Self::DomainPopulationDeviation if count >= 2 => Some(count),
            Self::PopulationDeviation if count >= 1 => Some(count),
            _ => None,
        }
    }
    pub(crate) fn deviation(self) -> bool {
        matches!(
            self,
            Self::SampleDeviation | Self::PopulationDeviation | Self::DomainPopulationDeviation
        )
    }
}
