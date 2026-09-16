//! Fallback for platforms without a clipboard backend yet (Linux): everything is empty.
use crate::convert::Flavor;
pub fn types() -> Vec<String> { vec![] }
pub fn has(_f: Flavor) -> bool { false }
pub fn read(_f: Flavor) -> Option<Vec<u8>> { None }
pub fn write(_items: &[(Flavor, Vec<u8>)], _keep_others: bool) -> Result<(), String> {
    Err("clipboard access is not implemented on this platform yet".into())
}
pub fn rtf_to_html(_rtf: &[u8]) -> Option<String> { None }
pub fn html_to_rtf(_html: &str) -> Option<Vec<u8>> { None }
