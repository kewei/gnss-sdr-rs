use soapysdr::Args;
use std::collections::HashMap;


// Convert a HashMap to Args
pub fn hashmap_to_args(map: HashMap<String, String>) -> Result<Args, String> {
    let args_str = map.iter().map(|(k,v)| format!("{}={}", k, v)).collect::<Vec<_>>().join(",");
    Ok(Args::from(args_str.as_str()))
}