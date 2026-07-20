use std::collections::HashMap;
use std::sync::Mutex;
use std::sync::OnceLock;
use viewkit::svg::SvgData;

include!(concat!(env!("OUT_DIR"), "/icons.rs"));

pub fn icon(name: &str) -> Option<SvgData> {
    static CACHE: OnceLock<Mutex<HashMap<String, SvgData>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));

    if let Some(icon) = cache.lock().expect("icon cache is poisoned").get(name) {
        return Some(icon.clone());
    }

    let icon = generated_icon_bytes(name)
        .map(|bytes| SvgData::decode(bytes).expect("failed to decode icon"))?;
    cache
        .lock()
        .expect("icon cache is poisoned")
        .insert(name.to_owned(), icon.clone());
    Some(icon)
}
