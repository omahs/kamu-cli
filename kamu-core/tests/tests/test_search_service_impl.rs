// Copyright Kamu Data, Inc. and contributors. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::utils::MinioServer;
use kamu::domain::*;
use kamu::infra::*;
use kamu::testing::*;
use opendatafabric::*;

use std::assert_matches::assert_matches;
use std::path::Path;
use std::sync::Arc;
use url::Url;

// Create repo/bar dataset in a repo and check it appears in searches
async fn do_test_search(tmp_workspace_dir: &Path, repo_url: Url) {
    let dataset_local_name = DatasetName::new_unchecked("foo");
    let repo_name = RepositoryName::new_unchecked("repo");
    let dataset_remote_name = RemoteDatasetName::try_from("repo/bar").unwrap();

    let workspace_layout = Arc::new(WorkspaceLayout::create(tmp_workspace_dir).unwrap());
    let local_repo = Arc::new(LocalDatasetRepositoryImpl::new(workspace_layout.clone()));
    let remote_repo_reg = Arc::new(RemoteRepositoryRegistryImpl::new(workspace_layout.clone()));
    let sync_svc = SyncServiceImpl::new(
        remote_repo_reg.clone(),
        local_repo.clone(),
        Arc::new(DatasetFactoryImpl::new()),
        Arc::new(kamu::infra::utils::ipfs_wrapper::IpfsClient::default()),
        IpfsGateway::default(),
    );

    let search_svc = SearchServiceImpl::new(remote_repo_reg.clone());

    // Add repository
    remote_repo_reg
        .add_repository(&repo_name, repo_url)
        .unwrap();

    // Add and sync dataset
    local_repo
        .create_dataset_from_snapshot(
            MetadataFactory::dataset_snapshot()
                .name(&dataset_local_name)
                .kind(DatasetKind::Root)
                .push_event(MetadataFactory::set_polling_source().build())
                .build(),
        )
        .await
        .unwrap();

    sync_svc
        .sync(
            &dataset_local_name.as_any_ref(),
            &dataset_remote_name.as_any_ref(),
            SyncOptions::default(),
            None,
        )
        .await
        .unwrap();

    // Search!
    assert_matches!(
        search_svc.search(None, SearchOptions::default()).await,
        Ok(SearchResult { datasets }) if datasets == vec![dataset_remote_name.clone()]
    );
    assert_matches!(
        search_svc.search(Some("bar"), SearchOptions::default()).await,
        Ok(SearchResult { datasets }) if datasets == vec![dataset_remote_name.clone()]
    );
    assert_matches!(
        search_svc.search(Some("foo"), SearchOptions::default()).await,
        Ok(SearchResult { datasets }) if datasets.is_empty()
    );
}

#[tokio::test]
async fn test_search_local_fs() {
    let tmp_workspace_dir = tempfile::tempdir().unwrap();
    let tmp_repo_dir = tempfile::tempdir().unwrap();
    let repo_url = Url::from_directory_path(tmp_repo_dir.path()).unwrap();

    do_test_search(tmp_workspace_dir.path(), repo_url).await;
}

#[tokio::test]
#[cfg_attr(feature = "skip_docker_tests", ignore)]
async fn test_search_s3() {
    let access_key = "AKIAIOSFODNN7EXAMPLE";
    let secret_key = "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY";
    std::env::set_var("AWS_ACCESS_KEY_ID", access_key);
    std::env::set_var("AWS_SECRET_ACCESS_KEY", secret_key);

    let tmp_workspace_dir = tempfile::tempdir().unwrap();
    let tmp_repo_dir = tempfile::tempdir().unwrap();
    let bucket = "test-bucket";
    std::fs::create_dir(tmp_repo_dir.path().join(bucket)).unwrap();

    let minio = MinioServer::new(tmp_repo_dir.path(), access_key, secret_key);

    use std::str::FromStr;
    let repo_url = Url::from_str(&format!(
        "s3+http://{}:{}/{}",
        minio.address, minio.host_port, bucket
    ))
    .unwrap();

    do_test_search(tmp_workspace_dir.path(), repo_url).await;
}
