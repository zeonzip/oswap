## oswap

> NOTE: For proper documentation consult the [documentation page](https://docs.rs/oswap).

``oswap`` is a procedural macro crate designed to automatically setup cross-platform interfacing code making your codebase not need to juggle around weird platform config code and instead be able to directly call functions which are universal across your codebase, backed by certain platform specific implementations. An example of usage of this crate would be to for example print what platform the current device is, along with a little message:

```rust
use oswap::{define_interface, define_platforms};

define_interface! { pub Platform, PlatformInterface, impl_interface,
    pub fn a_function(some: &str);
}

define_platforms![
    unix,
    windows,
    { path: android, cfg: target_os = "android" }
];
```

Then in the files unix.rs, windows.rs and android.rs you can then implement the custom interface. You can do this for example like this:

```rust
use crate::os::PlatformInterface;
use crate::os::Platform;

impl_interface!(
    fn a_function(some: &str) {
        println!("This is running on unix, and I want to tell you: {}!", some)
    }
);
```

And any other files in the crate can now call this cross-platform function like this:

```rust
use crate::os::{a_function};

pub mod os;

fn main() {
    a_function("a");
}
```

Super simple and can handle all your cross platform needs, if you like the crate please leave a star ⭐❤️ on the [GitHub](https://github.com/zeonzip/oswap)