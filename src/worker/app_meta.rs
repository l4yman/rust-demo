use std::{
    borrow::Borrow,
    collections::{BTreeMap, HashMap},
    fs,
    path::Path,
};

use serde::{Deserialize, Serialize};

use crate::{
    general::kv_interface::KvOps,
    result::{ErrCvt, WSResult},
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FnEventYaml {
    HttpApp { http_app: () },
    KvSet { kv_set: usize },
}

#[derive(PartialEq, Eq)]
pub enum FnEvent {
    HttpApp,
    KvSet(usize),
}

impl From<FnEventYaml> for FnEvent {
    fn from(yaml: FnEventYaml) -> Self {
        match yaml {
            FnEventYaml::HttpApp { http_app: _ } => Self::HttpApp,
            FnEventYaml::KvSet { kv_set } => Self::KvSet(kv_set),
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FnArgYaml {
    KvKey { kv_key: usize },
    HttpText { http_text: () },
}

pub enum FnArg {
    KvKey(usize),
    HttpText,
}

impl From<FnArgYaml> for FnArg {
    fn from(yaml: FnArgYaml) -> Self {
        match yaml {
            FnArgYaml::KvKey { kv_key } => Self::KvKey(kv_key),
            FnArgYaml::HttpText { http_text: _ } => Self::HttpText,
        }
    }
}

// pub enum FnArgPayload {
//     Vec(Vec<u8>),
// }

// pub fn arg_payloads_into_wasm_params(arg_payloads: Vec<FnArgPayload>, vm: &Vm) -> Vec<WasmValue> {
//     fn prepare_vec_in_vm(vm: &Vm, v: &[u8]) -> (i32, i32) {
//         let ptr = vm
//             .run_func(
//                 Some(&vm.instance_names()[0]),
//                 "allocate",
//                 vec![WasmValue::from_i32(v.len() as i32)],
//             )
//             .unwrap()[0]
//             .to_i32();
//         let mut mem = ManuallyDrop::new(unsafe {
//             Vec::from_raw_parts(
//                 vm.named_module(&vm.instance_names()[0])
//                     .unwrap()
//                     .memory("memory")
//                     .unwrap()
//                     .data_pointer_mut(ptr as u32, v.len() as u32)
//                     .unwrap(),
//                 v.len(),
//                 v.len(),
//             )
//         });
//         mem.copy_from_slice(v);
//         (ptr, v.len() as i32)
//     }
//     let mut params = vec![];
//     for arg in arg_payloads {
//         match arg {
//             FnArgPayload::KvKey(key) => {
//                 let (ptr, len) = prepare_vec_in_vm(&vm, &key);
//                 params.push(WasmValue::from_i32(ptr));
//                 params.push(WasmValue::from_i32(len));
//             }
//             FnArgPayload::HttpText(text) => {
//                 if text.len() > 0 {
//                     let (ptr, len) = prepare_vec_in_vm(&vm, &text);
//                     params.push(WasmValue::from_i32(ptr));
//                     params.push(WasmValue::from_i32(len));
//                 }
//             }
//         }
//     }

//     params
// }

#[derive(Debug, Serialize, Deserialize)]
pub struct FnMetaYaml {
    pub event: Vec<FnEventYaml>,
    // pub input: Option<Vec<FnInputYaml>>,
    pub args: Vec<FnArgYaml>,
    /// key to operations
    pub kvs: Option<BTreeMap<String, Vec<String>>>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct KeyPattern(pub String);

#[derive(Debug)]
pub struct KvMeta {
    set: bool,
    get: bool,
    delete: bool,
    pub pattern: KeyPattern,
}

pub struct FnMeta {
    pub event: Vec<FnEvent>,
    pub args: Vec<FnArg>,
    pub kvs: Option<Vec<KvMeta>>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppMetaYaml {
    pub fns: HashMap<String, FnMetaYaml>,
}

pub struct AppMeta {
    fns: HashMap<String, FnMeta>,
}

pub struct AppMetaManager {
    app_metas: HashMap<String, AppMeta>,
    pattern_2_app_fn: HashMap<String, Vec<(String, String)>>,
}

// impl FnEvent {
//     pub fn match_kv_ope(&self, ope: KvOps) -> bool {
//         match self {
//             Self::KvSet(_) => ope == KvOps::Set,
//             Self::HttpApp => false,
//         }
//     }
// }

impl AppMetaYaml {
    pub fn read(apps_dir: impl AsRef<Path>, appname: &str) -> AppMetaYaml {
        let file_path = apps_dir.as_ref().join(format!("{}/app.yaml", appname));
        let file = std::fs::File::open(file_path).unwrap_or_else(|err| {
            panic!("open config file failed, err: {:?}", err);
        });
        serde_yaml::from_reader(file).unwrap_or_else(|e| {
            panic!("parse yaml config file failed, err: {:?}", e);
        })
    }
    // // return true if key set is valid
    // pub fn check_key_set(&self, key: &str) -> bool {
    //     self.fns
    //         .iter()
    //         .any(|(_, fn_meta)| {
    //             if let Some(kvs)=&fn_meta.kvs{
    //                 kvs.iter().any(|(k, _)| key.contains(k))
    //             }else{
    //                 false
    //             })
    // }
}

impl FnMeta {
    pub fn match_key(&self, key: &[u8], ope: KvOps) -> Option<KeyPattern> {
        let key = if let Ok(key) = std::str::from_utf8(key) {
            key
        } else {
            return None;
        };
        if let Some(kvs) = &self.kvs {
            for kv in kvs {
                if kv.pattern.match_key(key) {
                    match ope {
                        KvOps::Get => {
                            if kv.get {
                                return Some(kv.pattern.clone());
                            }
                        }
                        KvOps::Set => {
                            if kv.set {
                                return Some(kv.pattern.clone());
                            }
                        }
                        KvOps::Delete => {
                            if kv.delete {
                                return Some(kv.pattern.clone());
                            }
                        }
                    }
                    tracing::info!("allow ope {:?}, cur ope:{:?}", kv, ope);
                }
            }
            // tracing::info!("no key pattern matched for key: {}", key);
        }

        None
    }

    pub fn try_get_kv_meta_by_index(&self, index: usize) -> Option<&KvMeta> {
        if let Some(kvs) = &self.kvs {
            return kvs.get(index);
        }
        None
    }

    /// index should be valid
    fn get_kv_meta_by_index_unwrap(&self, index: usize) -> &KvMeta {
        self.try_get_kv_meta_by_index(index).unwrap()
    }
    // /// get event related kvmeta matches operation
    // pub fn get_event_kv(&self, ope: KvOps, event: &FnEvent) -> Option<&KvMeta> {
    //     match event {
    //         FnEvent::KvSet(kv_set) => {
    //             if ope == KvOps::Set {
    //                 return Some(self.get_kv_meta_by_index_unwrap(*kv_set));
    //             }
    //         }
    //         FnEvent::HttpApp => {}
    //     }
    //     None
    // }

    /// find kv event trigger with match the `pattern` and `ope`
    pub fn find_will_trigger_kv_event(&self, pattern: &KeyPattern, ope: KvOps) -> Option<&KvMeta> {
        self.event.iter().find_map(|event| {
            match event {
                FnEvent::HttpApp => {}
                FnEvent::KvSet(key_index) => {
                    if ope == KvOps::Set {
                        let res = self.get_kv_meta_by_index_unwrap(*key_index);
                        if res.pattern == *pattern {
                            return Some(res);
                        }
                    }
                }
            }
            None
        })
    }
}

impl KeyPattern {
    pub fn new(input: String) -> Self {
        Self(input)
    }
    // match {} for any words
    // "xxxx_{}_{}" matches "xxxx_abc_123"
    // “xxxx{}{}" matches "xxxxabc123"
    pub fn match_key(&self, key: &str) -> bool {
        let re = self.0.replace("{}", "[a-zA-Z0-9]+");
        // let pattern_len = re.len();
        // tracing::info!("len:{}", re.len());
        let re = regex::Regex::new(&re).unwrap();
        if let Some(len) = re.find(key) {
            tracing::info!(
                "match key: {} with pattern: {} with len {} {} ",
                key,
                self.0,
                len.len(),
                key.len()
            );
            len.len() == key.len()
        } else {
            tracing::info!("not match key: {} with pattern: {}", key, self.0);
            false
        }
    }
    // pub fn matcher(&self) -> String {

    //     // let re = Regex::new(r"(.+)\{\}").unwrap();

    //     // if let Some(captured) = re.captures(&*self.0) {
    //     //     if let Some(capture_group) = captured.get(1) {
    //     //         let result = capture_group.as_str();
    //     //         // println!("Result: {}", result);
    //     //         return result.to_owned();
    //     //     }
    //     // }

    //     // self.0.clone()
    // }
}

impl From<FnMetaYaml> for FnMeta {
    fn from(yaml: FnMetaYaml) -> Self {
        let kvs = if let Some(kvs) = yaml.kvs {
            Some(
                kvs.into_iter()
                    .map(|(key, ops)| {
                        let mut set = false;
                        let mut get = false;
                        let mut delete = false;
                        for op in ops {
                            if op == "set" {
                                set = true;
                            } else if op == "get" {
                                get = true;
                            } else if op == "delete" {
                                delete = true;
                            } else {
                                panic!("invalid operation: {}", op);
                            }
                        }
                        // TODO: check key pattern
                        KvMeta {
                            delete,
                            set,
                            get,
                            pattern: KeyPattern::new(key),
                        }
                    })
                    .collect(),
            )
        } else {
            None
        };
        let res = Self {
            event: yaml.event.into_iter().map(|e| e.into()).collect(),
            args: yaml.args.into_iter().map(|a| a.into()).collect(),
            kvs,
        };
        // assert!(res.check_kv_valid());
        res
    }
}

impl From<AppMetaYaml> for AppMeta {
    fn from(yaml: AppMetaYaml) -> Self {
        let fns = yaml
            .fns
            .into_iter()
            .map(|(fnname, fnmeta)| (fnname, fnmeta.into()))
            .collect();
        Self { fns }
    }
}

impl AppMeta {
    pub fn get_fn_meta(&self, fnname: &str) -> Option<&FnMeta> {
        self.fns.get(fnname)
    }
    pub fn http_trigger_fn(&self) -> Option<&str> {
        self.fns.iter().find_map(|(fnname, fnmeta)| {
            if fnmeta.event.iter().any(|e| e == &FnEvent::HttpApp) {
                Some(fnname.as_str())
            } else {
                None
            }
        })
    }
}

impl AppMetaManager {
    pub fn new() -> Self {
        Self {
            app_metas: HashMap::new(),
            pattern_2_app_fn: HashMap::new(),
        }
    }
    pub fn get_app_meta(&self, app: &str) -> Option<&AppMeta> {
        self.app_metas.get(app)
    }
    pub fn get_pattern_triggers(
        &self,
        pattern: impl Borrow<str>,
    ) -> Option<&Vec<(String, String)>> {
        self.pattern_2_app_fn.get(pattern.borrow())
    }
    pub async fn load_all_app_meta(&mut self, file_dir: impl AsRef<Path>) -> WSResult<()> {
        let entries =
            fs::read_dir(file_dir.as_ref().join("apps")).map_err(|e| ErrCvt(e).to_ws_io_err())?;

        // 遍历文件夹中的每个条目
        for entry in entries {
            // 获取目录项的 Result<DirEntry, io::Error>
            let entry = entry.map_err(|e| ErrCvt(e).to_ws_io_err())?;
            // 获取目录项的文件名
            let file_name = entry.file_name();
            // dir name is the app name
            let app_name = file_name.to_str().unwrap().to_owned();
            assert!(entry.file_type().unwrap().is_dir());

            // read app config yaml
            let res = {
                let apps_dir = file_dir.as_ref().join("apps");
                let file_name_str = app_name.clone();
                tokio::task::spawn_blocking(move || AppMetaYaml::read(apps_dir, &*file_name_str))
                    .await
                    .unwrap()
            };

            // transform
            let meta: AppMeta = res.into();

            // build and checks
            // - build up key pattern to app fn
            for (fnname, fnmeta) in &meta.fns {
                for event in &fnmeta.event {
                    match event {
                        // not kv event, no key pattern
                        FnEvent::HttpApp => {}
                        FnEvent::KvSet(key_index) => {
                            let kvmeta = fnmeta.try_get_kv_meta_by_index(*key_index).unwrap();
                            self.pattern_2_app_fn
                                .entry(kvmeta.pattern.0.clone())
                                .or_insert_with(Vec::new)
                                .push((app_name.clone(), fnname.clone()));
                        }
                    }
                }
            }
            let _ = self.app_metas.insert(app_name, meta);
        }
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use crate::util;

    use super::*;
    #[test]
    fn test_key_pattern() {
        util::test_tracing_start();
        let pattern = KeyPattern::new("xxxx_{}_{}".to_owned());
        assert!(pattern.match_key("xxxx_abc_123"));
    }
}