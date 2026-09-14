//! Made-up projects for development builds with no CurseForge API key, because every
//! project endpoint needs one. None of this is CurseForge data: names, authors and
//! links are invented, and the frontend labels it as sample data. Release builds don't
//! contain this module.

use super::curseforge::{
    Asset, Author, Category, FileDependency, LatestFileIndex, Links,
    Pagination, Project, ProjectFile, SearchQuery, SearchResults,
};
use crate::ErrorKind;

const SAMPLE_ID_BASE: u64 = 9_000_000_000;
const SAMPLE_FILE_ID_BASE: u64 = 9_500_000_000;
const SAMPLE_AUTHOR: &str = "Sample author";
const SAMPLE_AUTHOR_ID: u64 = 9_100_000_000;

/// `(name, classId)`. Class IDs match the public `/categories` response.
const SAMPLES: &[(&str, u32, &[u32])] = &[
    ("Sample Tech Mod", 6, &[1, 4, 6]),
    ("Sample Performance Mod", 6, &[4, 5]),
    ("Sample Library", 6, &[1, 4, 5, 6]),
    ("Sample Mod With Dependency", 6, &[4]),
    ("Sample Forge Only Mod", 6, &[1]),
    ("Sample Author-Restricted Mod", 6, &[1, 4]),
    ("Sample Adventure Pack", 4471, &[4]),
    ("Sample Kitchen Sink Pack", 4471, &[1]),
    ("Sample Resource Pack", 12, &[]),
    ("Sample Shader", 6552, &[]),
    ("Sample Data Pack", 6945, &[]),
    ("Sample World", 17, &[]),
    ("Sample Bukkit Plugin", 5, &[]),
    ("Sample Customization", 4546, &[]),
    ("Sample Addon", 4559, &[]),
];

const RESTRICTED: &str = "Sample Author-Restricted Mod";
const WITH_DEPENDENCY: &str = "Sample Mod With Dependency";
const LIBRARY: &str = "Sample Library";

pub(crate) fn is_sample_id(id: u64) -> bool {
    id >= SAMPLE_ID_BASE
}

fn loader_tag(loader: u32) -> &'static str {
    match loader {
        1 => "Forge",
        4 => "Fabric",
        5 => "Quilt",
        _ => "NeoForge",
    }
}

fn project_at(index: usize) -> Project {
    let (name, class_id, loaders) = SAMPLES[index];
    let id = SAMPLE_ID_BASE + index as u64;
    let slug = name.to_lowercase().replace(' ', "-");
    Project {
        id,
        name: name.to_string(),
        slug: slug.clone(),
        summary:
            "Sample data for development. This isn't a real CurseForge project."
                .to_string(),
        links: Links {
            website_url: Some(format!("https://sample.invalid/{slug}")),
            ..Links::default()
        },
        class_id: Some(class_id),
        download_count: 1_000.0 * (SAMPLES.len() - index) as f64,
        thumbs_up_count: 10 * (SAMPLES.len() - index) as u64,
        logo: Some(Asset::default()),
        screenshots: Vec::new(),
        authors: vec![Author {
            id: Some(SAMPLE_AUTHOR_ID),
            name: SAMPLE_AUTHOR.to_string(),
            url: Some("https://sample.invalid/author".to_string()),
            avatar_url: None,
        }],
        categories: vec![Category {
            id: 0,
            name: "Sample".to_string(),
            slug: "sample".to_string(),
            class_id: Some(class_id),
            is_class: Some(false),
        }],
        date_created: Some("2026-01-01T00:00:00Z".to_string()),
        date_modified: Some(format!(
            "2026-{:02}-15T00:00:00Z",
            10 - (index % 10)
        )),
        date_released: Some("2026-01-01T00:00:00Z".to_string()),
        allow_mod_distribution: Some(name != RESTRICTED),
        is_available: Some(true),
        latest_files_indexes: loaders
            .iter()
            .map(|loader| LatestFileIndex {
                game_version: "1.21.1".to_string(),
                mod_loader: Some(*loader),
            })
            .collect(),
        latest_files: Vec::new(),
    }
}

pub(crate) fn search(query: &SearchQuery) -> SearchResults {
    let filter = query
        .search_filter
        .as_deref()
        .map(str::trim)
        .filter(|filter| !filter.is_empty())
        .map(str::to_lowercase);
    let mut matches: Vec<Project> = (0..SAMPLES.len())
        .map(project_at)
        .filter(|project| {
            query.class_id.is_none_or(|id| project.class_id == Some(id))
        })
        .filter(|project| {
            filter.as_ref().is_none_or(|filter| {
                project.name.to_lowercase().contains(filter)
            })
        })
        .filter(|project| {
            query.game_version.as_deref().is_none_or(|version| {
                project
                    .latest_files_indexes
                    .iter()
                    .any(|index| index.game_version == version)
            })
        })
        .filter(|project| {
            query.author_id.is_none_or(|author| {
                project.authors.iter().any(|a| a.id == Some(author))
            })
        })
        // Samples have no real categories, so choosing one finds nothing.
        .filter(|_| query.category_id.is_none())
        .filter(|project| {
            query.mod_loader_type.is_none_or(|loader| {
                project
                    .latest_files_indexes
                    .iter()
                    .any(|index| index.mod_loader == Some(loader))
            })
        })
        .collect();
    if query.sort_field == Some(4) {
        matches.sort_by(|a, b| a.name.cmp(&b.name));
    }
    let total = matches.len() as u64;
    let page_size = query.page_size.clamp(1, 50) as usize;
    let data: Vec<Project> = matches
        .into_iter()
        .skip(query.index as usize)
        .take(page_size)
        .collect();
    SearchResults {
        pagination: Pagination {
            index: query.index,
            page_size: page_size as u32,
            result_count: data.len() as u32,
            total_count: total,
        },
        data,
    }
}

