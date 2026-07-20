use std::sync::OnceLock;
use viewkit::svg::SvgData;

include!(concat!(env!("OUT_DIR"), "/icons.rs"));

pub fn icon(name: &str) -> Option<SvgData> {
    generated_icon_bytes(name).map(|bytes| SvgData::decode(bytes).expect("failed to decode icon"))
}
