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

#[derive(serde::Serialize, serde::Deserialize, Clone)]
pub struct CoreTestDesc {
    pub bin_path: std::path::PathBuf,
    pub extra: serde_json::Value,
}

impl CoreTestDesc {
    pub fn parse_extra<T: serde::de::DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
        serde_json::value::from_value(self.extra.clone())
    }

    pub fn into_extra<T: serde::de::DeserializeOwned>(self) -> Result<T, serde_json::Error> {
        serde_json::value::from_value(self.extra)
    }
}

#[cfg(feature = "groups")]
mod groups {
    #[derive(serde::Serialize, serde::Deserialize, Clone)]
    pub struct CoreGroupDesc {
        pub name: String,
        pub bin_path: std::path::PathBuf,
        pub extra: serde_json::Value,
    }

    impl CoreGroupDesc {
        pub fn parse_extra<T: serde::de::DeserializeOwned>(&self) -> Result<T, serde_json::Error> {
            serde_json::value::from_value(self.extra.clone())
        }

        pub fn into_extra<T: serde::de::DeserializeOwned>(self) -> Result<T, serde_json::Error> {
            serde_json::value::from_value(self.extra)
        }
    }
}

#[cfg(feature = "groups")]
pub use groups::CoreGroupDesc;

pub const CARGO_DIFFTESTS_VERSION_FILENAME: &str = "cargo_difftests_version";
pub const CARGO_DIFFTESTS_SELF_JSON_FILENAME: &str = "self.json";
pub const CARGO_DIFFTESTS_SELF_PROFILE_FILENAME: &str = "self.profraw";
pub const CARGO_DIFFTESTS_OTHER_PROFILE_FILENAME_TEMPLATE: &str = "%m_%p.profraw";
pub const CARGO_DIFFTESTS_GROUP_SELF_JSON_FILENAME: &str = "group_self.json";
pub const CARGO_DIFFTESTS_GROUP_FIRST_TEST_RUN: &str = "group_first_test_run";