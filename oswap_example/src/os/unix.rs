use crate::os::PlatformInterface;
use crate::os::Platform;

impl_interface!(
    fn a_function(some: &str) {
        println!("This is running on unix, and I want to tell you: {}!", some)
    }
);