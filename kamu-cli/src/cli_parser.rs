// Copyright Kamu Data, Inc. and contributors. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use std::net::IpAddr;

use clap::{value_parser, Arg, ArgAction, Command};
use opendatafabric::*;

fn tabular_output_params<'a>(app: Command) -> Command {
    app.args(&[
        Arg::new("output-format")
            .long("output-format")
            .short('o')
            .value_name("FMT")
            .value_parser([
                "table", "csv", "json", "json-ld",
                "json-soa",
                // "vertical",
                // "tsv",
                // "xmlattrs",
                // "xmlelements",
            ])
            .help("Format to display the results in"),
        /*Arg::new("no-color")
            .long("no-color")
            .action(ArgAction::SetTrue)
            .help("Control whether color is used for display"),
        Arg::new("incremental")
            .long("incremental")
            .action(ArgAction::SetTrue)
            .help("Display result rows immediately as they are fetched"),
        Arg::new("no-header")
            .long("no-header")
            .action(ArgAction::SetTrue)
            .help("Whether to show column names in query results"),
        Arg::new("header-interval")
            .long("header-interval")
            .value_name("INT")
            .help("The number of rows between which headers are displayed"),
        Arg::new("csv-delimiter")
            .long("csv-delimiter")
            .value_name("DELIM")
            .help("Delimiter in the csv output format"),
        Arg::new("csv-quote-character")
            .long("csv-quote-character")
            .value_name("CHAR")
            .help("Quote character in the csv output format"),
        Arg::new("null-value")
            .long("null-value")
            .value_name("VAL")
            .help("Use specified string in place of NULL values"),
        Arg::new("number-format")
            .long("number-format")
            .value_name("FMT")
            .help("Format numbers using DecimalFormat pattern"),
        Arg::new("date-format")
            .long("date-format")
            .value_name("FMT")
            .help("Format dates using SimpleDateFormat pattern"),
        Arg::new("time-format")
            .long("time-format")
            .value_name("FMT")
            .help("Format times using SimpleDateFormat pattern"),
        Arg::new("timestamp-format")
            .long("timestamp-format")
            .value_name("FMT")
            .help("Format timestamps using SimpleDateFormat pattern"),*/
    ])
}

