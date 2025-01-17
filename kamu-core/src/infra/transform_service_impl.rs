// Copyright Kamu Data, Inc. and contributors. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::domain::*;
use crate::infra::*;
use chrono::DateTime;
use chrono::Utc;
use opendatafabric::*;

use dill::*;
use futures::{StreamExt, TryFutureExt, TryStreamExt};
use opendatafabric::serde::flatbuffers::FlatbuffersMetadataBlockSerializer;
use opendatafabric::serde::MetadataBlockSerializer;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use tracing::debug;
use tracing::error;
use tracing::info;
use tracing::info_span;

pub struct TransformServiceImpl {
    local_repo: Arc<dyn LocalDatasetRepository>,
    engine_provisioner: Arc<dyn EngineProvisioner>,
    workspace_layout: Arc<WorkspaceLayout>,
}

#[component(pub)]
impl TransformServiceImpl {
    pub fn new(
        local_repo: Arc<dyn LocalDatasetRepository>,
        engine_provisioner: Arc<dyn EngineProvisioner>,
        workspace_layout: Arc<WorkspaceLayout>,
    ) -> Self {
        Self {
            local_repo,
            engine_provisioner,
            workspace_layout,
        }
    }

    // Note: Can be called from multiple threads
    async fn do_transform<CommitFn, Fut>(
        engine_provisioner: Arc<dyn EngineProvisioner>,
        operation: TransformOperation,
        commit_fn: CommitFn,
        listener: Arc<dyn TransformListener>,
    ) -> Result<TransformResult, TransformError>
    where
        CommitFn: FnOnce(MetadataBlock, PathBuf, PathBuf) -> Fut,
        Fut: futures::Future<Output = Result<TransformResult, TransformError>>,
    {
        let span = info_span!("Performing transform");
        let _span_guard = span.enter();
        info!(?operation, "Transform request");

        listener.begin();

        match Self::do_transform_inner(engine_provisioner, operation, commit_fn, listener.clone())
            .await
        {
            Ok(res) => {
                info!("Transform successful");
                listener.success(&res);
                Ok(res)
            }
            Err(err) => {
                error!(error = ?err, "Transform failed");
                listener.error(&err);
                Err(err)
            }
        }
    }

