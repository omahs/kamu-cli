// Copyright Kamu Data, Inc. and contributors. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::{
    path::{Path, PathBuf},
    process::Child,
    sync::Arc,
    time::Duration,
};

use container_runtime::{ContainerRuntime, ContainerRuntimeType, ExecArgs, RunArgs};
use odf::{
    engine::{EngineGrpcClient, ExecuteQueryError},
    ExecuteQueryInput, ExecuteQueryRequest, ExecuteQueryResponseSuccess,
};
use opendatafabric as odf;
use rand::Rng;
use tracing::{info, info_span, warn};

use super::ODFEngineConfig;
use crate::domain::*;
use crate::infra::WorkspaceLayout;

pub struct ODFEngine {
    container_runtime: ContainerRuntime,
    engine_config: ODFEngineConfig,
    image: String,
    workspace_layout: Arc<WorkspaceLayout>,
}

impl ODFEngine {
    const CT_VOLUME_DIR: &'static str = "/opt/engine/volume";

    pub fn new(
        container_runtime: ContainerRuntime,
        engine_config: ODFEngineConfig,
        image: &str,
        workspace_layout: Arc<WorkspaceLayout>,
    ) -> Self {
        Self {
            container_runtime,
            engine_config,
            image: image.to_owned(),
            workspace_layout,
        }
    }

    async fn transform_impl(
        &self,
        run_info: RunInfo,
        request: odf::ExecuteQueryRequest,
    ) -> Result<odf::ExecuteQueryResponseSuccess, EngineError> {
        let engine_container = EngineContainer::new(
            self.container_runtime.clone(),
            self.engine_config.clone(),
            &self.image,
            &run_info,
            vec![(
                self.workspace_layout.datasets_dir.clone(),
                PathBuf::from(Self::CT_VOLUME_DIR),
            )],
        )?;

        let mut client = engine_container.connect_client(&run_info).await?;

        let span = info_span!(
            "Performing engine operation",
            id = engine_container.container_name.as_str(),
            image = self.image.as_str(),
            operation = "execute_query",
            request = ?request,
        );
        let _span_guard = span.enter();

        let response = client.execute_query(request).await;

        info!(?response, "Operation response");

        cfg_if::cfg_if! {
            if #[cfg(unix)] {
                if self.container_runtime.config.runtime == ContainerRuntimeType::Docker {
                    self.container_runtime.exec_shell_cmd(ExecArgs::default(), &engine_container.container_name, &[format!(
                        "chown -R {}:{} {}",
                        users::get_current_uid(),
                        users::get_current_gid(),
                        Self::CT_VOLUME_DIR
                    )]).status()?;
                }
            }
        }

        response.map_err(|e| match e {
            ExecuteQueryError::InvalidQuery(e) => {
                EngineError::invalid_query(e.message, run_info.log_files())
            }
            e @ ExecuteQueryError::EngineInternalError(_) => {
                EngineError::internal(e, run_info.log_files())
            }
            e @ ExecuteQueryError::RpcError(_) => EngineError::internal(e, run_info.log_files()),
        })
    }

    fn to_container_path(&self, host_path: &Path) -> PathBuf {
        let host_path = Self::canonicalize_via_parent(host_path).unwrap();
        let datasets_path = self.workspace_layout.datasets_dir.canonicalize().unwrap();
        let repo_rel_path = host_path.strip_prefix(datasets_path).unwrap();

        let mut container_path = Self::CT_VOLUME_DIR.to_owned();
        container_path.push('/');
        container_path.push_str(&repo_rel_path.to_string_lossy());
        PathBuf::from(container_path)
    }

    fn canonicalize_via_parent(path: &Path) -> Result<PathBuf, std::io::Error> {
        match path.canonicalize() {
            Ok(p) => Ok(p),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                if let Some(parent) = path.parent() {
                    let mut cp = Self::canonicalize_via_parent(parent)?;
                    cp.push(path.file_name().unwrap());
                    Ok(cp)
                } else {
                    Err(e)
                }
            }
            e @ _ => e,
        }
    }
}

