// Copyright Kamu Data, Inc. and contributors. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::*;
use crate::domain::engine::IngestRequest;
use crate::domain::*;
use crate::infra::*;
use opendatafabric::serde::yaml::*;
use opendatafabric::*;

use ::serde::{Deserialize, Serialize};
use ::serde_with::skip_serializing_none;
use chrono::{DateTime, Utc};
use std::path::Path;
use std::sync::Arc;

pub struct ReadService {
    engine_provisioner: Arc<dyn EngineProvisioner>,
}

impl ReadService {
    pub fn new(engine_provisioner: Arc<dyn EngineProvisioner>) -> Self {
        Self { engine_provisioner }
    }

    // TODO: Don't use engine for anything but preprocessing
    pub async fn read<'a, 'b>(
        &'a self,
        dataset_handle: &'b DatasetHandle,
        dataset_layout: &'b DatasetLayout,
        source: &'b SetPollingSource,
        prev_checkpoint: Option<Multihash>,
        vocab: &'b DatasetVocabulary,
        system_time: DateTime<Utc>,
        source_event_time: Option<DateTime<Utc>>,
        offset: i64,
        out_data_path: &'b Path,
        out_checkpoint_path: &'b Path,
        for_prepared_at: DateTime<Utc>,
        _old_checkpoint: Option<ReadCheckpoint>,
        src_path: &'b Path,
        listener: Arc<dyn IngestListener>,
    ) -> Result<ExecutionResult<ReadCheckpoint>, IngestError>
    where
        'a: 'b,
    {
        // Terminate early for zero-sized files
        if src_path.metadata().int_err()?.len() == 0 {
            return Ok(ExecutionResult {
                was_up_to_date: false,
                checkpoint: ReadCheckpoint {
                    last_read: Utc::now(),
                    for_prepared_at: for_prepared_at,
                    system_time,
                    engine_response: ExecuteQueryResponseSuccess {
                        data_interval: None,
                        output_watermark: None,
                    },
                },
            });
        }

        let engine = self
            .engine_provisioner
            .provision_ingest_engine(listener.get_engine_provisioning_listener())
            .await?;

        // Clean up previous state leftovers
        if out_data_path.exists() {
            std::fs::remove_file(&out_data_path).int_err()?;
        }
        if out_checkpoint_path.exists() {
            std::fs::remove_file(&out_checkpoint_path).int_err()?;
        }

        let request = IngestRequest {
            dataset_id: dataset_handle.id.clone(),
            dataset_name: dataset_handle.name.clone(),
            ingest_path: src_path.to_owned(),
            system_time,
            event_time: source_event_time,
            offset,
            source: source.clone(),
            dataset_vocab: vocab.clone(),
            prev_checkpoint_path: prev_checkpoint.map(|cp| dataset_layout.checkpoint_path(&cp)),
            data_dir: dataset_layout.data_dir.clone(),
            out_data_path: out_data_path.to_owned(),
            new_checkpoint_path: out_checkpoint_path.to_owned(),
        };

        let mut response = engine.ingest(request).await?;

        if let Some(data_interval) = &mut response.data_interval {
            if data_interval.end < data_interval.start || data_interval.start != offset {
                return Err(EngineError::contract_error(
                    "Engine returned an output slice with invalid data inverval",
                    Vec::new(),
                )
                .into());
            }
            if !out_data_path.exists() {
                return Err(EngineError::contract_error(
                    "Engine did not write a response data file",
                    Vec::new(),
                )
                .into());
            }
        }

        Ok(ExecutionResult {
            was_up_to_date: false,
            checkpoint: ReadCheckpoint {
                last_read: Utc::now(),
                for_prepared_at: for_prepared_at,
                system_time,
                engine_response: response,
            },
        })
    }
}

#[skip_serializing_none]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct ReadCheckpoint {
    #[serde(with = "datetime_rfc3339")]
    pub last_read: DateTime<Utc>,
    #[serde(with = "datetime_rfc3339")]
    pub for_prepared_at: DateTime<Utc>,
    #[serde(with = "datetime_rfc3339")]
    pub system_time: DateTime<Utc>,
    #[serde(with = "ExecuteQueryResponseSuccessDef")]
    pub engine_response: ExecuteQueryResponseSuccess,
}