pub fn cli() -> Command {
    Command::new(crate::BINARY_NAME)
        .subcommand_required(true)
        .arg_required_else_help(true)
        .version(crate::VERSION)
        .args(&[
            Arg::new("verbose")
                .short('v')
                .action(ArgAction::Count)
                .help("Sets the level of verbosity (repeat for more)"),
            Arg::new("quiet")
                .long("quiet")
                .short('q')
                .action(ArgAction::SetTrue)
                .help("Suppress all non-essential output"),
        ])
        .after_help(indoc::indoc!(
            "
            To get help around individual commands use:
              kamu <command> -h
              kamu <command> <sub-command> -h
            "
        ))
        .subcommands(
            [
                Command::new("add")
                    .about("Add a new dataset or modify an existing one")
                    .args(&[
                        Arg::new("recursive")
                            .short('r')
                            .long("recursive")
                            .action(ArgAction::SetTrue)
                            .help("Recursively search for all manifest in the specified directory"),
                        Arg::new("replace")
                            .long("replace")
                            .action(ArgAction::SetTrue)
                            .help("Delete and re-add datasets that already exist"),
                        Arg::new("manifest")
                            .action(ArgAction::Append)
                            .required(true)
                            .index(1)
                            .help("Dataset manifest reference(s) (path, or URL)"),
                    ])
                    .after_help(indoc::indoc!(
                        "
                    This command creates a new dataset from the provided DatasetSnapshot manifest.

                    Note that after kamu creates a dataset the changes in the source file will \
                    not have any effect unless you run the add command again. When you are \
                    experimenting with adding new dataset you currently may need to delete \
                    and re-add it multiple times until you get your parameters and schema right.

                    In future versions the add command will allow you to modify the structure \
                    of already existing datasets (e.g. changing schema in a compatible way).

                    ### Examples ###

                    Add a root/derivative dataset from local manifest:

                        kamu add org.example.data.yaml

                    Add datasets from all manifests found in the current directory:

                        kamu add --recursive .

                    Add a dataset from manifest hosted externally (e.g. on GihHub):

                        kamu add https://raw.githubusercontent.com/kamu-data/kamu-repo-contrib\
                        /master/ca.bankofcanada.exchange-rates.daily.yaml

                    To add dataset from a repository see `kamu pull` command.
                    "
                    )),
                Command::new("complete")
                    .about("Completes a command in the shell")
                    .hide(true)
                    .arg(Arg::new("input").required(true).index(1))
                    .arg(
                        Arg::new("current")
                            .value_parser(value_parser!(usize))
                            .required(true)
                            .index(2),
                    )
                    .after_help(indoc::indoc!(
                        "
                    This hidden command is called by shell completions to use domain knowledge \
                    to complete commands and arguments.

                    ### Examples ###

                    Should complete to \"new\":

                        kamu complete \"kamu ne\" 1

                    Should complete to \"--derivative\":

                        kamu complete \"kamu new --de\" 2
                    "
                    )),
                Command::new("completions")
                    .about("Generate tab-completion scripts for your shell")
                    .after_help(indoc::indoc!(
                        "
                    The command outputs on `stdout`, allowing you to re-direct the output to the \
                    file of your choosing. Where you place the file will depend on which shell \
                    and which operating system you are using. Your particular configuration may \
                    also determine where these scripts need to be placed.

                    Here are some common set ups:

                    #### BASH ####

                    Append the following to your `~/.bashrc`:

                        source <(kamu completions bash)

                    You will need to reload your shell session (or execute the same command in \
                    your current one) for changes to take effect.

                    #### ZSH ####

                    Append the following to your `~/.zshrc`:

                        autoload -U +X bashcompinit && bashcompinit
                        source <(kamu completions bash)

                    Please contribute a guide for your favorite shell!
                    "
                    ))
                    .arg(Arg::new("shell").required(true).value_parser(
                        clap::builder::EnumValueParser::<clap_complete::Shell>::new(),
                    )),
                Command::new("config")
                    .about("Get or set configuration options")
                    .subcommand_required(true)
                    .arg_required_else_help(true)
                    .subcommands([
                        Command::new("list")
                            .about("Display current configuration combined from all config files")
                            .args(&[
                                Arg::new("user")
                                    .long("user")
                                    .action(ArgAction::SetTrue)
                                    .help("Show only user scope configuration"),
                                Arg::new("with-defaults")
                                    .long("with-defaults")
                                    .action(ArgAction::SetTrue)
                                    .help("Show configuration with all default values applied"),
                            ]),
                        Command::new("get")
                            .about("Get current configuration value")
                            .args(&[
                                Arg::new("user")
                                    .long("user")
                                    .action(ArgAction::SetTrue)
                                    .help("Operate on the user scope configuration file"),
                                Arg::new("with-defaults")
                                    .long("with-defaults")
                                    .action(ArgAction::SetTrue)
                                    .help(
                                        "Get default value if config option is not explicitly set",
                                    ),
                                Arg::new("cfgkey")
                                    .required(true)
                                    .index(1)
                                    .help("Path to the config option"),
                            ]),
                        Command::new("set")
                            .about("Set or unset configuration value")
                            .args(&[
                                Arg::new("user")
                                    .long("user")
                                    .action(ArgAction::SetTrue)
                                    .help("Operate on the user scope configuration file"),
                                Arg::new("cfgkey")
                                    .required(true)
                                    .index(1)
                                    .help("Path to the config option"),
                                Arg::new("value").index(2).help("New value to set"),
                            ]),
                    ])
                    .after_help(indoc::indoc!(
                        "
                    Configuration in `kamu` is managed very similarly to `git`. Starting with \
                    your current workspace and going up the directory tree you can have \
                    multiple `.kamuconfig` YAML files which are all merged together to get the \
                    resulting config.

                    Most commonly you will have a workspace-scoped config inside the `.kamu` \
                    directory and the user-scoped config residing in your home directory.

                    ### Examples ###

                    List current configuration as combined view of config files:

                        kamu config list

                    Get current configuration value:

                        kamu config get engine.runtime

                    Set configuration value in workspace scope:

                        kamu config set engine.runtime podman

                    Set configuration value in user scope:

                        kamu config set --user engine.runtime podman

                    Unset or revert to default value:

                        kamu config set --user engine.runtime
                    "
                    )),
                Command::new("delete")
                    .about("Delete a dataset")
                    .args(&[
                        Arg::new("all")
                            .short('a')
                            .long("all")
                            .action(ArgAction::SetTrue)
                            .help("Delete all datasets in the workspace"),
                        Arg::new("recursive")
                            .short('r')
                            .long("recursive")
                            .action(ArgAction::SetTrue)
                            .help("Also delete all transitive dependencies of specified datasets"),
                        Arg::new("dataset")
                            .action(ArgAction::Append)
                            .index(1)
                            .required(true)
                            .value_parser(value_parse_dataset_ref_local)
                            .help("Local dataset reference(s)"),
                        Arg::new("yes")
                            .short('y')
                            .long("yes")
                            .action(ArgAction::SetTrue)
                            .help("Don't ask for confirmation"),
                    ])
                    .after_help(indoc::indoc!(
                        "
                    This command deletes the dataset from your workspace, including both \
                    metadata and the raw data.

                    Take great care when deleting root datasets. If you have not pushed \
                    your local changes to a repository - the data will be lost.

                    Deleting a derivative dataset is usually not a big deal, since they \
                    can always be reconstructed, but it will disrupt downstream consumers.

                    ### Examples ###

                    Delete a local dataset:

                        kamu delete my.dataset
                    "
                    )),
                Command::new("init")
                    .about("Initialize an empty workspace in the current directory")
                    .args(&[
                        Arg::new("pull-images")
                            .long("pull-images")
                            .action(ArgAction::SetTrue)
                            .help("Only pull container images and exit"),
                        Arg::new("pull-test-images")
                            .long("pull-test-images")
                            .action(ArgAction::SetTrue)
                            .hide(true)
                            .help("Only pull test-related container images and exit"),
                        Arg::new("list-only")
                            .long("list-only")
                            .action(ArgAction::SetTrue)
                            .hide(true)
                            .help("List image names instead of pulling"),
                    ])
                    .after_help(indoc::indoc!(
                        "
                    A workspace is where kamu stores all the important information about \
                    datasets (metadata) and in some cases raw data.

                    It is recommended to create one kamu workspace per data science project, \
                    grouping all related datasets together.

                    Initializing a workspace creates a `.kamu` directory contains dataset \
                    metadata, data, and all supporting files (configs, known repositories etc.).
                    "
                    )),
                Command::new("inspect")
                    .about("Group of commands for exploring dataset metadata")
                    .subcommand_required(true)
                    .arg_required_else_help(true)
                    .subcommands([
                        Command::new("lineage")
                            .about("Shows the dependency tree of a dataset")
                            .args(&[
                                Arg::new("output-format")
                                    .long("output-format")
                                    .short('o')
                                    .value_name("FMT")
                                    .value_parser(["shell", "dot", "csv", "html"])
                                    .help("Format of an output"),
                                Arg::new("browse")
                                    .long("browse")
                                    .short('b')
                                    .action(ArgAction::SetTrue)
                                    .help("Produce HTML and open it in a browser"),
                                Arg::new("dataset")
                                    .action(ArgAction::Append)
                                    .index(1)
                                    .required(true)
                                    .value_parser(value_parse_dataset_ref_local)
                                    .help("Local dataset reference(s)"),
                            ])
                            .after_help(indoc::indoc!(
                                "
                            Presents the dataset-level lineage that includes current \
                            and past dependencies.

                            ### Examples ###

                            Show lineage of a single dataset:

                                kamu inspect lineage my.dataset

                            Show lineage graph of all datasets in a browser:

                                kamu inspect lineage -b

                            Render the lineage graph into a png image (needs graphviz installed):

                                kamu inspect lineage -o dot | dot -Tpng > depgraph.png
                            "
                            )),
                        Command::new("query")
                            .about("Shows the transformations used by a derivative dataset")
                            .args(&[Arg::new("dataset")
                                .required(true)
                                .index(1)
                                .value_parser(value_parse_dataset_ref_local)
                                .help("Local dataset reference")])
                            .after_help(indoc::indoc!(
                                "
                            This command allows you to audit the transformations performed \
                            by a derivative dataset and their evolution. Such audit is an \
                            important step in validating the trustworthiness of data (see \
                            `kamu verify` command).
                            "
                            )),
                        Command::new("schema")
                            .about("Shows the dataset schema")
                            .args(&[
                                Arg::new("dataset")
                                    .required(true)
                                    .index(1)
                                    .value_parser(value_parse_dataset_ref_local)
                                    .help("Local dataset reference"),
                                Arg::new("output-format")
                                    .long("output-format")
                                    .short('o')
                                    .value_name("FMT")
                                    .value_parser(["ddl", "parquet", "json"])
                                    .help("Format of an output"),
                            ])
                            .after_help(indoc::indoc!(
                                "
                            Displays the schema of the dataset. Note that dataset schemas can \
                            evolve over time and by default the latest schema will be shown.

                            ### Examples ###

                            Show logical schema of a dataset in the DDL format:

                                kamu inspect schema my.dataset

                            Show physical schema of the underlying Parquet files:

                                kamu inspect schema my.dataset -o parquet
                            "
                            )),
                    ]),
                tabular_output_params(
                    Command::new("list")
                        .about("List all datasets in the workspace")
                        .args(&[Arg::new("wide")
                            .long("wide")
                            .short('w')
                            .action(ArgAction::Count)
                            .help("Show more details (repeat for more)")])
                        .after_help(indoc::indoc!(
                            "
                        ### Examples ###

                        To see a human-friendly list of datasets in your workspace:

                            kamu list

                        To see more details:

                            kamu list -w

                        To get a machine-readable list of datasets:

                            kamu list -o csv
                        "
                        )),
                ),
                Command::new("log")
                    .about("Shows dataset metadata history")
                    .args(&[
                        Arg::new("dataset")
                            .required(true)
                            .index(1)
                            .value_parser(value_parse_dataset_ref_local)
                            .help("Local dataset reference"),
                        Arg::new("output-format")
                            .long("output-format")
                            .short('o')
                            .value_name("FMT")
                            .value_parser(["yaml"]),
                        Arg::new("filter")
                            .long("filter")
                            .short('f')
                            .value_name("FLT")
                            .value_parser(validate_log_filter),
                        Arg::new("limit")
                            .long("limit")
                            .value_parser(value_parser!(usize))
                            .default_value("500")
                            .help("Maximum number of blocks to display"),
                    ])
                    .after_help(indoc::indoc!(
                        "
                    Metadata of a dataset contains historical record of everything that \
                    ever influenced how data currently looks like.

                    This includes events such as:
                    - Data ingestion / transformation
                    - Change of query
                    - Change of schema
                    - Change of source URL or other ingestion steps in a root dataset

                    Use this command to explore how dataset evolved over time.

                    ### Examples ###

                    Show brief summaries of individual metadata blocks:

                        kamu log org.example.data

                    Show detailed content of all blocks:

                        kamu log -o yaml org.example.data

                    Using a filter to inspect blocks containing query changes of a \
                    derivative dataset:

                        kamu log -o yaml --filter source org.example.data
                    "
                    )),
                Command::new("new")
                    .about("Creates a new dataset manifest from a template")
                    .args(&[
                        Arg::new("root")
                            .long("root")
                            .action(ArgAction::SetTrue)
                            .help("Create a root dataset"),
                        Arg::new("derivative")
                            .long("derivative")
                            .action(ArgAction::SetTrue)
                            .help("Create a derivative dataset"),
                        Arg::new("name")
                            .required(true)
                            .index(1)
                            .value_parser(value_parse_dataset_name)
                            .help("Name of the new dataset"),
                    ])
                    .after_help(indoc::indoc!(
                        "
                    This command will create a dataset manifest from a template allowing \
                    you to customize the most relevant parts without having to remember \
                    the exact structure of the yaml file.

                    ### Examples ###

                    Create `org.example.data.yaml` file from template in the current directory:

                        kamu new org.example.data --root
                    "
                    )),
                Command::new("notebook")
                    .about("Starts the notebook server for exploring the data in the workspace")
                    .after_help(indoc::indoc!(
                        "
                    This command will run the Jupyter server and the Spark engine connected \
                    together, letting you query data with SQL before pulling it into the \
                    notebook for final processing and visualization.

                    For more information check out notebook examples at \
                    https://github.com/kamu-data/kamu-cli
                    "
                    ))
                    .arg(
                        Arg::new("env")
                            .short('e')
                            .long("env")
                            .value_name("VAR")
                            .action(ArgAction::Append)
                            .help(concat!(
                                "Pass specified environment variable into the notebook ",
                                "(e.g. `-e VAR` or `-e VAR=foo`)"
                            )),
                    ),
                Command::new("pull")
                    .about("Pull new data into the datasets")
                    .args(&[
                        Arg::new("all")
                            .short('a')
                            .long("all")
                            .action(ArgAction::SetTrue)
                            .help("Pull all datasets in the workspace"),
                        Arg::new("recursive")
                            .short('r')
                            .long("recursive")
                            .action(ArgAction::SetTrue)
                            .help("Also pull all transitive dependencies of specified datasets"),
                        Arg::new("fetch-uncacheable")
                            .long("fetch-uncacheable")
                            .action(ArgAction::SetTrue)
                            .help("Pull latest data from the uncacheable data sources"),
                        Arg::new("dataset")
                            .action(ArgAction::Append)
                            .index(1)
                            .value_parser(value_parse_dataset_ref_any)
                            .help("Local or remote dataset reference(s)"),
                        Arg::new("as")
                            .long("as")
                            .value_parser(value_parse_dataset_name)
                            .value_name("NAME")
                            .help("Local name of a dataset to use when syncing from a repository"),
                        Arg::new("no-alias")
                            .long("no-alias")
                            .action(ArgAction::SetTrue)
                            .help(
                                "Don't automatically add a remote push alias for this destination",
                            ),
                        Arg::new("fetch")
                            .long("fetch")
                            .value_name("SRC")
                            .help("Data location (path or URL)"),
                        Arg::new("set-watermark")
                            .long("set-watermark")
                            .value_name("TIME")
                            .help(concat!(
                                "Injects a manual watermark into the dataset to signify that ",
                                "no data is expected to arrive with event time that precedes it"
                            )),
                        Arg::new("force")
                            .short('f')
                            .long("force")
                            .action(ArgAction::SetTrue)
                            .help(
                                "Overwrite local version with remote, \
                                even if revisions have diverged",
                            ),
                    ])
                    .after_help(indoc::indoc!(
                        "
                    Pull is a multi-functional command that lets you make changes to a local \
                    dataset. Depending on the parameters and the types of datasets involved \
                    it can be used to:
                    - Ingest new data into a root dataset from an external source
                    - Run transformations on a derivative dataset to process previously unseen data
                    - Pull dataset from a remote repository into your workspace
                    - Update watermark on a dataset

                    ### Examples ###

                    Fetch latest data in a specific dataset:

                        kamu pull org.example.data

                    Fetch latest data for the entire dependency tree of a dataset:

                        kamu pull --recursive org.example.derivative

                    Refresh data of all datasets in the workspace:

                        kamu pull --all

                    Fetch dataset from a registered repository:

                        kamu pull kamu/org.example.data

                    Fetch dataset from a URL (optionally renaming it):

                        kamu pull ipfs://bafy...a0dx/data
                        kamu pull s3://my-bucket.example.org/odf/org.example.data
                        kamu pull s3+https://example.org:5000/data --as org.example.data

                    Advance the watermark of a dataset:

                        kamu pull --set-watermark 2020-01-01 org.example.data

                    Ingest data into the root dataset from file or URL (format should match \
                    one expected by the 'prepare' step):

                        kamu pull org.example.data --fetch path/to/data.csv
                        kamu pull org.example.data --fetch https://example.com/data.csv
                    "
                    )),
                Command::new("push")
                    .about("Push local data into a repository")
                    .args(&[
                        Arg::new("all")
                            .short('a')
                            .long("all")
                            .action(ArgAction::SetTrue)
                            .help("Push all datasets in the workspace"),
                        Arg::new("recursive")
                            .short('r')
                            .long("recursive")
                            .action(ArgAction::SetTrue)
                            .help("Also push all transitive dependencies of specified datasets"),
                        Arg::new("no-alias")
                            .long("no-alias")
                            .action(ArgAction::SetTrue)
                            .help(
                                "Don't automatically add a remote push alias for this destination",
                            ),
                        Arg::new("dataset")
                            .action(ArgAction::Append)
                            .index(1)
                            .value_parser(value_parse_dataset_ref_any)
                            .help("Local or remote dataset reference(s)"),
                        Arg::new("to")
                            .long("to")
                            .value_parser(value_parse_dataset_ref_remote)
                            .value_name("REM")
                            .help("Remote alias or a URL to push to"),
                        Arg::new("force")
                            .short('f')
                            .long("force")
                            .action(ArgAction::SetTrue)
                            .help(
                                "Overwrite remote version with local, \
                                even if revisions have diverged",
                            ),
                    ])
                    .after_help(indoc::indoc!(
                        "
                    Use this command to share your new dataset or new data with others. \
                    All changes performed by this command are atomic and non-destructive. \
                    This command will analyze the state of the dataset at the repository \
                    and will only upload data and metadata that wasn't previously seen.

                    Similarly to git, if someone else modified the dataset concurrently \
                    with you - your push will be rejected and you will have to resolve \
                    the conflict.

                    ### Examples ###

                    Sync dataset to a destination URL:

                        kamu push org.example.data --to \
                        s3://my-bucket.example.org/odf/org.example.data

                    Sync dataset to a named repository (see `kamu repo` command group):

                        kamu push org.example.data --to kamu-hub/org.example.data

                    Sync dataset that already has a push alias:

                        kamu push org.example.data

                    Add dataset to local IPFS node and update IPNS entry to the new CID:

                        kamu push org.example.data --to ipns://k5..zy
                    "
                    )),
                Command::new("rename")
                    .about("Rename a dataset")
                    .args(&[
                        Arg::new("dataset")
                            .required(true)
                            .index(1)
                            .value_parser(value_parse_dataset_ref_local)
                            .help("Dataset reference"),
                        Arg::new("name")
                            .required(true)
                            .index(2)
                            .help("The new name to give it"),
                    ])
                    .after_help(indoc::indoc!(
                        "
                    Use this command to rename a dataset in your local workspace.

                    ### Examples ###

                    Renaming is often useful when you pull a remote dataset by URL and it \
                    gets auto-assigned not the most convenient name:

                        kamu pull ipfs://bafy...a0da
                        kamu rename bafy...a0da my.dataset
                    "
                    )),
                Command::new("reset")
                    .about("Revert the dataset back to the specified state")
                    .args(&[
                        Arg::new("dataset")
                            .required(true)
                            .index(1)
                            .value_parser(value_parse_dataset_ref_local)
                            .help("ID of the dataset"),
                        Arg::new("hash")
                            .required(true)
                            .index(2)
                            .value_parser(value_parse_multihash)
                            .help("Hash of the block to reset to"),
                        Arg::new("yes")
                            .short('y')
                            .long("yes")
                            .action(ArgAction::SetTrue)
                            .help("Don't ask for confirmation"),
                    ])
                    .after_help(indoc::indoc!(
                        r"
                Resetting a dataset to the specified block erases all metadata blocks \
                that followed it and deletes all data added since that point. This can \
                sometimes be useful to resolve conflicts, but otherwise should be used with care.

                Keep in mind that blocks that were pushed to a repository could've \
                been already observed by other people, so resetting the history will not let \
                you take that data back.
                "
                    )),
                Command::new("repo")
                    .about("Manage set of tracked repositories")
                    .subcommand_required(true)
                    .arg_required_else_help(true)
                    .subcommands([
                        Command::new("add")
                            .about("Adds a repository")
                            .after_help(indoc::indoc!(
                                "
                            For Local Filesystem basic repository use following URL formats:
                                file:///home/me/example/repository
                                file:///c:/Users/me/example/repository

                            For S3-compatible basic repository use following URL formats:
                                s3://bucket.my-company.example
                                s3+http://my-minio-server:9000/bucket
                                s3+https://my-minio-server:9000/bucket
                        "
                            ))
                            .args(&[
                                Arg::new("name")
                                    .required(true)
                                    .index(1)
                                    .value_parser(value_parse_repository_name)
                                    .help("Local alias of the repository"),
                                Arg::new("url")
                                    .required(true)
                                    .index(2)
                                    .help("URL of the repository"),
                            ]),
                        Command::new("delete")
                            .about("Deletes a reference to repository")
                            .args(&[
                                Arg::new("all")
                                    .short('a')
                                    .long("all")
                                    .action(ArgAction::SetTrue)
                                    .help("Delete all known repositories"),
                                Arg::new("repository")
                                    .action(ArgAction::Append)
                                    .index(1)
                                    .value_parser(value_parse_repository_name)
                                    .help("Repository name(s)"),
                                Arg::new("yes")
                                    .short('y')
                                    .long("yes")
                                    .action(ArgAction::SetTrue)
                                    .help("Don't ask for confirmation"),
                            ]),
                        tabular_output_params(
                            Command::new("list").about("Lists known repositories"),
                        ),
                        Command::new("alias")
                            .about("Manage set of remote aliases associated with datasets")
                            .subcommand_required(true)
                            .arg_required_else_help(true)
                            .subcommands([
                                tabular_output_params(
                                    Command::new("list").about("Lists remote aliases").args(&[
                                        Arg::new("dataset")
                                            .index(1)
                                            .value_parser(value_parse_dataset_ref_local)
                                            .help("Local dataset reference"),
                                    ]),
                                ),
                                Command::new("add")
                                    .about("Adds a remote alias to a dataset")
                                    .args(&[
                                        Arg::new("dataset")
                                            .required(true)
                                            .index(1)
                                            .value_parser(value_parse_dataset_ref_local)
                                            .help("Local dataset reference"),
                                        Arg::new("alias")
                                            .required(true)
                                            .index(2)
                                            .value_parser(value_parse_dataset_ref_remote)
                                            .help("Remote dataset name"),
                                        Arg::new("push")
                                            .long("push")
                                            .action(ArgAction::SetTrue)
                                            .help("Add a push alias"),
                                        Arg::new("pull")
                                            .long("pull")
                                            .action(ArgAction::SetTrue)
                                            .help("Add a pull alias"),
                                    ]),
                                Command::new("delete")
                                    .about("Deletes a remote alias associated with a dataset")
                                    .args(&[
                                        Arg::new("all")
                                            .short('a')
                                            .long("all")
                                            .action(ArgAction::SetTrue)
                                            .help("Delete all aliases"),
                                        Arg::new("dataset")
                                            .required(true)
                                            .index(1)
                                            .value_parser(value_parse_dataset_ref_local)
                                            .help("Local dataset reference"),
                                        Arg::new("alias")
                                            .index(2)
                                            .value_parser(value_parse_dataset_ref_remote)
                                            .help("Remote dataset name"),
                                        Arg::new("push")
                                            .long("push")
                                            .action(ArgAction::SetTrue)
                                            .help("Add a push alias"),
                                        Arg::new("pull")
                                            .long("pull")
                                            .action(ArgAction::SetTrue)
                                            .help("Add a pull alias"),
                                    ]),
                            ])
                            .after_help(indoc::indoc!(
                                "
                            When you pull and push datasets from repositories kamu uses aliases \
                            to let you avoid specifying the full remote referente each time. \
                            Aliases are usually created the first time you do a push or pull \
                            and saved for later. If you have an unusual setup (e.g. pushing to \
                            multiple repositories) you can use this command to manage the aliases.

                            ### Examples ###

                            List all aliases:

                                kamu repo alias list

                            List all aliases of a specific dataset:

                                kamu repo alias list org.example.data

                            Add a new pull alias:

                                kamu repo alias add --pull \
                                org.example.data kamu.dev/me/org.example.data
                            "
                            )),
                    ])
                    .after_help(indoc::indoc!(
                        "
                    Repositories are nodes on the network that let users exchange datasets. \
                    In the most basic form, a repository can simply be a location where the \
                    dataset files are hosted over one of the supported file or object-based \
                    data transfer protocols. The owner of a dataset will have push privileges \
                    to this location, while other participants can pull data from it.

                    ### Examples ###

                    Show available repositories:

                        kamu repo list

                    Add S3 bucket as a repository:

                        kamu repo add example-repo s3://bucket.my-company.example
                    "
                    )),
                tabular_output_params(
                    Command::new("search")
                        .about("Searches for datasets in the registered repositories")
                        .args(&[
                            Arg::new("query")
                                .index(1)
                                .value_name("QUERY")
                                .help("Search terms"),
                            Arg::new("repo")
                                .long("repo")
                                .action(ArgAction::Append)
                                .value_name("REPO")
                                .value_parser(value_parse_repository_name)
                                .help("Repository name(s) to search in"),
                        ])
                        .after_help(indoc::indoc!(
                            "
                        Search is delegated to the repository implementations and its \
                        capabilities depend on the type of the repo. Whereas smart \
                        repos may support advanced full-text search, simple storage-only \
                        repos may be limited to a substring search by dataset name.

                        ### Examples ###

                        Search all repositories:

                            kamu search covid19

                        Search only specific repositories:

                            kamu search covid19 --repo kamu --repo statcan.gc.ca
                        "
                        )),
                ),
                tabular_output_params(
                    Command::new("sql")
                        .about("Executes an SQL query or drops you into an SQL shell")
                        .subcommand(
                            Command::new("server").about("Run JDBC server only").args(&[
                                Arg::new("address")
                                    .long("address")
                                    .value_parser(value_parser!(IpAddr))
                                    .default_value("127.0.0.1")
                                    .help("Expose JDBC server on specific network interface"),
                                Arg::new("port")
                                    .long("port")
                                    .default_value("10000")
                                    .value_parser(value_parser!(u16))
                                    .help("Expose JDBC server on specific port"),
                                Arg::new("livy")
                                    .long("livy")
                                    .action(ArgAction::SetTrue)
                                    .help("Run Livy server instead of JDBC")
                                    .hide(true),
                            ]),
                        )
                        .args(&[
                            Arg::new("url").long("url").value_name("URL").help(
                                "URL of a running JDBC server (e.g jdbc:hive2://example.com:10000)",
                            ),
                            Arg::new("command")
                                .short('c')
                                .long("command")
                                .value_name("CMD")
                                .help("SQL command to run"),
                            Arg::new("script")
                                .long("script")
                                .value_name("FILE")
                                .help("SQL script file to execute"),
                            Arg::new("engine")
                                .long("engine")
                                .value_parser(["spark", "datafusion"])
                                .value_name("ENG")
                                .help("Engine type to use for this SQL session"),
                        ])
                        .after_help(indoc::indoc!(
                            "
                        SQL shell allows you to explore data of all dataset in your \
                        workspace using one of the supported data processing engines. \
                        This can be a great way to prepare and test a query that you \
                        cal later turn into derivative dataset.

                        ### Examples ###

                        Drop into SQL shell:

                            kamu sql

                        Execute SQL command and return its output in CSV format:

                            kamu sql -c 'SELECT * FROM `org.example.data` LIMIT 10' -o csv

                        Run SQL server to use with external data processing tools:

                            kamu sql server --address 0.0.0.0 --port 8080

                        Connect to a remote SQL server:

                            kamu sql --url jdbc:hive2://example.com:10000

                        Note: Currently when connecting to a remote SQL kamu server \
                        you will need to manually instruct it to load datasets from \
                        the data files. This can be done using the following command:

                            CREATE TEMP VIEW `my.dataset` AS \
                            (SELECT * FROM parquet.`kamu_data/my.dataset`);
                    "
                        )),
                ),
                Command::new("system")
                    .about("Command group for system-level functionality")
                    .subcommand_required(true)
                    .arg_required_else_help(true)
                    .subcommands([
                        Command::new("api-server")
                            .about("Run HTTP + GraphQL server")
                            .subcommands([
                                Command::new("gql-query")
                                    .about("Executes the GraphQL query and prints out the result")
                                    .args([
                                        Arg::new("full")
                                            .long("full")
                                            .action(ArgAction::SetTrue)
                                            .help("Display the full result including extensions"),
                                        Arg::new("query").index(1).required(true),
                                    ]),
                                Command::new("gql-schema").about("Prints the GraphQL schema"),
                            ])
                            .args([
                                Arg::new("address")
                                    .long("address")
                                    .value_parser(value_parser!(std::net::IpAddr))
                                    .help("Expose HTTP server on specific network interface"),
                                Arg::new("http-port")
                                    .long("http-port")
                                    .value_parser(value_parser!(u16))
                                    .help("Expose HTTP server on specific port"),
                            ])
                            .after_help(indoc::indoc!(
                                "
                            ### Examples ###

                            Run API server on a specified port:

                                kamu system api-server --http-port 12312

                            Execute a single GraphQL query and print result to stdout:

                                kamu system api-server gql-query '{ apiVersion }'

                            Print out GraphQL API schema:

                                kamu system api-server gql-schema
                            "
                            )),
                        Command::new("ipfs")
                            .about("IPFS helpers")
                            .subcommand_required(true)
                            .subcommands([Command::new("add")
                                .about("Adds the specified dataset to IPFS and returns the CID")
                                .args([Arg::new("dataset")
                                    .index(1)
                                    .required(true)
                                    .value_parser(value_parse_dataset_ref_local)
                                    .help("Dataset reference")])]),
                    ]),
                tabular_output_params(
                    Command::new("tail")
                        .about("Displays a sample of most recent records in a dataset")
                        .args([
                            Arg::new("dataset")
                                .required(true)
                                .index(1)
                                .value_parser(value_parse_dataset_ref_local)
                                .help("Local dataset reference"),
                            Arg::new("num-records")
                                .long("num-records")
                                .short('n')
                                .value_parser(value_parser!(u64))
                                .default_value("10")
                                .value_name("NUM")
                                .help("Number of records to display"),
                        ])
                        .after_help(indoc::indoc!(
                            "
                    This command is simply a shortcut for:

                        kamu sql --engine datafusion --command 'SELECT * FROM \"{dataset}\" \
                        ORDER BY {offset_col} DESC LIMIT {num_records}'
                    "
                        )),
                ),
                Command::new("ui")
                    .about("Opens web interface")
                    .args([
                        Arg::new("address")
                            .long("address")
                            .value_parser(value_parser!(std::net::IpAddr))
                            .help("Expose HTTP server on specific network interface"),
                        Arg::new("http-port")
                            .long("http-port")
                            .value_parser(value_parser!(u16))
                            .help("Which port to run HTTP server on"),
                    ])
                    .after_help(indoc::indoc!(
                        "
                    Starts a built-in HTTP + GraphQL server and opens a pre-packaged \
                    Web UI application in your browser.

                    ### Examples ###

                    Starts server and opens UI in your default browser:

                        kamu ui

                    Start server on a specific port:

                        kamu ui --http-port 12345
                    "
                    )),
                Command::new("verify")
                    .about("Verifies the validity of a dataset")
                    .args(&[
                        Arg::new("recursive")
                            .short('r')
                            .long("recursive")
                            .action(ArgAction::SetTrue)
                            .help(
                                "Verify the entire transformation chain \
                            starting with root datasets",
                            ),
                        Arg::new("integrity")
                            .long("integrity")
                            .action(ArgAction::SetTrue)
                            .help(concat!(
                                "Check only the hashes of metadata ",
                                "and data without replaying transformations"
                            )),
                        Arg::new("dataset")
                            .action(ArgAction::Append)
                            .index(1)
                            .num_args(1..)
                            .required(true)
                            .value_parser(value_parse_dataset_ref_local)
                            .help("Local dataset reference(s)"),
                    ])
                    .after_help(indoc::indoc!(
                        "
                    Validity of derivative data is determined by:
                    - Trustworthiness of the source data that went into it
                    - Soundness of the derivative transformation chain that shaped it
                    - Guaranteeing that derivative data was in fact produced by \
                    declared transformations

                    For the first two, you can inspect the dataset lineage so see which root \
                    datasets the data is coming from and whether their publishers are credible. \
                    Then you can audit all derivative transformations to ensure they are sound \
                    and non-malicious.

                    This command can help you with the last stage. It uses the history of \
                    transformations stored in metadata to first compare the hashes of data \
                    with ones stored in metadata (i.e. verify that data corresponds to metadata). \
                    Then it repeats all declared transformations locally to ensure that what's \
                    declared in metadata actually produces the presented result.

                    The combination of the above steps can give you a high certainty that the \
                    data you're using is trustworthy.

                    When called on a root dataset the command will only perform the integrity \
                    check of comparing data hashes to metadata.

                    ### Examples ###

                    Verify the data in a dataset starting from its immediate inputs:

                        kamu verify com.example.deriv

                    Verify the entire transformation chain starting with root datasets \
                    (may download a lot of data):

                        kamu pull --recursive com.example.deriv

                    Verify only the hashes of metadata and data, without replaying the \
                    transformations. This is useful when you trust the peers performing \
                    transformations but want to ensure data was not tampered in storage \
                    or during the transmission:

                        kamu verify --integrity com.example.deriv
                    "
                    )),
            ],
        )
}

