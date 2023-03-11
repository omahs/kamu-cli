// Copyright Kamu Data, Inc. and contributors. All rights reserved.
//
// Use of this software is governed by the Business Source License
// included in the LICENSE file.
//
// As of the Change Date specified in that file, in accordance with
// the Business Source License, use of this software will be governed
// by the Apache License, Version 2.0.

// @generated
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ExecuteQueryRequest {
    #[prost(bytes = "vec", tag = "1")]
    pub flatbuffer: ::prost::alloc::vec::Vec<u8>,
}
#[derive(Clone, PartialEq, ::prost::Message)]
pub struct ExecuteQueryResponse {
    #[prost(bytes = "vec", tag = "1")]
    pub flatbuffer: ::prost::alloc::vec::Vec<u8>,
}
include!("engine.tonic.rs");
// @@protoc_insertion_point(module)
