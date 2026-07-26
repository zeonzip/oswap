use crate::os::PlatformInterface;
use crate::os::Platform;

impl_interface!(
    fn a_function() {
        println!("im on unix")
    }
);