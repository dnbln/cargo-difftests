/*
 *        Copyright (c) 2023-2024 Dinu Blanovschi
 *
 *    Licensed under the Apache License, Version 2.0 (the "License");
 *    you may not use this file except in compliance with the License.
 *    You may obtain a copy of the License at
 *
 *        https://www.apache.org/licenses/LICENSE-2.0
 *
 *    Unless required by applicable law or agreed to in writing, software
 *    distributed under the License is distributed on an "AS IS" BASIS,
 *    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 *    See the License for the specific language governing permissions and
 *    limitations under the License.
 */

#[derive(serde::Serialize, serde::Deserialize, Clone, Debug)]
#[repr(transparent)]
pub struct CoreTestDesc(serde_json::Value);

impl CoreTestDesc {
    pub fn parse_extra<T: serde::de::DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::value::from_value(self.0.clone())
    }

    pub fn into_extra<T: serde::de::DeserializeOwned>(self) -> Result<T, serde_json::Error> {
        serde_json::value::from_value(self.0)
    }
}

pub const CARGO_DIFFTESTS_VERSION_FILENAME: &str = "cargo_difftests_version";
pub const CARGO_DIFFTESTS_SELF_JSON_FILENAME: &str = "self.json";
pub const CARGO_DIFFTESTS_TEST_BINARY_FILENAME: &str = "test_binary";
pub const CARGO_DIFFTESTS_TEST_NAME_FILENAME: &str = "test_name";
