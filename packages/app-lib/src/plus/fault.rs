//! Named failure points for rollback tests. They compile to nothing in normal
//! builds.

/// Aborts the process when `PLUS_CRASH_AT` names this point.
#[cfg(feature = "plus-fault-injection")]
pub(crate) fn hit(point: &str) {
    if std::env::var("PLUS_CRASH_AT").as_deref() == Ok(point) {
        std::process::abort();
    }
}

#[cfg(not(feature = "plus-fault-injection"))]
pub(crate) fn hit(_point: &str) {}

/// Returns an error when `PLUS_FAIL_AT` names this point.
#[cfg(feature = "plus-fault-injection")]
pub(crate) fn fail_point(point: &str) -> crate::Result<()> {
    if std::env::var("PLUS_FAIL_AT").as_deref() == Ok(point) {
        return Err(crate::ErrorKind::ContentStoreError(format!(
            "injected failure at {point}"
        ))
        .into());
    }
    Ok(())
}

#[cfg(not(feature = "plus-fault-injection"))]
pub(crate) fn fail_point(_point: &str) -> crate::Result<()> {
    Ok(())
}
