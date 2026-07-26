use oswap::{define_interface, define_platforms};

define_interface! { Platform, PlatformInterface, impl_interface, 
    pub fn a_function();
}

define_platforms!(
    unix,
    windows
);