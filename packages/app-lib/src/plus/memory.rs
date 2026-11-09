//! Automatic memory for instances whose modpack recommends an amount, such as a
//! Technic pack on Solder or an instance imported from the FTB App. The recommendation
//! is trimmed to what this PC can spare, and it stays automatic only until someone sets
//! the instance's memory themselves.

use super::settings::{InstanceMemoryPlan, MemorySource, PlusSettings};
use crate::State;
use crate::state::{
    EditInstance, InstanceLaunchOverridesPatch, MemorySettings,
};
use serde::Serialize;

/// Minecraft barely starts with less.
const MIN_ALLOCATION_MB: u32 = 512;
/// Amounts are rounded down to this, like the memory slider's own steps.
const STEP_MB: u64 = 256;
/// Windows and other apps keep at least a quarter of the RAM, and up to this much on
/// smaller PCs.
const SMALL_PC_RESERVE_MB: u64 = 4096;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub struct MemoryChoice {
    pub allocated_mb: u32,
    /// Less than the recommendation, because the PC can't spare that much.
    pub reduced: bool,
}

/// The most an instance should get on a PC with this much RAM.
pub fn safe_maximum_mb(total_mb: u64) -> u32 {
    let reserve = (total_mb / 4).max((total_mb / 2).min(SMALL_PC_RESERVE_MB));
    let available = total_mb.saturating_sub(reserve) / STEP_MB * STEP_MB;
    u32::try_from(available)
        .unwrap_or(u32::MAX)
        .max(MIN_ALLOCATION_MB)
}

pub fn choose(recommended_mb: u32, total_mb: u64) -> MemoryChoice {
    let safe = safe_maximum_mb(total_mb);
    let wanted = recommended_mb.max(MIN_ALLOCATION_MB);
    if wanted <= safe {
        return MemoryChoice {
            allocated_mb: wanted,
            reduced: false,
        };
    }
    MemoryChoice {
        allocated_mb: safe,
        reduced: true,
    }
}

async fn total_memory_mb() -> crate::Result<u64> {
    Ok(crate::api::jre::get_max_memory().await? / 1024)
}

/// Where the instance's memory stands, for the Java and memory settings.
#[derive(Clone, Debug, Serialize)]
pub struct MemoryStatus {
    pub recommended_mb: Option<u32>,
    pub source: Option<MemorySource>,
    pub automatic: bool,
    /// The instance's own setting. `None` means it uses the default from Settings.
    pub current_mb: Option<u32>,
    pub total_mb: u64,
    /// What the recommendation comes to on this PC.
    pub suggested_mb: Option<u32>,
    pub reduced: bool,
}

async fn plan_of(
    instance_id: &str,
    state: &State,
) -> Option<InstanceMemoryPlan> {
    PlusSettings::read(&state.pool)
        .await
        .ok()?
        .automatic_memory
        .instances
        .get(instance_id)
        .cloned()
}

async fn save_plan(
    instance_id: &str,
    plan: &InstanceMemoryPlan,
    state: &State,
) -> crate::Result<()> {
    PlusSettings::merge_patch(
        &serde_json::json!({ "automatic_memory": { "instances": {
            instance_id: plan
        } } }),
        &state.pool,
    )
    .await?;
    Ok(())
}

async fn current_memory(instance_id: &str) -> crate::Result<Option<u32>> {
    Ok(crate::api::instance::get(instance_id)
        .await?
        .and_then(|metadata| metadata.launch_overrides.memory)
        .map(|memory| memory.maximum))
}

async fn set_memory(instance_id: &str, maximum: u32) -> crate::Result<()> {
    crate::api::instance::edit(
        instance_id,
        EditInstance {
            launch_overrides: Some(InstanceLaunchOverridesPatch {
                memory: Some(Some(MemorySettings { maximum })),
                ..InstanceLaunchOverridesPatch::default()
            }),
            ..EditInstance::default()
        },
    )
    .await?;
    Ok(())
}

/// Brings an automatic plan up to date. Memory someone changed since Pawrinth set it
/// stays as it is, and the plan stops being automatic.
async fn reconcile(
    instance_id: &str,
    mut plan: InstanceMemoryPlan,
    total_mb: u64,
    state: &State,
) -> crate::Result<InstanceMemoryPlan> {
    let (true, Some(recommended)) = (plan.automatic, plan.recommended_mb)
    else {
        return Ok(plan);
    };
    let current = current_memory(instance_id).await?;
    if plan.applied_mb.is_some() && current != plan.applied_mb {
        plan.automatic = false;
        save_plan(instance_id, &plan, state).await?;
        return Ok(plan);
    }
    let choice = choose(recommended, total_mb);
    if current != Some(choice.allocated_mb) {
        set_memory(instance_id, choice.allocated_mb).await?;
        plan.applied_mb = Some(choice.allocated_mb);
        save_plan(instance_id, &plan, state).await?;
    }
    Ok(plan)
}

