// Copyright Kamu Data, Inc. and contributors. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::domain::{DatasetNotFoundError, GetDatasetError, InternalError};
use opendatafabric::*;

use thiserror::Error;

/////////////////////////////////////////////////////////////////////////////////////////

#[async_trait::async_trait(?Send)]
pub trait ProvenanceService: Sync + Send {
    /// Passes the visitor through the dependency graph of a dataset
    /// Some predefined visitors are available.
    async fn get_dataset_lineage(
        &self,
        dataset_ref: &DatasetRefLocal,
        visitor: &mut dyn LineageVisitor,
        options: LineageOptions,
    ) -> Result<(), GetLineageError>;
}

/////////////////////////////////////////////////////////////////////////////////////////

pub trait LineageVisitor {
    fn begin(&mut self);
    fn enter(&mut self, dataset: &NodeInfo<'_>) -> bool;
    fn exit(&mut self, dataset: &NodeInfo<'_>);
    fn done(&mut self);
}

#[derive(Debug, Clone)]
pub enum NodeInfo<'a> {
    Local {
        id: DatasetID,
        name: DatasetName,
        kind: DatasetKind,
        dependencies: &'a [TransformInput],
    },
    Remote {
        id: DatasetID,
        name: DatasetName,
    },
}

impl<'a> NodeInfo<'a> {
    pub fn id(&self) -> &DatasetID {
        match self {
            NodeInfo::Local { id, .. } => id,
            NodeInfo::Remote { id, .. } => id,
        }
    }

    pub fn name(&self) -> &DatasetName {
        match self {
            NodeInfo::Local { name, .. } => name,
            NodeInfo::Remote { name, .. } => name,
        }
    }
}

/////////////////////////////////////////////////////////////////////////////////////////

pub struct LineageOptions {}

/////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Error)]
pub enum GetLineageError {
    #[error(transparent)]
    NotFound(#[from] DatasetNotFoundError),
    #[error(transparent)]
    Internal(
        #[from]
        #[backtrace]
        InternalError,
    ),
}

impl From<GetDatasetError> for GetLineageError {
    fn from(v: GetDatasetError) -> Self {
        match v {
            GetDatasetError::NotFound(e) => Self::NotFound(e),
            GetDatasetError::Internal(e) => Self::Internal(e),
        }
    }
}
