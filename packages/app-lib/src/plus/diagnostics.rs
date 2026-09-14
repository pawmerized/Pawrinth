//! Local crash log analysis. Every finding says how sure it is, and nothing is
//! sent anywhere.

use super::manifest::ManifestEntry;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    /// The log states this cause directly.
    Confirmed,
    /// The log shows a pattern that usually has this cause.
    Possible,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    MissingDependency,
    VersionMismatch,
    WrongLoader,
    JavaVersion,
    OutOfMemory,
    ModConflict,
    MixinFailure,
    CorruptConfig,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub kind: FindingKind,
    pub confidence: Confidence,
    /// The log line this came from, so the user can check it themselves.
    pub evidence: String,
    /// Project or file name named by the log, if any.
    pub subject: Option<String>,
    /// Installed files whose names match the subject.
    pub related_files: Vec<String>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct CrashDiagnosis {
    pub findings: Vec<Finding>,
}

fn after<'a>(line: &'a str, marker: &str) -> Option<&'a str> {
    line.find(marker)
        .map(|index| line[index + marker.len()..].trim())
}

fn first_token(text: &str) -> Option<String> {
    let token: String = text
        .trim_matches(|c: char| matches!(c, '\'' | '"' | '`' | '[' | '{' | '('))
        .chars()
        .take_while(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | '.'))
        .collect();
    (!token.is_empty()).then_some(token)
}

fn subject_after(line: &str, markers: &[&str]) -> Option<String> {
    markers
        .iter()
        .find_map(|marker| after(line, marker))
        .and_then(first_token)
}

fn classify(line: &str) -> Option<(FindingKind, Confidence, Option<String>)> {
    let lower = line.to_lowercase();
    if lower.contains("requires") && lower.contains("which is missing") {
        return Some((
            FindingKind::MissingDependency,
            Confidence::Confirmed,
            subject_after(line, &["requires ", "Requires "]),
        ));
    }
    if lower.contains("missing or unsupported mandatory dependencies")
        || lower.contains("missing mods:")
    {
        return Some((
            FindingKind::MissingDependency,
            Confidence::Confirmed,
            subject_after(line, &["Mod ID: ", "modid: "]),
        ));
    }
    if lower.contains("mod resolution failed")
        || (lower.contains("requires")
            && lower.contains("but only the wrong version"))
    {
        return Some((
            FindingKind::VersionMismatch,
            Confidence::Confirmed,
            subject_after(line, &["requires ", "Requires "]),
        ));
    }
    if lower.contains("unsupportedclassversionerror")
        || lower.contains("class file version")
        || lower.contains("java version mismatch")
    {
        return Some((FindingKind::JavaVersion, Confidence::Confirmed, None));
    }
    if lower.contains("java.lang.outofmemoryerror")
        || lower.contains("there is insufficient memory")
    {
        return Some((FindingKind::OutOfMemory, Confidence::Confirmed, None));
    }
    if lower.contains("incompatible mod set")
        || lower.contains("found incompatible mods")
        || lower.contains("duplicate mods")
    {
        return Some((
            FindingKind::ModConflict,
            Confidence::Confirmed,
            subject_after(line, &["Mod ", "mod "]),
        ));
    }
    if lower.contains("was loaded on") && lower.contains("wrong loader") {
        return Some((FindingKind::WrongLoader, Confidence::Confirmed, None));
    }
    if lower.contains("mixin apply failed")
        || lower.contains("mixinapplyerror")
        || lower.contains("mixin transformation")
    {
        return Some((
            FindingKind::MixinFailure,
            Confidence::Possible,
            subject_after(line, &["from mod ", "mixins."]),
        ));
    }
    if lower.contains("failed loading config file")
        || lower.contains("configloadingexception")
    {
        return Some((FindingKind::CorruptConfig, Confidence::Possible, None));
    }
    None
}

/// Finds known failures in a log. Lines come from the player's own game, so each
/// finding keeps only the one line it's based on.
pub fn diagnose(log: &str, installed: &[ManifestEntry]) -> CrashDiagnosis {
    const MAX_EVIDENCE_CHARS: usize = 400;
    let mut findings: Vec<Finding> = Vec::new();
    for line in log.lines() {
        let Some((kind, confidence, subject)) = classify(line) else {
            continue;
        };
        if findings
            .iter()
            .any(|finding| finding.kind == kind && finding.subject == subject)
        {
            continue;
        }
        let related_files = subject
            .as_deref()
            .map(|subject| {
                let subject = subject.to_lowercase();
                installed
                    .iter()
                    .filter(|entry| {
                        entry.file_name.to_lowercase().contains(&subject)
                    })
                    .map(|entry| entry.file_name.clone())
                    .collect()
            })
            .unwrap_or_default();
        findings.push(Finding {
            kind,
            confidence,
            evidence: line.trim().chars().take(MAX_EVIDENCE_CHARS).collect(),
            subject,
            related_files,
        });
    }
    CrashDiagnosis { findings }
}

pub async fn diagnose_instance_log(
    instance_id: &str,
    log: &str,
) -> crate::Result<CrashDiagnosis> {
    let manifest = super::operations::instance_manifest(instance_id).await?;
    Ok(diagnose(log, &manifest.entries))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plus::manifest::tests::entry;

    #[test]
    fn missing_dependency_is_confirmed_and_names_the_mod() {
        let diagnosis = diagnose(
            "Mod 'Example' (example) 1.0 requires fabric-api, which is missing!",
            &[],
        );
        assert_eq!(diagnosis.findings.len(), 1);
        let finding = &diagnosis.findings[0];
        assert_eq!(finding.kind, FindingKind::MissingDependency);
        assert_eq!(finding.confidence, Confidence::Confirmed);
        assert_eq!(finding.subject.as_deref(), Some("fabric-api"));
    }

    #[test]
    fn java_version_errors_are_recognized() {
        let diagnosis = diagnose(
            "java.lang.UnsupportedClassVersionError: compiled by a more recent version",
            &[],
        );
        assert_eq!(diagnosis.findings[0].kind, FindingKind::JavaVersion);
    }

    #[test]
    fn mixin_failures_stay_possible_not_confirmed() {
        let diagnosis = diagnose("Mixin apply failed sodium.mixins.json", &[]);
        assert_eq!(diagnosis.findings[0].confidence, Confidence::Possible);
    }

    #[test]
    fn related_files_come_only_from_a_name_in_the_log() {
        let installed = [
            entry("1", "mods/fabric-api-0.100.jar", "a", Some("A")),
            entry("2", "mods/sodium.jar", "b", Some("B")),
        ];
        let diagnosis =
            diagnose("X requires fabric-api, which is missing!", &installed);
        assert_eq!(
            diagnosis.findings[0].related_files,
            ["fabric-api-0.100.jar"]
        );
    }

    #[test]
    fn an_ordinary_stack_trace_blames_nothing() {
        let log = "java.lang.NullPointerException: oops\n\tat com.example.Thing.run(Thing.java:1)";
        assert!(diagnose(log, &[]).findings.is_empty());
    }

    #[test]
    fn repeated_lines_are_reported_once() {
        let line = "X requires fabric-api, which is missing!\n";
        assert_eq!(diagnose(&line.repeat(5), &[]).findings.len(), 1);
    }
}
