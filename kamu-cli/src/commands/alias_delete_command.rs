// Copyright Kamu Data, Inc. and contributors. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use super::{CLIError, Command};
use kamu::domain::*;
use opendatafabric::*;

use std::sync::Arc;

pub struct AliasDeleteCommand {
    remote_alias_reg: Arc<dyn RemoteAliasesRegistry>,
    dataset: DatasetRefLocal,
    alias: Option<DatasetRefRemote>,
    all: bool,
    pull: bool,
    push: bool,
}

impl AliasDeleteCommand {
    pub fn new(
        remote_alias_reg: Arc<dyn RemoteAliasesRegistry>,
        dataset: DatasetRefLocal,
        alias: Option<DatasetRefRemote>,
        all: bool,
        pull: bool,
        push: bool,
    ) -> Self {
        Self {
            remote_alias_reg,
            dataset,
            alias,
            all,
            pull,
            push,
        }
    }
}

#[async_trait::async_trait(?Send)]
impl Command for AliasDeleteCommand {
    async fn run(&mut self) -> Result<(), CLIError> {
        let mut aliases = self
            .remote_alias_reg
            .get_remote_aliases(&self.dataset)
            .await
            .map_err(CLIError::failure)?;

        let mut count = 0;

        if self.all {
            count += aliases.clear(RemoteAliasKind::Pull)?;
            count += aliases.clear(RemoteAliasKind::Push)?;
        } else if let Some(alias) = &self.alias {
            let both = !self.pull && !self.push;

            if self.pull || both {
                if aliases.delete(alias, RemoteAliasKind::Pull)? {
                    count += 1;
                }
            }
            if self.push || both {
                if aliases.delete(alias, RemoteAliasKind::Push)? {
                    count += 1;
                }
            }
        } else {
            return Err(CLIError::usage_error("Specify either an alias or --all"));
        }

        eprintln!(
            "{}",
            console::style(format!("Deleted {} alias(es)", count))
                .green()
                .bold()
        );

        Ok(())
    }
}