fn value_parse_dataset_name(s: &str) -> Result<DatasetName, String> {
    match DatasetName::try_from(s) {
        Ok(v) => Ok(v),
        Err(_) => Err(format!(
            "Dataset name can only contain alphanumerics, dashes, and dots, e.g. `my.dataset-id`",
        )),
    }
}

#[allow(dead_code)]
fn value_parse_dataset_remote_name(s: &str) -> Result<RemoteDatasetName, String> {
    match RemoteDatasetName::try_from(s) {
        Ok(v) => Ok(v),
        Err(_) => Err(format!(
            "Remote dataset name should be in form: \
            `repository/dataset-id` or `repository/account/dataset-id`",
        )),
    }
}

fn value_parse_dataset_ref_local(s: &str) -> Result<DatasetRefLocal, String> {
    match DatasetRefLocal::try_from(s) {
        Ok(v) => Ok(v),
        Err(_) => Err(format!(
            "Local reference should be in form: `did:odf:...` or `my.dataset.id`",
        )),
    }
}

fn value_parse_dataset_ref_remote(s: &str) -> Result<DatasetRefRemote, String> {
    match DatasetRefRemote::try_from(s) {
        Ok(v) => Ok(v),
        Err(_) => Err(format!(
            "Remote reference should be in form: \
            `did:odf:...` or `repository/account/dataset-id` or `scheme://some-url`",
        )),
    }
}

