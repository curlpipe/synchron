// util.rs - common utilities for helping out

// Utility macro for easy dbus property addition
#[macro_export]
macro_rules! add_prop {
    ($props:expr, $prop:expr, $value:expr) => {
        $props.insert($prop.to_string(), Variant(Box::new($value)));
    };
}