/// Records a modpack's recommendation for a new instance and applies it, unless the
/// instance already has memory someone chose.
pub async fn recommend(
    instance_id: &str,
    recommended_mb: u32,
    source: MemorySource,
) -> crate::Result<()> {
    let state = State::get().await?;
    let previous = plan_of(instance_id, &state).await;
    let chosen_by_someone = previous
        .as_ref()
        .is_some_and(|plan| !plan.automatic)
        || (previous.is_none() && current_memory(instance_id).await?.is_some());
    let plan = InstanceMemoryPlan {
        recommended_mb: Some(recommended_mb),
        source: Some(source),
        automatic: !chosen_by_someone,
        applied_mb: previous.and_then(|plan| plan.applied_mb),
    };
    save_plan(instance_id, &plan, &state).await?;
    reconcile(instance_id, plan, total_memory_mb().await?, &state).await?;
    Ok(())
}

pub async fn status(instance_id: &str) -> crate::Result<MemoryStatus> {
    let state = State::get().await?;
    let total_mb = total_memory_mb().await?;
    let plan = match plan_of(instance_id, &state).await {
        Some(plan) => reconcile(instance_id, plan, total_mb, &state).await?,
        None => InstanceMemoryPlan::default(),
    };
    let choice = plan
        .recommended_mb
        .map(|recommended| choose(recommended, total_mb));
    Ok(MemoryStatus {
        recommended_mb: plan.recommended_mb,
        source: plan.source,
        automatic: plan.automatic,
        current_mb: current_memory(instance_id).await?,
        total_mb,
        suggested_mb: choice.map(|choice| choice.allocated_mb),
        reduced: choice.is_some_and(|choice| choice.reduced),
    })
}

/// Someone set the instance's memory themselves, so it isn't changed automatically any
/// more.
pub async fn set_manual(instance_id: &str) -> crate::Result<()> {
    let state = State::get().await?;
    if let Some(mut plan) = plan_of(instance_id, &state).await
        && plan.automatic
    {
        plan.automatic = false;
        save_plan(instance_id, &plan, &state).await?;
    }
    Ok(())
}

/// Goes back to the amount worked out from the recommendation.
pub async fn use_automatic(instance_id: &str) -> crate::Result<MemoryStatus> {
    let state = State::get().await?;
    if let Some(mut plan) = plan_of(instance_id, &state).await
        && plan.recommended_mb.is_some()
    {
        plan.automatic = true;
        plan.applied_mb = None;
        save_plan(instance_id, &plan, &state).await?;
        let total_mb = total_memory_mb().await?;
        let choice = choose(plan.recommended_mb.unwrap_or_default(), total_mb);
        set_memory(instance_id, choice.allocated_mb).await?;
        plan.applied_mb = Some(choice.allocated_mb);
        save_plan(instance_id, &plan, &state).await?;
    }
    status(instance_id).await
}

/// Checks every automatic plan again, for example after the PC's RAM changed, and
/// forgets plans for instances that no longer exist.
pub(crate) async fn refresh_all(state: &State) -> crate::Result<()> {
    let settings = PlusSettings::read(&state.pool).await?;
    if settings.automatic_memory.instances.is_empty() {
        return Ok(());
    }
    let total_mb = total_memory_mb().await?;
    for (instance_id, plan) in settings.automatic_memory.instances {
        if crate::api::instance::get(&instance_id).await?.is_none() {
            PlusSettings::merge_patch(
                &serde_json::json!({ "automatic_memory": { "instances": {
                    instance_id: null
                } } }),
                &state.pool,
            )
            .await?;
            continue;
        }
        if let Err(error) = reconcile(&instance_id, plan, total_mb, state).await
        {
            tracing::warn!("Couldn't update automatic memory: {error}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const GIB: u64 = 1024;

    #[test]
    fn windows_keeps_a_fair_share() {
        assert_eq!(safe_maximum_mb(4 * GIB), 2048);
        assert_eq!(safe_maximum_mb(6 * GIB), 3072);
        assert_eq!(safe_maximum_mb(8 * GIB), 4096);
        assert_eq!(safe_maximum_mb(16 * GIB), 12288);
        assert_eq!(safe_maximum_mb(32 * GIB), 24576);
    }

    #[test]
    fn tiny_pcs_still_get_enough_to_start() {
        assert_eq!(safe_maximum_mb(1024), 512);
        assert_eq!(safe_maximum_mb(0), MIN_ALLOCATION_MB);
    }

    #[test]
    fn a_recommendation_that_fits_is_used_as_it_is() {
        assert_eq!(
            choose(6144, 16 * GIB),
            MemoryChoice {
                allocated_mb: 6144,
                reduced: false,
            }
        );
    }

    #[test]
    fn a_recommendation_that_doesnt_fit_is_reduced() {
        assert_eq!(
            choose(8192, 8 * GIB),
            MemoryChoice {
                allocated_mb: 4096,
                reduced: true,
            }
        );
        assert_eq!(
            choose(5120, 4 * GIB),
            MemoryChoice {
                allocated_mb: 2048,
                reduced: true,
            }
        );
    }

    #[test]
    fn odd_recommendations_still_give_a_usable_amount() {
        assert_eq!(choose(0, 16 * GIB).allocated_mb, MIN_ALLOCATION_MB);
        assert_eq!(choose(100, 16 * GIB).allocated_mb, MIN_ALLOCATION_MB);
    }
}
