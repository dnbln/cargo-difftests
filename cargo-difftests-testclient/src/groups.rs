use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Eq, serde::Serialize)]
#[serde(into = "String")]
pub struct GroupName(pub std::borrow::Cow<'static, str>);

impl From<&'static str> for GroupName {
    fn from(name: &'static str) -> Self {
        Self(name.into())
    }
}

impl From<String> for GroupName {
    fn from(name: String) -> Self {
        Self(name.into())
    }
}

impl From<GroupName> for String {
    fn from(name: GroupName) -> Self {
        name.0.into_owned()
    }
}

impl GroupName {
    pub fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

impl PartialEq for GroupName {
    fn eq(&self, other: &Self) -> bool {
        self.as_str() == other.as_str()
    }
}

impl std::hash::Hash for GroupName {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        <str as std::hash::Hash>::hash(self.as_str(), state)
    }
}

#[derive(Debug, Clone)]
pub struct GroupBasicInfo {
    pub name: GroupName,
    pub bin_path: PathBuf,
    pub temp_dir: PathBuf,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct GroupMeta<T: serde::Serialize> {
    pub bin_path: PathBuf,
    pub name: GroupName,
    #[serde(skip)]
    pub temp_dir: PathBuf,
    pub extra: T,
}

impl<T: serde::Serialize> GroupMeta<T> {
    fn basic_info(&self) -> GroupBasicInfo {
        GroupBasicInfo {
            name: self.name.clone(),
            bin_path: self.bin_path.clone(),
            temp_dir: self.temp_dir.clone(),
        }
    }
}

fn init_group_dir<T: serde::Serialize>(meta: GroupMeta<T>) -> std::io::Result<()> {
    let clean = !std::env::var("CARGO_DIFFTESTS_GROUP_NO_CLEAN").is_ok();

    let new = if meta.temp_dir.exists() {
        if clean {
            std::fs::remove_dir_all(&meta.temp_dir)?;
            true
        } else {
            false
        }
    } else {
        true
    };

    std::fs::create_dir_all(&meta.temp_dir)?;

    if new {
        std::fs::write(
            meta.temp_dir
                .join(cargo_difftests_core::CARGO_DIFFTESTS_GROUP_FIRST_TEST_RUN),
            "",
        )?;
    }

    let meta_str = serde_json::to_string(&meta).unwrap();
    std::fs::write(
        meta.temp_dir
            .join(cargo_difftests_core::CARGO_DIFFTESTS_GROUP_SELF_JSON_FILENAME),
        meta_str,
    )?;

    std::fs::write(
        meta.temp_dir
            .join(cargo_difftests_core::CARGO_DIFFTESTS_VERSION_FILENAME),
        env!("CARGO_PKG_VERSION"),
    )?;

    Ok(())
}

fn group_descriptions_lock() -> std::sync::MutexGuard<'static, HashMap<GroupName, GroupBasicInfo>> {
    use std::sync::{Mutex, OnceLock};
    static LOCK: OnceLock<Mutex<HashMap<GroupName, GroupBasicInfo>>> = OnceLock::new();
    let lock = LOCK.get_or_init(|| Mutex::new(HashMap::new()));
    lock.lock().unwrap()
}

#[cfg(feature = "parallel-groups")]
pub(crate) enum State {
    None,
    Running {
        group_name: Option<GroupName>,
        running_test_count: usize,
    },
}

#[cfg(feature = "parallel-groups")]
fn crs_condvar() -> &'static std::sync::Condvar {
    static CURRENTLY_RUNNING_STATE_CONDVAR: std::sync::OnceLock<std::sync::Condvar> =
        std::sync::OnceLock::new();
    CURRENTLY_RUNNING_STATE_CONDVAR.get_or_init(|| std::sync::Condvar::new())
}

#[cfg(feature = "parallel-groups")]
fn currently_running_state() -> std::sync::MutexGuard<'static, State> {
    static CURRENTLY_RUNNING_STATE_LOCK: std::sync::OnceLock<std::sync::Mutex<State>> =
        std::sync::OnceLock::new();
    let lock = CURRENTLY_RUNNING_STATE_LOCK.get_or_init(|| std::sync::Mutex::new(State::None));
    lock.lock().unwrap()
}

