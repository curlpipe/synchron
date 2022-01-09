// util.rs - common utilities for helping out

// Utility macro for easy dbus property addition
#[macro_export]
macro_rules! add_prop {
    ($props:expr, $prop:expr, $value:expr) => {
        $props.insert($prop.to_string(), Variant(Box::new($value)));
    };
}

pub fn expand_path(path: &str) -> Option<String> {
    // Utility function for expanding paths
    let with_user = expanduser::expanduser(path).ok()?;
    let full_path = std::fs::canonicalize(with_user).ok()?;
    full_path.into_os_string().into_string().ok()
}
