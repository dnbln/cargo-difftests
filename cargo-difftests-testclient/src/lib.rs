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

pub fn write_desc<T: serde::Serialize>(desc: T) -> std::io::Result<()> {
    let tmpdir = std::env::var_os("CARGO_DIFFTEST_DIR")
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::Other, "no temp dir"))?;
    let dir = std::path::Path::new(&tmpdir);

    let s = serde_json::to_string(&desc)?;

    std::fs::write(
        dir.join(cargo_difftests_core::CARGO_DIFFTESTS_SELF_JSON_FILENAME),
        s,
    )?;

    Ok(())
}