#[async_trait::async_trait]
impl Engine for ODFEngine {
    async fn transform(
        &self,
        request: ExecuteQueryRequest,
    ) -> Result<ExecuteQueryResponseSuccess, EngineError> {
        let request_adj = ExecuteQueryRequest {
            prev_checkpoint_path: request
                .prev_checkpoint_path
                .map(|p| self.to_container_path(&p)),
            new_checkpoint_path: self.to_container_path(&request.new_checkpoint_path),
            out_data_path: self.to_container_path(&request.out_data_path),
            inputs: request
                .inputs
                .into_iter()
                .map(|input| ExecuteQueryInput {
                    data_paths: input
                        .data_paths
                        .into_iter()
                        .map(|p| self.to_container_path(&p))
                        .collect(),
                    schema_file: self.to_container_path(&input.schema_file),
                    ..input
                })
                .collect(),
            ..request
        };

        let run_info = RunInfo::new(&self.workspace_layout.run_info_dir);

        self.transform_impl(run_info, request_adj).await
    }
}

/////////////////////////////////////////////////////////////////////////////////////////

struct RunInfo {
    run_id: String,
    logs_dir: PathBuf,
    stdout_path: PathBuf,
    stderr_path: PathBuf,
}

impl RunInfo {
    fn new(logs_dir: &Path) -> Self {
        let run_id: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(10)
            .map(char::from)
            .collect();

        let stdout_path = logs_dir.join(format!("engine-{}.stdout.txt", &run_id));
        let stderr_path = logs_dir.join(format!("engine-{}.stderr.txt", &run_id));

        Self {
            run_id,
            logs_dir: logs_dir.to_owned(),
            stdout_path,
            stderr_path,
        }
    }

    pub fn log_files(&self) -> Vec<PathBuf> {
        let mut logs = self.demux_logs().unwrap_or(Vec::new());
        logs.push(self.stdout_path.clone());
        logs.push(self.stderr_path.clone());
        logs
    }

    // ODF adapters log in bunyan format (JSON per line)
    // To make logs more readable we parse the logs to demultiplex
    // logs from multiple processes into different files
    fn demux_logs(&self) -> Result<Vec<PathBuf>, std::io::Error> {
        use std::collections::BTreeMap;
        use std::fs::File;
        use std::io::{BufRead, Write};

        let mut demuxed: BTreeMap<String, (PathBuf, File)> = BTreeMap::new();
        let file = File::open(&self.stdout_path)?;
        let reader = std::io::BufReader::new(file);
        for line in reader.lines() {
            let line = line?;
            let obj = json::parse(&line).unwrap_or(json::Null);
            let process = &obj["process"];
            let stream = &obj["stream"];
            if process.is_null() || !process.is_string() || stream.is_null() || !stream.is_string()
            {
                continue;
            }

            let filename =
                self.demuxed_filename(process.as_str().unwrap(), stream.as_str().unwrap());

            let file = match demuxed.get_mut(&filename) {
                Some((_, f)) => f,
                None => {
                    let path = self.logs_dir.join(&filename);
                    let f = File::create(&path)?;
                    demuxed.insert(filename.clone(), (path, f));
                    &mut demuxed.get_mut(&filename).unwrap().1
                }
            };

            writeln!(file, "{}", obj["msg"].as_str().unwrap_or_default())?;
        }

        Ok(demuxed.into_values().map(|(path, _)| path).collect())
    }

    fn demuxed_filename(&self, process: &str, stream: &str) -> String {
        format!("engine-{}-{}.{}.txt", &self.run_id, process, stream)
    }
}

/////////////////////////////////////////////////////////////////////////////////////////

struct EngineContainer {
    container_runtime: ContainerRuntime,
    config: ODFEngineConfig,
    container_name: String,
    adapter_host_port: u16,
    engine_process: Child,
}

