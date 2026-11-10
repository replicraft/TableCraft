//! One closed original-request reconstruction service, without journal or target I/O.
use super::{
    NativeIntentError, NativeRecoveryQualificationError, NativeUpdateBody, NativeUpdateIntent,
    NativeUpdateProfile,
};
use crate::{NativeEditPlan, NativeImage, format::page::PAGE_BYTES, header::check};
use tablecraft_core::limits::ResourceBudget;

pub(super) fn rebuild(
    image: NativeImage,
    intent: &NativeUpdateIntent,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeEditPlan, NativeRecoveryQualificationError> {
    check(budget, cancelled)?;
    intent.verify_before(image.as_bytes(), budget, cancelled)?;
    rebuild_verified_binding(image, intent, budget, cancelled)
}

/// The caller verified this intent against the exact bytes used to capture
/// `image`, before allocation. Recovery retains that ordering without charging
/// the same digest traversal a second time.
pub(super) fn rebuild_verified_binding(
    image: NativeImage,
    intent: &NativeUpdateIntent,
    budget: &mut ResourceBudget,
    cancelled: &dyn Fn() -> bool,
) -> Result<NativeEditPlan, NativeRecoveryQualificationError> {
    let plan = match (intent.profile(), intent.body()) {
        #[cfg(feature = "ace16-create")]
        (
            NativeUpdateProfile::Ace16HistoricalShortTextUpdates,
            NativeUpdateBody::Fitted(original),
        ) => {
            let groups = original.borrowed_groups(budget, cancelled)?;
            image.plan_historical_short_text_updates(
                original.table(),
                &groups,
                budget,
                cancelled,
            )?
        }
        #[cfg(feature = "ace16-create")]
        (
            NativeUpdateProfile::Ace16HistoricalForwardedNumberMaterialization,
            NativeUpdateBody::Fixed(original),
        ) => {
            let groups = original.borrowed_groups(budget, cancelled)?;
            image.plan_historical_forwarded_number_materialization(
                original.table(),
                &groups,
                budget,
                cancelled,
            )?
        }
        #[cfg(feature = "ace16-create")]
        (
            NativeUpdateProfile::Ace16HistoricalMixedNumberMaterialization,
            NativeUpdateBody::Fixed(original),
        ) => {
            let groups = original.borrowed_groups(budget, cancelled)?;
            image.plan_historical_mixed_number_materialization(
                original.table(),
                &groups,
                budget,
                cancelled,
            )?
        }
        #[cfg(feature = "ace16-create")]
        (
            NativeUpdateProfile::Ace16HistoricalNumberMaterialization,
            NativeUpdateBody::Fixed(original),
        ) => {
            let groups = original.borrowed_groups(budget, cancelled)?;
            image.plan_historical_number_materialization(
                original.table(),
                &groups,
                budget,
                cancelled,
            )?
        }
        #[cfg(feature = "ace16-create")]
        (NativeUpdateProfile::Ace16HistoricalPresentFixed, NativeUpdateBody::Fixed(original)) => {
            let groups = original.borrowed_groups(budget, cancelled)?;
            image.plan_historical_present_fixed_updates(
                original.table(),
                &groups,
                budget,
                cancelled,
            )?
        }
        (NativeUpdateProfile::Jet4Int32, NativeUpdateBody::Int32(original)) => image
            .plan_int32_updates(
                original.table(),
                original.column(),
                original.updates(),
                budget,
                cancelled,
            )?,
        (NativeUpdateProfile::Ace16BeforeChangeInt32, NativeUpdateBody::Int32(original)) => image
            .plan_before_change_int32_updates(
            original.table(),
            original.column(),
            original.updates(),
            budget,
            cancelled,
        )?,
        (NativeUpdateProfile::Ace16BeforeChangeFixed, NativeUpdateBody::Fixed(original)) => {
            let groups = original.borrowed_groups(budget, cancelled)?;
            image.plan_before_change_fixed_updates(original.table(), &groups, budget, cancelled)?
        }
        (NativeUpdateProfile::Ace16OrdinaryFixed, NativeUpdateBody::Fixed(original)) => {
            let groups = original.borrowed_groups(budget, cancelled)?;
            image.plan_ordinary_fixed_updates(original.table(), &groups, budget, cancelled)?
        }
        (NativeUpdateProfile::Ace16OrdinaryIeeeFixed, NativeUpdateBody::Fixed(original)) => {
            let groups = original.borrowed_groups(budget, cancelled)?;
            image.plan_ordinary_ieee_updates(original.table(), &groups, budget, cancelled)?
        }
        (NativeUpdateProfile::Ace16OrdinaryScalarFixed, NativeUpdateBody::Fixed(original)) => {
            let groups = original.borrowed_groups(budget, cancelled)?;
            image.plan_ordinary_scalar_updates(original.table(), &groups, budget, cancelled)?
        }
        (NativeUpdateProfile::Ace16OrdinaryGuidFixed, NativeUpdateBody::Fixed(original)) => {
            let groups = original.borrowed_groups(budget, cancelled)?;
            image.plan_ordinary_guid_updates(original.table(), &groups, budget, cancelled)?
        }
        (NativeUpdateProfile::Ace16OrdinaryFitted, NativeUpdateBody::Fitted(original)) => {
            let groups = original.borrowed_groups(budget, cancelled)?;
            image.plan_ordinary_fitted_updates(original.table(), &groups, budget, cancelled)?
        }
        (
            NativeUpdateProfile::Ace16OrdinaryOwnedGrowth,
            NativeUpdateBody::OwnedGrowth(original),
        ) => {
            let groups = original.borrowed_groups(budget, cancelled)?;
            let plan = image.plan_ordinary_owned_growth_updates(
                original.table(),
                &groups,
                budget,
                cancelled,
            )?;
            if plan.before_bytes().len() != plan.after_bytes().len() {
                return Err(NativeIntentError::UnsupportedProfile.into());
            }
            plan
        }
        (
            NativeUpdateProfile::Ace16OrdinaryEofAllocation,
            NativeUpdateBody::EofAllocation(original),
        ) => {
            let groups = original.borrowed_groups(budget, cancelled)?;
            let plan =
                image.plan_ordinary_eof_updates(original.table(), &groups, budget, cancelled)?;
            if plan.before_bytes().len().checked_add(PAGE_BYTES) != Some(plan.after_bytes().len()) {
                return Err(NativeRecoveryQualificationError::ImageMismatch);
            }
            plan
        }
        _ => return Err(NativeIntentError::UnsupportedProfile.into()),
    };
    Ok(plan)
}
