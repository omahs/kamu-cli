// Copyright Kamu Data, Inc. and contributors. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use kamu::domain::*;
use kamu::infra::*;
use kamu_cli::CLIError;
use opendatafabric::serde::yaml::*;
use opendatafabric::*;

use std::ffi::{OsStr, OsString};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use datafusion::parquet::file::reader::FileReader;
use datafusion::parquet::file::serialized_reader::SerializedFileReader;
use futures::stream::TryStreamExt;
use thiserror::Error;

// Test wrapper on top of CLI library
pub struct Kamu {
    workspace_layout: WorkspaceLayout,
    workspace_path: PathBuf,
    _temp_dir: Option<tempfile::TempDir>,
}

impl Kamu {
    pub fn new<P: Into<PathBuf>>(workspace_path: P) -> Self {
        let workspace_path = workspace_path.into();
        let workspace_layout = WorkspaceLayout::new(workspace_path.join(".kamu"));
        Self {
            workspace_layout,
            workspace_path,
            _temp_dir: None,
        }
    }

    pub async fn new_workspace_tmp() -> Self {
        let temp_dir = tempfile::tempdir().unwrap();
        let inst = Self::new(temp_dir.path());
        let inst = Self {
            _temp_dir: Some(temp_dir),
            ..inst
        };

        inst.execute(["init"]).await.unwrap();

        // TODO: Remove when podman is the default
        inst.execute(["config", "set", "engine.runtime", "podman"])
            .await
            .unwrap();

        inst
    }

    pub fn workspace_path(&self) -> &Path {
        &self.workspace_path
    }

    pub fn dataset_layout(&self, dataset_name: &DatasetName) -> DatasetLayout {
        self.workspace_layout.dataset_layout(dataset_name)
    }

    pub async fn get_last_data_slice(&self, dataset_name: &DatasetName) -> ParquetHelper {
        let local_repo = LocalDatasetRepositoryImpl::new(Arc::new(self.workspace_layout.clone()));

        let dataset = local_repo
            .get_dataset(&dataset_name.as_local_ref())
            .await
            .unwrap();
        let part_file = dataset
            .as_metadata_chain()
            .iter_blocks()
            .filter_data_stream_blocks()
            .filter_map_ok(|(_, b)| b.event.output_data)
            .map_ok(|slice| self.dataset_layout(dataset_name).data_slice_path(&slice))
            .try_first()
            .await
            .unwrap()
            .expect("Data file not found");

        ParquetHelper::open(&part_file)
    }

    pub async fn execute<I, S>(&self, cmd: I) -> Result<(), CommandError>
    where
        I: IntoIterator<Item = S>,
        S: Into<OsString>,
    {
        let mut full_cmd = vec![OsStr::new("kamu").to_owned(), OsStr::new("-q").to_owned()];
        full_cmd.extend(cmd.into_iter().map(|i| i.into()));

        let app = kamu_cli::cli();
        let matches = app.try_get_matches_from(&full_cmd).unwrap();

        kamu_cli::run(self.workspace_layout.clone(), matches)
            .await
            .map_err(|e| CommandError {
                cmd: full_cmd,
                error: e,
            })
    }

    pub async fn add_dataset(&self, dataset_snapshot: DatasetSnapshot) -> Result<(), CommandError> {
        let content = YamlDatasetSnapshotSerializer
            .write_manifest(&dataset_snapshot)
            .unwrap();
        let mut f = tempfile::NamedTempFile::new().unwrap();
        f.as_file().write(&content).unwrap();
        f.flush().unwrap();

        self.execute(["add".as_ref(), f.path().as_os_str()]).await
    }
}

/////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Error)]
#[error("Command {cmd:?} failed: {error}")]
pub struct CommandError {
    cmd: Vec<OsString>,
    error: CLIError,
}

/////////////////////////////////////////////////////////////////////////////////////////

pub struct ParquetHelper {
    pub reader: SerializedFileReader<std::fs::File>,
}

impl ParquetHelper {
    fn new(reader: SerializedFileReader<std::fs::File>) -> Self {
        Self { reader }
    }

    fn open(path: &Path) -> Self {
        Self::new(SerializedFileReader::new(std::fs::File::open(&path).unwrap()).unwrap())
    }

    pub fn column_names(&self) -> Vec<String> {
        self.reader
            .metadata()
            .file_metadata()
            .schema_descr()
            .columns()
            .iter()
            .map(|cd| cd.path().string())
            .collect()
    }

    pub fn records<F, R>(&self, row_mapper: F) -> Vec<R>
    where
        R: Ord,
        F: Fn(datafusion::parquet::record::Row) -> R,
    {
        let mut records: Vec<_> = self
            .reader
            .get_row_iter(None)
            .unwrap()
            .map(row_mapper)
            .collect();

        records.sort();
        records
    }
}