fn value_parse_dataset_ref_any(s: &str) -> Result<DatasetRefAny, String> {
    match DatasetRefAny::try_from(s) {
        Ok(v) => Ok(v),
        Err(_) => Err(format!(
            "Dataset reference should be in form: \
            `my.dataset.id` or `repository/account/dataset-id` or `did:odf:...` \
            or `scheme://some-url`",
        )),
    }
}

fn value_parse_repository_name(s: &str) -> Result<RepositoryName, String> {
    match RepositoryName::try_from(s) {
        Ok(v) => Ok(v),
        Err(_) => Err(format!(
            "RepositoryID can only contain alphanumerics, dashes, and dots",
        )),
    }
}

fn value_parse_multihash(s: &str) -> Result<Multihash, String> {
    match Multihash::try_from(s) {
        Ok(v) => Ok(v),
        Err(_) => Err(format!("Block hash must be a valid multihash string",)),
    }
}

fn validate_log_filter(s: &str) -> Result<String, String> {
    let items: Vec<_> = s.split(',').collect();
    for item in items {
        match item {
            "source" => Ok(()),
            "watermark" => Ok(()),
            "data" => Ok(()),
            _ => Err(format!(
                "Filter should be a comma-separated list of values like: source,data,watermark"
            )),
        }?;
    }
    Ok(s.to_string())
}