fn index_of(id: u64) -> Option<usize> {
    id.checked_sub(SAMPLE_ID_BASE)
        .map(|index| index as usize)
        .filter(|index| *index < SAMPLES.len())
}

pub(crate) fn project(id: u64) -> crate::Result<Project> {
    index_of(id).map(project_at).ok_or_else(|| {
        ErrorKind::InputError("unknown sample project".to_string()).into()
    })
}

pub(crate) fn files(project_id: u64) -> crate::Result<Vec<ProjectFile>> {
    let project = project(project_id)?;
    let index = index_of(project_id).unwrap_or_default();
    let (_, _, loaders) = SAMPLES[index];
    let dependencies = if project.name == WITH_DEPENDENCY {
        let library = SAMPLES
            .iter()
            .position(|sample| sample.0 == LIBRARY)
            .unwrap_or_default();
        vec![FileDependency {
            mod_id: SAMPLE_ID_BASE + library as u64,
            relation_type: 3,
        }]
    } else {
        Vec::new()
    };
    Ok([("1.21.1", "2.0.0", 1_u32), ("1.20.1", "1.0.0", 2_u32)]
        .iter()
        .enumerate()
        .map(|(position, (game_version, version, release_type))| {
            let mut game_versions = vec![(*game_version).to_string()];
            game_versions
                .extend(loaders.iter().map(|l| loader_tag(*l).to_string()));
            ProjectFile {
                id: SAMPLE_FILE_ID_BASE + (index as u64) * 10 + position as u64,
                mod_id: project_id,
                display_name: format!("{} {version}", project.name),
                file_name: format!("{}-{version}.jar", project.slug),
                release_type: *release_type,
                file_date: format!("2026-0{}-01T00:00:00Z", 9 - position),
                file_length: 1024,
                download_count: 0,
                download_url: None,
                game_versions,
                dependencies: dependencies.clone(),
                hashes: Vec::new(),
                is_available: Some(true),
            }
        })
        .collect())
}

pub(crate) fn file(
    project_id: u64,
    file_id: u64,
) -> crate::Result<ProjectFile> {
    files(project_id)?
        .into_iter()
        .find(|file| file.id == file_id)
        .ok_or_else(|| {
            ErrorKind::InputError("unknown sample file".to_string()).into()
        })
}

pub(crate) const DESCRIPTION: &str = "<p>Sample data for development. This isn't a real CurseForge project, and nothing here can be downloaded.</p>";
pub(crate) const CHANGELOG: &str =
    "<p>Sample changelog. This isn't real release information.</p>";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_sample_says_it_is_a_sample() {
        for index in 0..SAMPLES.len() {
            let project = project_at(index);
            assert!(project.name.starts_with("Sample "));
            assert!(
                project.summary.contains("isn't a real CurseForge project")
            );
            assert!(
                project.links.website_url.unwrap().contains(".invalid"),
                "sample links must never resolve"
            );
            assert!(is_sample_id(project.id));
        }
    }

    #[test]
    fn sample_files_can_never_be_downloaded() {
        for index in 0..SAMPLES.len() {
            for file in files(SAMPLE_ID_BASE + index as u64).unwrap() {
                assert!(file.download_url.is_none());
            }
        }
    }

    #[test]
    fn search_filters_and_paginates_for_real() {
        let query = SearchQuery {
            class_id: Some(6),
            page_size: 2,
            index: 2,
            ..SearchQuery::default()
        };
        let results = search(&query);
        assert_eq!(results.pagination.total_count, 6);
        assert_eq!(results.data.len(), 2);
        assert_eq!(results.data[0].name, SAMPLES[2].0);

        let filtered = search(&SearchQuery {
            class_id: Some(6),
            search_filter: Some("forge only".to_string()),
            page_size: 20,
            ..SearchQuery::default()
        });
        assert_eq!(filtered.pagination.total_count, 1);

        let by_loader = search(&SearchQuery {
            class_id: Some(6),
            mod_loader_type: Some(5),
            page_size: 20,
            ..SearchQuery::default()
        });
        assert_eq!(by_loader.pagination.total_count, 2);

        let by_version = search(&SearchQuery {
            class_id: Some(6),
            game_version: Some("1.8".to_string()),
            page_size: 20,
            ..SearchQuery::default()
        });
        assert_eq!(by_version.pagination.total_count, 0);
    }
}
