// Copyright Kamu Data, Inc. and contributors. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

use crate::serde::flatbuffers::FlatbuffersEngineProtocol;
use crate::serde::{EngineProtocolDeserializer, EngineProtocolSerializer};
use crate::{
    ExecuteQueryRequest, ExecuteQueryResponse, ExecuteQueryResponseInternalError,
    ExecuteQueryResponseInvalidQuery, ExecuteQueryResponseSuccess,
};

use super::grpc_generated::engine_client::EngineClient as EngineClientGRPC;
use super::grpc_generated::ExecuteQueryRequest as ExecuteQueryRequestGRPC;

use thiserror::Error;

pub struct EngineGrpcClient {
    client: EngineClientGRPC<tonic::transport::Channel>,
}

impl EngineGrpcClient {
    pub async fn connect(host: &str, port: u16) -> Result<Self, tonic::transport::Error> {
        let client = EngineClientGRPC::connect(format!("http://{}:{}", host, port)).await?;

        Ok(Self { client })
    }

    pub async fn execute_query(
        &mut self,
        request: ExecuteQueryRequest,
    ) -> Result<ExecuteQueryResponseSuccess, ExecuteQueryError> {
        let fb = FlatbuffersEngineProtocol
            .write_execute_query_request(&request)
            .unwrap();

        let request_grpc = tonic::Request::new(ExecuteQueryRequestGRPC {
            flatbuffer: fb.collapse_vec(),
        });

        let mut stream = self.client.execute_query(request_grpc).await?.into_inner();

        while let Some(response_grpc) = stream.message().await? {
            let response = FlatbuffersEngineProtocol
                .read_execute_query_response(&response_grpc.flatbuffer)
                .unwrap();

            match response {
                ExecuteQueryResponse::Progress => (),
                ExecuteQueryResponse::Success(success) => return Ok(success),
                ExecuteQueryResponse::InvalidQuery(error) => return Err(error.into()),
                ExecuteQueryResponse::InternalError(error) => return Err(error.into()),
            }
        }

        unreachable!("Engine did not transmit Success or Error message in the response stream")
    }
}

///////////////////////////////////////////////////////////////////////////////

#[derive(Debug, Error)]
pub enum ExecuteQueryError {
    #[error("Invalid query: {0}")]
    InvalidQuery(ExecuteQueryResponseInvalidQuery),
    #[error("Engine internal error: {0}")]
    EngineInternalError(ExecuteQueryResponseInternalError),
    #[error("Rpc error: {0}")]
    RpcError(#[from] tonic::Status),
}

impl From<ExecuteQueryResponseInvalidQuery> for ExecuteQueryError {
    fn from(error: ExecuteQueryResponseInvalidQuery) -> Self {
        Self::InvalidQuery(error)
    }
}

impl From<ExecuteQueryResponseInternalError> for ExecuteQueryError {
    fn from(error: ExecuteQueryResponseInternalError) -> Self {
        Self::EngineInternalError(error)
    }
}
