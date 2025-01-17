// Copyright Kamu Data, Inc. and contributors. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::test_object_repository_shared;
use kamu::infra::*;

/////////////////////////////////////////////////////////////////////////////////////////

#[tokio::test]
async fn test_insert_bytes() {
    let repo = ObjectRepositoryInMemory::new();
    test_object_repository_shared::test_insert_bytes(&repo).await;
}

#[tokio::test]
async fn test_delete() {
    let repo = ObjectRepositoryInMemory::new();
    test_object_repository_shared::test_delete(&repo, true).await;
}

#[tokio::test]
async fn test_insert_precomputed() {
    let repo = ObjectRepositoryInMemory::new();
    test_object_repository_shared::test_insert_precomputed(&repo).await;
}

#[tokio::test]
async fn test_insert_expect() {
    let repo = ObjectRepositoryInMemory::new();
    test_object_repository_shared::test_insert_expect(&repo).await;
}

/////////////////////////////////////////////////////////////////////////////////////////
