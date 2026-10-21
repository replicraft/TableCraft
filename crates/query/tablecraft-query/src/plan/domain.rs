//! Closed read-only domain function identities; no host/application dispatch.
use super::{Aggregate, Statistic};
#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum DomainFunction {
    Average,
    Count,
    First,
    Last,
    Lookup,
    Minimum,
    Maximum,
    Sum,
    SampleVariance,
    PopulationVariance,
    SampleDeviation,
    PopulationDeviation,
}
impl DomainFunction {
    pub(crate) fn resolve(name: &str) -> Option<Self> {
        [
            ("DAvg", Self::Average),
            ("DCount", Self::Count),
            ("DFirst", Self::First),
            ("DLast", Self::Last),
            ("DLookup", Self::Lookup),
            ("DMin", Self::Minimum),
            ("DMax", Self::Maximum),
            ("DSum", Self::Sum),
            ("DVar", Self::SampleVariance),
            ("DVarP", Self::PopulationVariance),
            ("DStDev", Self::SampleDeviation),
            ("DStDevP", Self::PopulationDeviation),
        ]
        .into_iter()
        .find_map(|(label, kind)| name.eq_ignore_ascii_case(label).then_some(kind))
    }
    pub(crate) fn aggregate(self) -> Aggregate {
        match self {
            Self::Average => Aggregate::Average,
            Self::Count => Aggregate::Count,
            Self::First | Self::Lookup => Aggregate::First,
            Self::Last => Aggregate::Last,
            Self::Minimum => Aggregate::Minimum,
            Self::Maximum => Aggregate::Maximum,
            Self::Sum => Aggregate::Sum,
            Self::SampleVariance => Aggregate::Statistic(Statistic::SampleVariance),
            Self::PopulationVariance => Aggregate::Statistic(Statistic::PopulationVariance),
            Self::SampleDeviation => Aggregate::Statistic(Statistic::SampleDeviation),
            Self::PopulationDeviation => Aggregate::Statistic(Statistic::DomainPopulationDeviation),
        }
    }
}
