use super::*;
use crate::tools::Tool;

#[test]
fn test_openz_embedded_paths_use_openz_dir_for_defaults() {
    std::env::remove_var("SEARCHXYZ_INDEX_PATH");
    std::env::remove_var("SEARCHXYZ_CACHE_PATH");
    let mut config = Config::default();
    apply_openz_embedded_paths(&mut config);
    assert!(config.index.path.ends_with(".openz/searchxyz/index"));
    assert!(config.cache.path.ends_with(".openz/searchxyz/cache.json"));
}

#[test]
fn test_searchxyz_tools_metadata() {
    assert_eq!(SearchXyzDoctorTool.name(), "searchxyz_doctor");
    assert_eq!(SearchXyzSearchWebTool.name(), "searchxyz_search_web");
    assert_eq!(
        SearchXyzBrowserSearchTool.name(),
        "searchxyz_browser_search"
    );
    assert_eq!(SearchXyzReadUrlTool.name(), "searchxyz_read_url");
    assert_eq!(
        SearchXyzSearchAndReadTool.name(),
        "searchxyz_search_and_read"
    );
    assert_eq!(SearchXyzRecallTool.name(), "searchxyz_recall");
    assert_eq!(SearchXyzListSourcesTool.name(), "searchxyz_list_sources");
    assert_eq!(SearchXyzDeepResearchTool.name(), "searchxyz_deep_research");
    assert_eq!(SearchXyzIndexContentTool.name(), "searchxyz_index_content");
    assert_eq!(SearchXyzSiteMapTool.name(), "searchxyz_site_map");
    assert_eq!(
        SearchXyzIndexRelationshipTool.name(),
        "searchxyz_index_relationship"
    );
    assert_eq!(SearchXyzQueryGraphTool.name(), "searchxyz_query_graph");
    assert_eq!(
        SearchXyzReadGithubRepoTool.name(),
        "searchxyz_read_github_repo"
    );
    assert_eq!(
        SearchXyzExportResearchTool.name(),
        "searchxyz_export_research"
    );
    assert_eq!(
        SearchXyzImportResearchTool.name(),
        "searchxyz_import_research"
    );
    assert_eq!(SearchXyzDeleteSourceTool.name(), "searchxyz_delete_source");
    assert_eq!(SearchXyzClearIndexTool.name(), "searchxyz_clear_index");
}
