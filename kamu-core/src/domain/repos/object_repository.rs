// Copyright Kamu Data, Inc. and contributors. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::domain::InternalError;
use opendatafabric::Multihash;

use async_trait::async_trait;
use bytes::Bytes;
use thiserror::Error;
use tokio::io::AsyncRead;

/////////////////////////////////////////////////////////////////////////////////////////

type AsyncReadObj = dyn AsyncRead + Send + Unpin;

/////////////////////////////////////////////////////////////////////////////////////////

/// Represents a content-addressable storage
#[async_trait]
pub trait ObjectRepository {
    async fn contains(&self, hash: &Multihash) -> Result<bool, InternalError>;

    async fn get_bytes(&self, hash: &Multihash) -> Result<Bytes, GetError>;

    async fn get_stream(&self, hash: &Multihash) -> Result<Box<AsyncReadObj>, GetError>;

    async fn insert_bytes<'a>(
        &'a self,
        data: &'a [u8],
        options: InsertOpts<'a>,
    ) -> Result<InsertResult, InsertError>;

    async fn insert_stream<'a>(
        &'a self,
        src: Box<AsyncReadObj>,
        options: InsertOpts<'a>,
    ) -> Result<InsertResult, InsertError>;

    // /// Attempts to inserts file via atomic move on local FS.
    // /// Otherwise will copy the contents and delete the source file.
    // async fn insert_file(&self, file: &Path, options: InsertOpts)
    //     -> Result<Multihash, InsertError>;

    async fn delete(&self, hash: &Multihash) -> Result<(), InternalError>;
}

/////////////////////////////////////////////////////////////////////////////////////////

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct InsertResult {
    pub hash: Multihash,
    pub already_existed: bool,
}

/////////////////////////////////////////////////////////////////////////////////////////

#[derive(Clone, Default, Debug)]
pub struct InsertOpts<'a> {
    /// Insert object using provided hash computed elsewhere.
    ///
    /// Warning: Use only when you fully trust the source of the precomputed hash.
    pub precomputed_hash: Option<&'a Multihash>,

    /// Insert will result in error if computed hash does not match this one.
    pub expected_hash: Option<&'a Multihash>,

    /// Hints the size of an object
    pub size_hint: Option<usize>,
}

/////////////////////////////////////////////////////////////////////////////////////////
// Errors
/////////////////////////////////////////////////////////////////////////////////////////

#[derive(Error, PartialEq, Eq, Debug)]
#[error("object does not exist: {hash}")]
pub struct ObjectNotFoundError {
    pub hash: Multihash,
}

/////////////////////////////////////////////////////////////////////////////////////////

#[derive(Error, Debug)]
pub enum GetError {
    #[error(transparent)]
    NotFound(#[from] ObjectNotFoundError),
    #[error(transparent)]
    Internal(#[from] InternalError),
}

/////////////////////////////////////////////////////////////////////////////////////////

#[derive(Error, Debug)]
#[error("expected hash {expected} but got {actual}")]
pub struct HashMismatchError {
    pub expected: Multihash,
    pub actual: Multihash,
}

/////////////////////////////////////////////////////////////////////////////////////////

#[derive(Error, Debug)]
pub enum InsertError {
    #[error(transparent)]
    HashMismatch(#[from] HashMismatchError),
    #[error(transparent)]
    Internal(#[from] InternalError),
}