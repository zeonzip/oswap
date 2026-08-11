use oswap::{define_interface, define_platforms};

define_interface! { pub Platform, PlatformInterface, impl_interface,
    pub fn a_function(some: &str);
}

define_platforms![
    unix,
    windows
];