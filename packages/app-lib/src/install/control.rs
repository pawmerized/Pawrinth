use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex};
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard, watch};
use uuid::Uuid;

static CONTROLS: LazyLock<Mutex<HashMap<Uuid, Arc<InstallControl>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

static SHARED_STEPS: LazyLock<DashMap<String, Arc<AsyncMutex<()>>>> =
    LazyLock::new(DashMap::new);

tokio::task_local! {
    pub(crate) static CURRENT_INSTALL: Arc<InstallControl>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
    Running,
    Paused,
    Canceling,
    Finishing,
}

#[derive(Debug)]
pub(crate) struct InstallControl {
    mode: watch::Sender<Mode>,
}

impl InstallControl {
    pub(super) fn set_paused(&self, paused: bool) -> crate::Result<()> {
        let mut accepted = false;
        self.mode.send_modify(|mode| {
            if matches!(mode, Mode::Running | Mode::Paused) {
                *mode = if paused { Mode::Paused } else { Mode::Running };
                accepted = true;
            }
        });
        if accepted { Ok(()) } else { Err(unavailable()) }
    }

    pub(super) fn cancel(&self) -> crate::Result<()> {
        let mut accepted = false;
        self.mode.send_modify(|mode| {
            if *mode != Mode::Finishing {
                *mode = Mode::Canceling;
                accepted = true;
            }
        });
        if accepted { Ok(()) } else { Err(unavailable()) }
    }

    pub(super) fn is_canceling(&self) -> bool {
        *self.mode.borrow() == Mode::Canceling
    }

    pub(crate) async fn checkpoint(&self) -> crate::Result<()> {
        let mut mode = self.mode.subscribe();
        loop {
            let current = *mode.borrow_and_update();
            match current {
                Mode::Running => return Ok(()),
                Mode::Canceling => return Err(canceled()),
                Mode::Finishing => return Err(unavailable()),
                Mode::Paused => {}
            }
            mode.changed().await.map_err(|_| unavailable())?;
        }
    }

    pub(super) async fn canceled(&self) {
        let mut mode = self.mode.subscribe();
        loop {
            if *mode.borrow_and_update() == Mode::Canceling {
                return;
            }
            if mode.changed().await.is_err() {
                return;
            }
        }
    }

    pub(super) async fn finish_work(&self) -> crate::Result<()> {
        loop {
            self.checkpoint().await?;
            if self.mode.send_if_modified(|mode| {
                if *mode != Mode::Running {
                    return false;
                }
                *mode = Mode::Finishing;
                true
            }) {
                return Ok(());
            }
        }
    }

    pub(super) fn finish_failed(&self) {
        self.mode.send_modify(|mode| {
            if *mode != Mode::Canceling {
                *mode = Mode::Finishing;
            }
        });
    }
}

pub(super) struct Registration {
    job_id: Uuid,
    pub control: Arc<InstallControl>,
}

impl Drop for Registration {
    fn drop(&mut self) {
        let mut controls =
            CONTROLS.lock().unwrap_or_else(|error| error.into_inner());
        if controls
            .get(&self.job_id)
            .is_some_and(|current| Arc::ptr_eq(current, &self.control))
        {
            controls.remove(&self.job_id);
        }
    }
}

pub(super) fn register(job_id: Uuid) -> Registration {
    let (mode, _) = watch::channel(Mode::Running);
    let control = Arc::new(InstallControl { mode });
    CONTROLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .insert(job_id, control.clone());
    Registration { job_id, control }
}

pub(super) fn get(job_id: Uuid) -> Option<Arc<InstallControl>> {
    CONTROLS
        .lock()
        .unwrap_or_else(|error| error.into_inner())
        .get(&job_id)
        .cloned()
}

pub(super) fn snapshot(job_id: Uuid) -> (bool, bool, bool) {
    let Some(control) = get(job_id) else {
        return (false, false, false);
    };
    let mode = *control.mode.borrow();
    (
        mode == Mode::Paused,
        mode == Mode::Canceling,
        matches!(mode, Mode::Running | Mode::Paused),
    )
}

pub(super) async fn checkpoint(job_id: Uuid) -> crate::Result<()> {
    if let Some(control) = get(job_id) {
        control.checkpoint().await?;
    }
    Ok(())
}

pub(super) fn ensure_not_canceled(job_id: Uuid) -> crate::Result<()> {
    if get(job_id).is_some_and(|control| control.is_canceling()) {
        return Err(canceled());
    }
    Ok(())
}

fn unavailable() -> crate::Error {
    crate::ErrorKind::InputError(
        "install job is no longer controllable".to_string(),
    )
    .into()
}

fn canceled() -> crate::Error {
    crate::ErrorKind::InputError("install was canceled".to_string()).into()
}

/// Takes a turn at a step that installs of different instances would otherwise run on the
/// same shared files. Waiting ends if the install is canceled. A paused install still takes
/// its turn, because the holder never stops for a pause, so it can't hold up the others.
pub(crate) async fn lock_shared_step(
    key: String,
) -> crate::Result<OwnedMutexGuard<()>> {
    let lock = SHARED_STEPS.entry(key).or_default().value().clone();
    let Ok(control) = CURRENT_INSTALL.try_with(Clone::clone) else {
        return Ok(lock.lock_owned().await);
    };
    tokio::select! {
        biased;
        () = control.canceled() => Err(canceled()),
        guard = lock.lock_owned() => Ok(guard),
    }
}

pub(crate) async fn download_step<F: std::future::Future>(
    future: F,
) -> crate::Result<F::Output> {
    let Ok(control) = CURRENT_INSTALL.try_with(Clone::clone) else {
        return Ok(future.await);
    };
    control.checkpoint().await?;
    tokio::select! {
        biased;
        () = control.canceled() => Err(canceled()),
        output = future => {
            control.checkpoint().await?;
            Ok(output)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[tokio::test]
    async fn a_paused_install_still_takes_its_turn() {
        let registration = register(Uuid::new_v4());
        registration.control.set_paused(true).unwrap();

        let result = tokio::time::timeout(
            Duration::from_secs(5),
            CURRENT_INSTALL.scope(
                registration.control.clone(),
                lock_shared_step("paused-test".to_string()),
            ),
        )
        .await
        .expect("a pause must not hold back the turn");

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn canceling_stops_waiting_for_a_turn() {
        let registration = register(Uuid::new_v4());
        let _held = lock_shared_step("canceled-test".to_string()).await;
        let waiting = tokio::spawn(CURRENT_INSTALL.scope(
            registration.control.clone(),
            lock_shared_step("canceled-test".to_string()),
        ));

        registration.control.cancel().unwrap();
        let result = tokio::time::timeout(Duration::from_secs(5), waiting)
            .await
            .expect("canceling must end the wait")
            .unwrap();

        assert!(result.is_err());
    }
}
