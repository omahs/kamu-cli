// Copyright Kamu Data, Inc. and contributors. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::{CLIError, Command};
use crate::{output::*, records_writers::TableWriter};
use kamu::domain::*;
use opendatafabric::*;

use futures::TryStreamExt;
use std::sync::Arc;

pub struct AliasListCommand {
    local_repo: Arc<dyn LocalDatasetRepository>,
    remote_alias_reg: Arc<dyn RemoteAliasesRegistry>,
    output_config: Arc<OutputConfig>,
    dataset_ref: Option<DatasetRefLocal>,
}

impl AliasListCommand {
    pub fn new(
        local_repo: Arc<dyn LocalDatasetRepository>,
        remote_alias_reg: Arc<dyn RemoteAliasesRegistry>,
        output_config: Arc<OutputConfig>,
        dataset_ref: Option<DatasetRefLocal>,
    ) -> Self {
        Self {
            local_repo,
            remote_alias_reg,
            output_config,
            dataset_ref,
        }
    }

    // TODO: support multiple format specifiers
    async fn print_machine_readable(&self, datasets: &Vec<DatasetHandle>) -> Result<(), CLIError> {
        use std::io::Write;

        let mut out = std::io::stdout();
        write!(out, "Dataset,Kind,Alias\n")?;

        for ds in datasets {
            let aliases = self
                .remote_alias_reg
                .get_remote_aliases(&ds.as_local_ref())
                .await?;

            for alias in aliases.get_by_kind(RemoteAliasKind::Pull) {
                write!(out, "{},{},{}\n", &ds.name, "pull", &alias)?;
            }
            for alias in aliases.get_by_kind(RemoteAliasKind::Push) {
                write!(out, "{},{},{}\n", &ds.name, "push", &alias)?;
            }
        }

        Ok(())
    }

    async fn print_pretty(&self, datasets: &Vec<DatasetHandle>) -> Result<(), CLIError> {
        use prettytable::*;

        let mut items = 0;
        let mut table = Table::new();
        table.set_format(TableWriter::get_table_format());

        table.set_titles(row![bc->"Dataset", bc->"Kind", bc->"Alias"]);

        for ds in datasets {
            let aliases = self
                .remote_alias_reg
                .get_remote_aliases(&ds.as_local_ref())
                .await?;
            let mut pull_aliases: Vec<_> = aliases
                .get_by_kind(RemoteAliasKind::Pull)
                .map(|a| a.to_string())
                .collect();
            let mut push_aliases: Vec<_> = aliases
                .get_by_kind(RemoteAliasKind::Push)
                .map(|a| a.to_string())
                .collect();
            pull_aliases.sort();
            push_aliases.sort();

            for alias in pull_aliases {
                items += 1;
                table.add_row(Row::new(vec![
                    Cell::new(&ds.name),
                    Cell::new("Pull"),
                    Cell::new(&alias),
                ]));
            }

            for alias in push_aliases {
                items += 1;
                table.add_row(Row::new(vec![
                    Cell::new(&ds.name),
                    Cell::new("Push"),
                    Cell::new(&alias),
                ]));
            }
        }

        // Header doesn't render when there are no data rows in the table
        if items == 0 {
            table.add_row(Row::new(vec![Cell::new(""), Cell::new("")]));
        }

        table.printstd();
        Ok(())
    }
}

#[async_trait::async_trait(?Send)]
impl Command for AliasListCommand {
    async fn run(&mut self) -> Result<(), CLIError> {
        let mut datasets: Vec<_> = if let Some(dataset_ref) = &self.dataset_ref {
            let hdl = self.local_repo.resolve_dataset_ref(dataset_ref).await?;
            vec![hdl]
        } else {
            self.local_repo.get_all_datasets().try_collect().await?
        };

        datasets.sort_by(|a, b| a.name.cmp(&b.name));

        // TODO: replace with formatters
        match self.output_config.format {
            OutputFormat::Table => self.print_pretty(&datasets).await?,
            OutputFormat::Csv => self.print_machine_readable(&datasets).await?,
            _ => unimplemented!("Unsupported format: {:?}", self.output_config.format),
        }

        Ok(())
    }
}