    // Note: Can be called from multiple threads
    async fn do_transform_inner<CommitFn, Fut>(
        engine_provisioner: Arc<dyn EngineProvisioner>,
        operation: TransformOperation,
        commit_fn: CommitFn,
        listener: Arc<dyn TransformListener>,
    ) -> Result<TransformResult, TransformError>
    where
        CommitFn: FnOnce(MetadataBlock, PathBuf, PathBuf) -> Fut,
        Fut: futures::Future<Output = Result<TransformResult, TransformError>>,
    {
        let new_checkpoint_path = PathBuf::from(&operation.request.new_checkpoint_path);
        let system_time = operation.request.system_time.clone();
        let out_data_path = PathBuf::from(&operation.request.out_data_path);
        let offset = operation.request.offset;

        let engine = engine_provisioner
            .provision_engine(
                match operation.request.transform {
                    Transform::Sql(ref sql) => &sql.engine,
                },
                listener.clone().get_engine_provisioning_listener(),
            )
            .await?;

        let response = engine.transform(operation.request).await?;

        let output_data = if let Some(interval) = response.data_interval {
            // TODO: Move out this to validation
            if interval.end < interval.start || interval.start != offset {
                return Err(EngineError::contract_error(
                    "Engine returned an output slice with invalid offset range",
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
            if out_data_path.is_symlink() || !out_data_path.is_file() {
                return Err(EngineError::contract_error(
                    "Engine wrote data not as a plain file",
                    Vec::new(),
                )
                .into());
            }

            let span = info_span!("Computing data hashes");
            let _span_guard = span.enter();

            // TODO: Move out into data commit procedure of sorts
            let logical_hash =
                crate::infra::utils::data_utils::get_parquet_logical_hash(&out_data_path)
                    .int_err()?;

            let physical_hash =
                crate::infra::utils::data_utils::get_file_physical_hash(&out_data_path)
                    .int_err()?;

            let size = std::fs::metadata(&out_data_path).int_err()?.len() as i64;

            Some(DataSlice {
                logical_hash,
                physical_hash,
                interval,
                size,
            })
        } else if out_data_path.exists() {
            return Err(EngineError::contract_error(
                "Engine wrote data file while the ouput slice is empty",
                Vec::new(),
            )
            .into());
        } else {
            None
        };

        let output_checkpoint = if new_checkpoint_path.exists() {
            if new_checkpoint_path.is_symlink() || !new_checkpoint_path.is_file() {
                return Err(EngineError::contract_error(
                    "Engine wrote checkpoint not as a plain file",
                    Vec::new(),
                )
                .into());
            }

            let physical_hash =
                crate::infra::utils::data_utils::get_file_physical_hash(&new_checkpoint_path)
                    .int_err()?;

            let size = std::fs::metadata(&new_checkpoint_path).int_err()?.len() as i64;

            Some(Checkpoint {
                physical_hash,
                size,
            })
        } else {
            None
        };

        let metadata_block = MetadataBlock {
            system_time,
            prev_block_hash: None, // Filled out at commit
            event: MetadataEvent::ExecuteQuery(ExecuteQuery {
                input_slices: operation.input_slices,
                input_checkpoint: operation.input_checkpoint,
                output_data,
                output_checkpoint,
                output_watermark: response.output_watermark,
            }),
            sequence_number: 0, // Filled out at commit
        };

        let result = commit_fn(
            metadata_block,
            out_data_path.clone(),
            new_checkpoint_path.clone(),
        )
        .await?;

        // Commit should clean up
        assert!(!out_data_path.exists());
        assert!(!new_checkpoint_path.exists());

        Ok(result)
    }

    async fn commit_transform(
        dataset_handle: DatasetHandle,
        dataset: Arc<dyn Dataset>,
        prev_block_hash: Multihash,
        prev_sequence_number: i32,
        new_block: MetadataBlock,
        new_data_path: PathBuf,
        new_checkpoint_path: PathBuf,
    ) -> Result<TransformResult, TransformError> {
        let new_block = MetadataBlock {
            prev_block_hash: Some(prev_block_hash.clone()),
            sequence_number: prev_sequence_number + 1,
            ..new_block
        };
        let new_block_t = new_block.as_typed::<ExecuteQuery>().unwrap();

        // Commit data
        if let Some(data_slice) = &new_block_t.event.output_data {
            dataset
                .as_data_repo()
                .insert_file_move(
                    &new_data_path,
                    InsertOpts {
                        precomputed_hash: Some(&data_slice.physical_hash),
                        expected_hash: None,
                        size_hint: Some(data_slice.size as usize),
                    },
                )
                .await
                .int_err()?;
        }

        // Commit checkpoint
        if let Some(checkpoint) = &new_block_t.event.output_checkpoint {
            dataset
                .as_checkpoint_repo()
                .insert_file_move(
                    &new_checkpoint_path,
                    InsertOpts {
                        precomputed_hash: Some(&checkpoint.physical_hash),
                        expected_hash: None,
                        size_hint: Some(checkpoint.size as usize),
                    },
                )
                .await
                .int_err()?;
        }

        let new_block_hash = dataset
            .as_metadata_chain()
            .append(new_block, AppendOpts::default())
            .await
            .int_err()?;

        info!(output_dataset = %dataset_handle, new_head = %new_block_hash, "Committed new block");

        Ok(TransformResult::Updated {
            old_head: prev_block_hash,
            new_head: new_block_hash,
            num_blocks: 1,
        })
    }

    // TODO: PERF: Avoid multiple passes over metadata chain
    pub async fn get_next_operation(
        &self,
        dataset_handle: &DatasetHandle,
        system_time: DateTime<Utc>,
    ) -> Result<Option<TransformOperation>, InternalError> {
        let span = info_span!("Evaluating transform");
        let _span_guard = span.enter();

        let dataset = self
            .local_repo
            .get_dataset(&dataset_handle.as_local_ref())
            .await
            .int_err()?;
        let output_chain = dataset.as_metadata_chain();

        // TODO: limit traversal depth
        let mut sources: Vec<_> = output_chain
            .iter_blocks()
            .try_filter_map(|(_, b)| async move {
                match b.event {
                    MetadataEvent::SetTransform(st) => Ok(Some(st)),
                    MetadataEvent::SetPollingSource(_) => {
                        Err("Transform called on non-derivative dataset"
                            .int_err()
                            .into())
                    }
                    _ => Ok(None),
                }
            })
            .try_collect()
            .await
            .int_err()?;

        // TODO: source could've changed several times
        if sources.len() > 1 {
            unimplemented!("Transform evolution is not yet supported");
        }

        let source = sources.pop().unwrap();
        debug!(?source, "Transforming using source");

        if futures::stream::iter(&source.inputs)
            .map(|input| input.id.as_ref().unwrap().as_local_ref())
            .then(|input_ref| async move { self.is_never_pulled(&input_ref).await })
            .any_ok(|never_pulled| *never_pulled)
            .await?
        {
            info!("Not processing because one of the inputs was never pulled");
            return Ok(None);
        }

        // Prepare inputs
        let input_slices: Vec<_> = futures::stream::iter(&source.inputs)
            .then(|input| self.get_input_slice(input.id.as_ref().unwrap(), output_chain))
            .try_collect()
            .await
            .int_err()?;

        let query_inputs: Vec<_> = futures::stream::iter(&input_slices)
            .then(|i| self.to_query_input(i, None))
            .try_collect()
            .await
            .int_err()?;

        // Nothing to do?
        if query_inputs
            .iter()
            .all(|i| i.data_paths.is_empty() && i.explicit_watermarks.is_empty())
        {
            return Ok(None);
        }

        let vocab = self.get_vocab(&dataset_handle.as_local_ref()).await?;

        // TODO: Checkpoint hash should be contained in metadata explicitly, not inferred
        let prev_checkpoint = output_chain
            .iter_blocks()
            .filter_map_ok(|(_, b)| b.event.into_variant::<ExecuteQuery>())
            .filter_map_ok(|b| b.output_checkpoint)
            .try_first()
            .await
            .int_err()?;

        let data_offset_end = output_chain
            .iter_blocks()
            .filter_map_ok(|(_, b)| b.event.into_variant::<ExecuteQuery>())
            .filter_map_ok(|eq| eq.output_data)
            .map_ok(|s| s.interval.end)
            .try_first()
            .await
            .int_err()?;

        // TODO: This service shouldn't know specifics of dataset layouts
        // perhaps it should only receive a staging file to write into from Dataset interface
        let output_layout = self.workspace_layout.dataset_layout(&dataset_handle.name);
        // TODO: Avoid giving engines write access directly to data and checkpoint dirs
        // to prevent accidents and creation of garbage files like .crc
        let out_data_path = output_layout.data_dir.join(".pending");
        let prev_checkpoint_path = prev_checkpoint
            .as_ref()
            .map(|cp| output_layout.checkpoint_path(&cp.physical_hash));
        let new_checkpoint_path = output_layout.checkpoints_dir.join(".pending");

        // Clean up previous state leftovers
        if out_data_path.exists() {
            std::fs::remove_file(&out_data_path).int_err()?;
        }
        if new_checkpoint_path.exists() {
            std::fs::remove_file(&new_checkpoint_path).int_err()?;
        }

        Ok(Some(TransformOperation {
            dataset_handle: dataset_handle.clone(),
            input_slices,
            input_checkpoint: prev_checkpoint.map(|cp| cp.physical_hash),
            request: ExecuteQueryRequest {
                dataset_id: dataset_handle.id.clone(),
                dataset_name: dataset_handle.name.clone(),
                system_time,
                offset: data_offset_end.map(|e| e + 1).unwrap_or(0),
                vocab,
                transform: source.transform,
                inputs: query_inputs,
                prev_checkpoint_path,
                new_checkpoint_path,
                out_data_path,
            },
        }))
    }

    async fn is_never_pulled(&self, dataset_ref: &DatasetRefLocal) -> Result<bool, InternalError> {
        let dataset = self.local_repo.get_dataset(dataset_ref).await.int_err()?;
        Ok(dataset
            .as_metadata_chain()
            .iter_blocks()
            .filter_data_stream_blocks()
            .filter_map_ok(|(_, b)| b.event.output_data)
            .try_first()
            .await
            .int_err()?
            .is_none())
    }

    // TODO: Avoid iterating through output chain multiple times
    async fn get_input_slice(
        &self,
        dataset_id: &DatasetID,
        output_chain: &dyn MetadataChain,
    ) -> Result<InputSlice, InternalError> {
        let input_handle = self
            .local_repo
            .resolve_dataset_ref(&dataset_id.as_local_ref())
            .await
            .int_err()?;
        let input_dataset = self
            .local_repo
            .get_dataset(&input_handle.as_local_ref())
            .await
            .int_err()?;
        let input_chain = input_dataset.as_metadata_chain();

        // Determine last processed input block
        let last_processed_block = output_chain
            .iter_blocks()
            .filter_map_ok(|(_, b)| b.event.into_variant::<ExecuteQuery>())
            .map_ok(|eq| eq.input_slices)
            .flatten_ok()
            .filter_ok(|slice| slice.dataset_id == *dataset_id)
            .filter_map_ok(|slice| slice.block_interval)
            .map_ok(|bi| bi.end)
            .try_first()
            .await
            .int_err()?;

        // Collect unprocessed input blocks
        let blocks_unprocessed: Vec<_> = input_chain
            .iter_blocks()
            .take_while_ok(|(block_hash, _)| Some(block_hash) != last_processed_block.as_ref())
            .try_collect()
            .await
            .int_err()?;

        // Sanity check: First (chronologically) unprocessed block should immediately follow the last processed block
        if let Some((first_unprocessed_hash, first_unprocessed_block)) = blocks_unprocessed.last() {
            if first_unprocessed_block.prev_block_hash != last_processed_block {
                panic!(
                    "Input data for {} is inconsistent - first unprocessed block {} does not imediately follows last processed block {:?}",
                    input_handle, first_unprocessed_hash, last_processed_block
                );
            }
        }

        let block_interval = if blocks_unprocessed.is_empty() {
            None
        } else {
            Some(BlockInterval {
                start: blocks_unprocessed.last().map(|(h, _)| h.clone()).unwrap(),
                end: blocks_unprocessed.first().map(|(h, _)| h.clone()).unwrap(),
            })
        };

        // Determine unprocessed offset range. Can be (None, None) or [start, end]
        let offset_end = blocks_unprocessed
            .iter()
            .filter_map(|(_, b)| b.as_data_stream_block())
            .filter_map(|b| b.event.output_data)
            .map(|s| s.interval.end)
            .next();
        let offset_start = blocks_unprocessed
            .iter()
            .rev()
            .filter_map(|(_, b)| b.as_data_stream_block())
            .filter_map(|b| b.event.output_data)
            .map(|s| s.interval.start)
            .next();
        let data_interval = match (offset_start, offset_end) {
            (None, None) => None,
            (Some(start), Some(end)) if start <= end => Some(OffsetInterval { start, end }),
            _ => panic!(
                "Input data for {} is inconsistent at block interval {:?} - unprocessed offset range ended up as ({:?}, {:?})",
                input_handle, block_interval, offset_start, offset_end
            ),
        };

        Ok(InputSlice {
            dataset_id: dataset_id.to_owned(),
            block_interval,
            data_interval,
        })
    }

    // TODO: Avoid traversing same blocks again
    async fn to_query_input(
        &self,
        slice: &InputSlice,
        vocab_hint: Option<DatasetVocabulary>,
    ) -> Result<ExecuteQueryInput, InternalError> {
        let input_handle = self
            .local_repo
            .resolve_dataset_ref(&slice.dataset_id.as_local_ref())
            .await
            .int_err()?;
        let input_dataset = self
            .local_repo
            .get_dataset(&input_handle.as_local_ref())
            .await
            .int_err()?;
        let input_chain = input_dataset.as_metadata_chain();
        let input_layout = self.workspace_layout.dataset_layout(&input_handle.name);

        // List of part files and watermarks that will be used by the engine
        // Note: Engine will still filter the records by the offset interval
        let mut data_paths = Vec::new();
        let mut explicit_watermarks = Vec::new();

        if let Some(block_interval) = &slice.block_interval {
            let hash_to_stop_at = input_chain
                .get_block(&block_interval.start)
                .await
                .expect("Starting block of the interval not found")
                .prev_block_hash;

            let mut block_stream = input_chain
                .iter_blocks_interval(&block_interval.end, hash_to_stop_at.as_ref(), false)
                .filter_data_stream_blocks();

            while let Some((_, block)) = block_stream.try_next().await.int_err()? {
                if let Some(slice) = &block.event.output_data {
                    data_paths.push(input_layout.data_slice_path(slice));
                }

                if let Some(wm) = block.event.output_watermark {
                    explicit_watermarks.push(Watermark {
                        system_time: block.system_time,
                        event_time: wm,
                    });
                }
            }

            // Note: Order is important, so we reverse it to make chronological
            data_paths.reverse();
            explicit_watermarks.reverse();
        }

        // TODO: Migrate to providing schema directly
        // TODO: Will not work with schema evolution
        let schema_file = if let Some(p) = data_paths.last() {
            p.clone()
        } else {
            let last_slice = input_chain
                .iter_blocks()
                .filter_data_stream_blocks()
                .filter_map_ok(|(_, b)| b.event.output_data)
                .try_first()
                .await
                .int_err()?
                .unwrap();
            input_layout.data_slice_path(&last_slice)
        };

        let vocab = match vocab_hint {
            Some(v) => v,
            None => self.get_vocab(&input_handle.as_local_ref()).await?,
        };

        let is_empty = data_paths.is_empty() && explicit_watermarks.is_empty();

        let input = ExecuteQueryInput {
            dataset_id: input_handle.id.clone(),
            dataset_name: input_handle.name.clone(),
            vocab,
            data_interval: slice.data_interval.clone(),
            data_paths,
            schema_file,
            explicit_watermarks,
        };

        info!(
            %input_handle,
            ?input,
            ?slice,
            is_empty,
            "Computed query input"
        );

        Ok(input)
    }

    // TODO: Avoid iterating through output chain multiple times
    async fn get_vocab(
        &self,
        dataset_ref: &DatasetRefLocal,
    ) -> Result<DatasetVocabulary, InternalError> {
        let dataset = self.local_repo.get_dataset(dataset_ref).await.int_err()?;
        Ok(dataset
            .as_metadata_chain()
            .iter_blocks()
            .filter_map_ok(|(_, b)| b.event.into_variant::<SetVocab>())
            .try_first()
            .await
            .int_err()?
            .map(|sv| sv.into())
            .unwrap_or_default())
    }

    // TODO: Improve error handling
    // Need an inconsistent medata error?
    pub async fn get_verification_plan(
        &self,
        dataset_handle: &DatasetHandle,
        block_range: (Option<Multihash>, Option<Multihash>),
    ) -> Result<Vec<VerificationStep>, VerificationError> {
        let span = info_span!("Preparing transformations replay plan");
        let _span_guard = span.enter();

        let dataset = self
            .local_repo
            .get_dataset(&dataset_handle.as_local_ref())
            .await?;
        let metadata_chain = dataset.as_metadata_chain();

        let head = match block_range.1 {
            None => metadata_chain.get_ref(&BlockRef::Head).await?,
            Some(hash) => hash,
        };
        let tail = block_range.0;

        let mut source = None;
        let mut vocab = None;
        let mut blocks = Vec::new();
        let mut finished_range = false;

        {
            let mut block_stream = metadata_chain.iter_blocks_interval(&head, None, false);

            // TODO: This can be simplified
            while let Some((block_hash, block)) = block_stream.try_next().await? {
                match block.event {
                    MetadataEvent::SetTransform(st) => {
                        if source.is_none() {
                            source = Some(st);
                        } else {
                            // TODO: Support dataset evolution
                            unimplemented!(
                                "Verifying datasets with evolving queries is not yet supported"
                            );
                        }
                    }
                    MetadataEvent::SetVocab(sv) => {
                        if vocab.is_none() {
                            vocab = Some(sv.into())
                        }
                    }
                    MetadataEvent::ExecuteQuery(_) => {
                        if !finished_range {
                            blocks.push((block_hash.clone(), block));
                        }
                    }
                    MetadataEvent::AddData(_) | MetadataEvent::SetPollingSource(_) => {
                        unreachable!()
                    }
                    MetadataEvent::Seed(_)
                    | MetadataEvent::SetAttachments(_)
                    | MetadataEvent::SetInfo(_)
                    | MetadataEvent::SetLicense(_)
                    | MetadataEvent::SetWatermark(_) => (),
                }

                if !finished_range && Some(&block_hash) == tail.as_ref() {
                    finished_range = true;
                }
            }
        }

        // Ensure start_block was found if specified
        if tail.is_some() && !finished_range {
            return Err(InvalidIntervalError {
                head,
                tail: tail.unwrap(),
            }
            .into());
        }

        let source = source.ok_or(
            "Expected a derivative dataset but SetTransform block was not found".int_err(),
        )?;
        let dataset_layout = self.workspace_layout.dataset_layout(&dataset_handle.name);

        let dataset_vocabs: BTreeMap<_, _> = futures::stream::iter(&source.inputs)
            .map(|input| {
                (
                    input.id.clone().unwrap(),
                    input.id.as_ref().unwrap().as_local_ref(),
                )
            })
            .then(|(input_id, input_ref)| async move {
                self.get_vocab(&input_ref)
                    .map_ok(|vocab| (input_id, vocab))
                    .await
            })
            .try_collect()
            .await?;

        let mut plan = Vec::new();

        for (block_hash, block) in blocks.into_iter().rev() {
            let block_t = block.as_typed::<ExecuteQuery>().unwrap();

            let inputs = futures::stream::iter(&block_t.event.input_slices)
                .map(|slice| {
                    (
                        slice,
                        dataset_vocabs
                            .get(&slice.dataset_id)
                            .map(|v| v.clone())
                            .unwrap(),
                    )
                })
                .then(|(slice, vocab)| self.to_query_input(slice, Some(vocab)))
                .try_collect()
                .await?;

            let step = VerificationStep {
                operation: TransformOperation {
                    dataset_handle: dataset_handle.clone(),
                    input_slices: block_t.event.input_slices.clone(),
                    input_checkpoint: block_t.event.input_checkpoint.clone(),
                    request: ExecuteQueryRequest {
                        dataset_id: dataset_handle.id.clone(),
                        dataset_name: dataset_handle.name.clone(),
                        system_time: block.system_time,
                        offset: block_t
                            .event
                            .output_data
                            .as_ref()
                            .map(|s| s.interval.start)
                            .unwrap_or(0), // TODO: Assuming offset does not matter if block is not supposed to produce data
                        transform: source.transform.clone(),
                        vocab: vocab.clone().unwrap_or_default(),
                        inputs,
                        prev_checkpoint_path: block_t
                            .event
                            .input_checkpoint
                            .as_ref()
                            .map(|cp| dataset_layout.checkpoint_path(cp)),
                        new_checkpoint_path: dataset_layout.checkpoints_dir.join(".pending"),
                        out_data_path: dataset_layout.data_dir.join(".pending"),
                    },
                },
                expected_block: block,
                expected_hash: block_hash,
            };

            plan.push(step);
        }

        Ok(plan)
    }

    async fn transform_impl(
        &self,
        dataset_ref: DatasetRefLocal,
        maybe_listener: Option<Arc<dyn TransformListener>>,
    ) -> Result<TransformResult, TransformError> {
        let listener = maybe_listener.unwrap_or_else(|| Arc::new(NullTransformListener));
        let dataset_handle = self.local_repo.resolve_dataset_ref(&dataset_ref).await?;

        let span = info_span!("Transforming dataset", %dataset_handle);
        let _span_guard = span.enter();

        // TODO: There might be more operations to do
        // TODO: Inject time source
        if let Some(operation) = self.get_next_operation(&dataset_handle, Utc::now()).await? {
            let dataset = self
                .local_repo
                .get_dataset(&dataset_handle.as_local_ref())
                .await?;
            let meta_chain = dataset.as_metadata_chain();

            let head = meta_chain.get_ref(&BlockRef::Head).await.int_err()?;

            let head_block = meta_chain.get_block(&head).await.int_err()?;

            Self::do_transform(
                self.engine_provisioner.clone(),
                operation,
                move |new_block, new_data_path, new_checkpoint_path| {
                    Self::commit_transform(
                        dataset_handle,
                        dataset,
                        head,
                        head_block.sequence_number,
                        new_block,
                        new_data_path,
                        new_checkpoint_path,
                    )
                },
                listener,
            )
            .await
        } else {
            listener.begin();
            listener.success(&TransformResult::UpToDate);
            Ok(TransformResult::UpToDate)
        }
    }
}

#[async_trait::async_trait(?Send)]
impl TransformService for TransformServiceImpl {
    async fn transform(
        &self,
        dataset_ref: &DatasetRefLocal,
        maybe_listener: Option<Arc<dyn TransformListener>>,
    ) -> Result<TransformResult, TransformError> {
        info!(
            dataset_ref = ?dataset_ref,
            "Transforming a single dataset"
        );

        self.transform_impl(dataset_ref.clone(), maybe_listener)
            .await
    }

    async fn transform_multi(
        &self,
        dataset_refs: &mut dyn Iterator<Item = DatasetRefLocal>,
        maybe_multi_listener: Option<Arc<dyn TransformMultiListener>>,
    ) -> Vec<(DatasetRefLocal, Result<TransformResult, TransformError>)> {
        let multi_listener =
            maybe_multi_listener.unwrap_or_else(|| Arc::new(NullTransformMultiListener));

        let dataset_refs: Vec<_> = dataset_refs.collect();
        info!(?dataset_refs, "Transforming multiple datasets");

        let mut futures = Vec::new();

        for dataset_ref in &dataset_refs {
            let f = match self.local_repo.resolve_dataset_ref(dataset_ref).await {
                Ok(hdl) => {
                    let maybe_listener = multi_listener.begin_transform(&hdl);
                    self.transform_impl(hdl.into(), maybe_listener)
                }
                // Relying on this call to fail to avoid boxing the futures
                Err(_) => self.transform_impl(dataset_ref.clone(), None),
            };
            futures.push(f);
        }

        let results = futures::future::join_all(futures).await;
        dataset_refs.into_iter().zip(results).collect()
    }

    async fn verify_transform(
        &self,
        dataset_ref: &DatasetRefLocal,
        block_range: (Option<Multihash>, Option<Multihash>),
        _options: VerificationOptions,
        maybe_listener: Option<Arc<dyn VerificationListener>>,
    ) -> Result<VerificationResult, VerificationError> {
        let listener = maybe_listener.unwrap_or(Arc::new(NullVerificationListener {}));

        let dataset_handle = self.local_repo.resolve_dataset_ref(dataset_ref).await?;

        let span = info_span!("Replaying dataset transformations", %dataset_handle, ?block_range);
        let _span_guard = span.enter();

        let verification_plan = self
            .get_verification_plan(&dataset_handle, block_range)
            .await?;
        let num_steps = verification_plan.len();
        listener.begin_phase(VerificationPhase::ReplayTransform);

        for (step_index, step) in verification_plan.into_iter().enumerate() {
            let operation = step.operation;
            let expected_block_hash = step.expected_hash;
            let expected_block = step.expected_block;

            // Will be set during "commit" step
            let mut actual_block = None;
            let mut actual_block_hash = None;

            info!(
                block_hash = %expected_block_hash,
                "Replaying block"
            );

            listener.begin_block(
                &expected_block_hash,
                step_index,
                num_steps,
                VerificationPhase::ReplayTransform,
            );

            let transform_listener = listener
                .clone()
                .get_transform_listener()
                .unwrap_or_else(|| Arc::new(NullTransformListener));

            Self::do_transform(
                self.engine_provisioner.clone(),
                operation,
                |mut new_block: MetadataBlock, new_data_path, new_checkpoint_path| async {
                    let new_block_t = new_block.as_typed_mut::<ExecuteQuery>().unwrap();
                    let expected_block_t = expected_block.as_typed::<ExecuteQuery>().unwrap();

                    // Cleanup not needed outputs
                    if new_block_t.event.output_data.is_some() {
                        std::fs::remove_file(new_data_path).int_err()?;
                    }
                    if new_block_t.event.output_checkpoint.is_some() {
                        std::fs::remove_file(new_checkpoint_path).int_err()?;
                    }

                    // We overwrite the physical hash with the expected one because Parquet format is non-reproducible
                    // We rely only on logical hash for equivalence test
                    if let Some(slice) = &mut new_block_t.event.output_data {
                        if let Some(expected_physical_hash) = expected_block_t
                            .event
                            .output_data
                            .as_ref()
                            .map(|s| &s.physical_hash)
                        {
                            slice.physical_hash = expected_physical_hash.clone();
                        }
                    }

                    // We're not considering checkpoints in equivalence checks.
                    if let Some(actual_checkpoint) = &mut new_block_t.event.output_checkpoint {
                        if let Some(expected_checkpoint) = &expected_block_t.event.output_checkpoint
                        {
                            actual_checkpoint.physical_hash =
                                expected_checkpoint.physical_hash.clone();
                        }
                    }

                    // Link new block
                    new_block.prev_block_hash = expected_block.prev_block_hash.clone();
                    new_block.sequence_number = expected_block.sequence_number;

                    // All we care about is the new block and its hash
                    actual_block_hash = Some(Multihash::from_digest_sha3_256(
                        &FlatbuffersMetadataBlockSerializer
                            .write_manifest(&new_block)
                            .int_err()?,
                    ));

                    actual_block = Some(new_block);

                    Ok(TransformResult::Updated {
                        old_head: expected_block.prev_block_hash.clone().unwrap(),
                        new_head: actual_block_hash.clone().unwrap(),
                        num_blocks: 1,
                    })
                },
                transform_listener,
            )
            .await?;

            let actual_block = actual_block.unwrap();
            let actual_block_hash = actual_block_hash.unwrap();
            debug!(expected = ?expected_block, actual = ?actual_block, "Comparing results");

            if expected_block_hash != actual_block_hash || expected_block != actual_block {
                info!(block_hash = %expected_block_hash, expected = ?expected_block, actual = ?actual_block, "Block invalid");

                let err = VerificationError::DataNotReproducible(DataNotReproducible {
                    expected_block_hash,
                    expected_block,
                    actual_block_hash,
                    actual_block,
                });
                listener.error(&err);
                return Err(err);
            }

            info!(block_hash = %expected_block_hash, "Block valid");
            listener.end_block(
                &expected_block_hash,
                step_index,
                num_steps,
                VerificationPhase::ReplayTransform,
            );
        }

        listener.end_phase(VerificationPhase::ReplayTransform);
        Ok(VerificationResult::Valid)
    }

    async fn verify_transform_multi(
        &self,
        _datasets: &mut dyn Iterator<Item = VerificationRequest>,
        _options: VerificationOptions,
        _listener: Option<Arc<dyn VerificationMultiListener>>,
    ) -> Result<VerificationResult, VerificationError> {
        unimplemented!()
    }
}

/////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, PartialEq)]
pub struct TransformOperation {
    pub dataset_handle: DatasetHandle,
    pub input_slices: Vec<InputSlice>,
    pub input_checkpoint: Option<Multihash>,
    pub request: ExecuteQueryRequest,
}

#[derive(Debug)]
pub struct VerificationStep {
    pub operation: TransformOperation,
    pub expected_block: MetadataBlock,
    pub expected_hash: Multihash,
}