pub(crate) struct GroupDifftestsEnv {
    #[cfg(not(feature = "parallel-groups"))]
    _t_lock: std::sync::MutexGuard<'static, ()>,

    #[cfg(feature = "parallel-groups")]
    self_llvm_profile_path: PathBuf,
}

#[cfg(feature = "parallel-groups")]
impl Drop for GroupDifftestsEnv {
    fn drop(&mut self) {
        let mut _l = wr_test_group_dec();
        match &mut *_l {
            State::None => unreachable!(),
            State::Running {
                running_test_count, ..
            } => {
                if *running_test_count == 0 {
                    super::SelfProfileWriter::do_write_to_file(&self.self_llvm_profile_path);

                    *_l = State::None;
                    drop(_l);
                    crs_notify();
                }
            }
        }
    }
}

pub fn init_group<T: serde::Serialize>(
    name: GroupName,
    group_meta_resolver: fn(GroupName) -> GroupMeta<T>,
) -> std::io::Result<super::DifftestsEnv> {
    let mut group_descriptions = group_descriptions_lock();
    let meta = match group_descriptions.entry(name.clone()) {
        Entry::Occupied(entry) => entry.get().clone(),
        Entry::Vacant(entry) => {
            let meta = group_meta_resolver(name.clone());

            debug_assert_eq!(meta.name, name);

            entry.insert(meta.basic_info());

            let basic = meta.basic_info();

            init_group_dir(meta)?;

            basic
        }
    };

    #[cfg(not(feature = "parallel-groups"))]
    let _t_lock = super::test_lock();

    #[cfg(feature = "parallel-groups")]
    wr_test_group_inc(Some(name.clone()));

    Ok(super::DifftestsEnv {
        llvm_profile_file_name: "LLVM_PROFILE_FILE".into(),
        llvm_profile_file_value: meta
            .temp_dir
            .join(cargo_difftests_core::CARGO_DIFFTESTS_OTHER_PROFILE_FILENAME_TEMPLATE)
            .into_os_string(),
        difftests_env_inner: super::DifftestsEnvInner::Group(GroupDifftestsEnv {
            #[cfg(not(feature = "parallel-groups"))]
            _t_lock,
            #[cfg(feature = "parallel-groups")]
            self_llvm_profile_path: meta
                .temp_dir
                .join(cargo_difftests_core::CARGO_DIFFTESTS_SELF_PROFILE_FILENAME),
        }),
    })
}

#[cfg(feature = "parallel-groups")]
pub(crate) fn wr_test_group_inc(group_name: Option<GroupName>) {
    let crs_condvar = crs_condvar();

    let crs = currently_running_state();

    let _l = match group_name {
        Some(group_name) => {
            let mut crs_lock = crs_condvar
                .wait_while(crs, |crs| match crs {
                    State::None => false,
                    State::Running {
                        group_name: crg, ..
                    } => crg.as_ref() != Some(&group_name),
                })
                .unwrap();

            match &mut *crs_lock {
                State::None => {
                    *crs_lock = State::Running {
                        group_name: Some(group_name),
                        running_test_count: 1,
                    };
                }
                State::Running {
                    running_test_count, ..
                } => {
                    *running_test_count += 1;
                }
            }
        }
        None => {
            let mut crs_lock = crs_condvar
                .wait_while(crs, |crs| match crs {
                    State::None => false,
                    State::Running { .. } => true,
                })
                .unwrap();

            *crs_lock = State::Running {
                group_name: None,
                running_test_count: 1,
            };
        }
    };
}

#[cfg(feature = "parallel-groups")]
pub(crate) fn wr_test_group_dec() -> std::sync::MutexGuard<'static, State> {
    let mut crs = currently_running_state();

    match &mut *crs {
        State::None => unreachable!(),
        State::Running {
            running_test_count, ..
        } => {
            *running_test_count -= 1;
        }
    }

    crs
}

#[cfg(feature = "parallel-groups")]
pub(crate) fn crs_notify() {
    let crs_condvar = crs_condvar();
    crs_condvar.notify_all();
}
