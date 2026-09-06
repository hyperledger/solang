// SPDX-License-Identifier: Apache-2.0

//! This module defines the json format for `solang compile --standard-json`.

use crate::abi::ethereum::ABI;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

#[derive(Serialize)]
pub struct EwasmContract {
    pub wasm: String,
}

#[derive(Serialize)]
pub struct JsonContract {
    pub abi: Vec<ABI>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ewasm: Option<EwasmContract>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_space: Option<u32>,
}

#[derive(Serialize)]
pub struct JsonResult {
    pub errors: Vec<OutputJson>,
    pub target: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    pub program: String,
    pub contracts: HashMap<String, HashMap<String, JsonContract>>,
}

#[derive(Deserialize)]
pub struct StandardJsonInput {
    #[serde(default)]
    pub language: Option<String>,
    #[serde(default)]
    pub sources: BTreeMap<String, StandardJsonSource>,
    #[serde(default)]
    pub settings: StandardJsonSettings,
}

#[derive(Deserialize)]
pub struct StandardJsonSource {
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub urls: Vec<String>,
}

#[derive(Default, Deserialize)]
pub struct StandardJsonSettings {
    #[serde(default)]
    pub remappings: Vec<String>,
}

#[derive(Serialize)]
pub struct LocJson {
    pub file: String,
    pub start: usize,
    pub end: usize,
}

#[derive(Serialize)]
#[allow(non_snake_case)]
pub struct OutputJson {
    pub sourceLocation: Option<LocJson>,
    #[serde(rename = "type")]
    pub ty: String,
    pub component: String,
    pub severity: String,
    pub message: String,
    pub formattedMessage: String,
}