impl EngineContainer {
    const ADAPTER_PORT: u16 = 2884;

    pub fn new(
        container_runtime: ContainerRuntime,
        config: ODFEngineConfig,
        image: &str,
        run_info: &RunInfo,
        volume_map: Vec<(PathBuf, PathBuf)>,
    ) -> Result<Self, EngineError> {
        let stdout_file = std::fs::File::create(&run_info.stdout_path)?;
        let stderr_file = std::fs::File::create(&run_info.stderr_path)?;

        let container_name = format!("kamu-engine-{}", &run_info.run_id);

        let mut cmd = container_runtime.run_cmd(RunArgs {
            image: image.to_owned(),
            container_name: Some(container_name.clone()),
            volume_map: volume_map,
            user: Some("root".to_owned()),
            expose_ports: vec![Self::ADAPTER_PORT],
            ..RunArgs::default()
        });

        info!(command = ?cmd, image = image, id = container_name.as_str(), "Starting engine");

        let engine_process = KillOnDrop::new(
            cmd.stdout(std::process::Stdio::from(stdout_file)) // Stdio::inherit()
                .stderr(std::process::Stdio::from(stderr_file)) // Stdio::inherit()
                .spawn()
                .map_err(|e| EngineError::internal(e, run_info.log_files()))?,
        );

        let adapter_host_port = container_runtime
            .wait_for_host_port(&container_name, Self::ADAPTER_PORT, config.start_timeout)
            .map_err(|e| EngineError::internal(e, run_info.log_files()))?;

        container_runtime
            .wait_for_socket(adapter_host_port, config.start_timeout)
            .map_err(|e| EngineError::internal(e, run_info.log_files()))?;

        info!(id = container_name.as_str(), "Engine running");

        Ok(Self {
            container_runtime,
            config,
            container_name,
            adapter_host_port,
            engine_process: engine_process.unwrap(),
        })
    }

    pub async fn connect_client(
        &self,
        run_info: &RunInfo,
    ) -> Result<EngineGrpcClient, EngineError> {
        Ok(EngineGrpcClient::connect(
            &self.container_runtime.get_runtime_host_addr(),
            self.adapter_host_port,
        )
        .await
        .map_err(|e| EngineError::internal(e, run_info.log_files()))?)
    }

    pub fn has_exited(&mut self) -> bool {
        match self.engine_process.try_wait() {
            Ok(Some(_)) => true,
            Ok(None) => false,
            Err(_) => true,
        }
    }
}

impl Drop for EngineContainer {
    fn drop(&mut self) {
        if self.has_exited() {
            return;
        }

        info!(id = self.container_name.as_str(), "Shutting down engine");

        cfg_if::cfg_if! {
            if #[cfg(unix)] {
                unsafe {
                    libc::kill(self.engine_process.id() as i32, libc::SIGTERM);
                }

                let start = std::time::Instant::now();
                while (std::time::Instant::now() - start) < self.config.shutdown_timeout {
                    if self.has_exited() {
                        return;
                    }
                    std::thread::sleep(Duration::from_millis(100));
                }

                warn!(id = self.container_name.as_str(), "Engine did not shutdown gracefully, killing");
            }
        }

        let _ = self.engine_process.kill();
    }
}

///////////////////////////////////////////////////////////////////////////////

// TODO: Improve reliability and move this into ContainerRuntime
struct KillOnDrop(Option<Child>);

impl KillOnDrop {
    fn new(child: Child) -> Self {
        Self(Some(child))
    }

    fn unwrap(mut self) -> Child {
        self.0.take().unwrap()
    }
}

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        cfg_if::cfg_if! {
            if #[cfg(unix)] {
                if let Some(child) = self.0.take() {
                    unsafe { libc::kill(child.id() as i32, libc::SIGTERM); }
                }
            } else {
                if let Some(mut child) = self.0.take() {
                    let _ = child.kill();
                }
            }
        }
    }
}
