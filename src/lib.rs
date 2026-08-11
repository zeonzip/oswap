use proc_macro::{TokenStream};
use quote::quote;
use syn::{braced, parse_macro_input, token, FnArg, Ident, Item, LitStr, Meta, Token, TraitItemFn, Visibility};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;

struct VisibilityTraitFn {
    vis: Visibility,
    method: TraitItemFn,
}

impl Parse for VisibilityTraitFn {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(VisibilityTraitFn {
            vis: input.parse()?,     // Inherited if there's no `pub`
            method: input.parse()?,  // fn some_trait_thing(...) ; or { ... }
        })
    }
}


struct Input {
    platform_vis: Visibility,
    platform: Ident,
    interface: Ident,
    impl_macro: Ident,
    methods: Vec<VisibilityTraitFn>,
}

impl Parse for Input {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let platform_vis = input.parse()?;
        let platform: Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        let interface: Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        let impl_macro: Ident = input.parse()?;
        input.parse::<Token![,]>()?;

        // remaining tokens are the methods; parse until the stream is empty
        let mut methods = Vec::new();
        while !input.is_empty() {
            methods.push(input.parse()?);
        }

        Ok(Input { platform_vis, platform, interface, impl_macro, methods })
    }
}

/// ## The `define_interface` macro:
///
/// This macro defines a struct and a trait which will handle defining the function interface which each
/// platform implementation will utilize; in simpler words this macro just defines an interface which each platform makes specific code for.
///
/// The usage of the macro is the following:
/// ```
/// define_interface! { Platform, PlatformInterface, impl_interface,
///     pub fn a_function(example_arg: &str);
/// }
/// ```
///
/// As we see it takes three arguments first which is first the struct name which will be used for the empty
/// marker struct which each platform internally implements the interface for. A safe default is just calling
/// it `Platform` and not including any visibility for the struct (though the crate supports adding a visibility
/// indicator for the struct and trait) (trait explained later) with for example `pub Platform` or `pub(crate) Platform`. This means that
/// it essentially just defines where the types are accessible, and therefore where the trait can be implemented for the struct
/// (which we will come back to later in the impl_interface section).
///
/// The second argument is the name of the interface trait. This trait will contain all the function signature definitions
/// which will be the cross-platform universal interface functions. This trait is what each platform will implement for the common struct,
/// and have their own unique platform-specific implementation for.
///
/// For the rest of this documentation we will just refer to the struct name and the trait interface with their respective default names.
///
/// The third argument is the name of the implementation macro. The implementation macro is what the different platform
/// modules use to implement their platform specific code. The specific usage of this macro is detailed in the next paragraph.
/// **IMPORTANT: If you define the publicity indicator of Platform to be pub and not pub(crate) or otherwise, the implementation macro will be marked with ``#[macro_export]``, and it
/// will not just be private within your own crate.**
///
/// After the arguments have been filled out, the next input this macro takes is the function interface itself.
/// It takes any amount of trait style functions (meaning yes it supports default implementations for functions) and an optional
/// visibility indicator (e.g `pub(crate)`) before the function itself. This is of course not carried into the trait definition
/// itself since trait functions cannot have visibility indicators. The visibility indicator is used to inform the macro
/// what visibility the re-exported universal function should have. All functions defined in the interface is re-exported in the
/// current module in the following way:
///
/// ```ignore
/// <optional-visibility> fn function_name(some_arg: &str, another_example_arg: u32) -> SomeReturnType {
///     <Platform as PlatformInterface>::function_name(some_arg, another_example_arg)
/// }
/// ```
///
/// Each platform which your project supports should have their own respective module which uses the impl_interface macro to define their
/// platform-specific implementation of the interface. Using our example interface which we defined in the first section of the documentation
/// we can make an example usage of the impl_interface macro assuming the module this is defined within is named unix.rs (for example) and the super is where
/// the interface.
///
/// ```
/// use super::{Platform, PlatformInterface};
///
/// impl_interface! {
///     fn a_function(example_arg: &str) {
///         println!("I'm running on unix and I want to tell you: {} !", example_arg)
///     }
/// }
/// ```
///
/// ### Three important things to take note of here:
/// - We omit the visibility indicator we defined in the interface; the reason for this is that the visibility indicator in the interface
/// definition only tells the macro something about the re-exported functions. The functions we are implementing here which are present in the trait
/// are unrelated to that visibility indicator.
/// - It looks like we are directly implementing a trait; which we in practice. The macro transparently applies the body of the macro
/// to a trait implementation which is ``impl PlatformInterface for Platform``, this also means that as mentioned earlier; if there is a default
/// implementation of a function you can choose to omit the function in your implementation and let the default implementation stay in place.
/// - We are explicitly importing `Platform` and `PlatformInterface`. The reason for this not being automatic is that there is no good
/// way to infer exactly where the current module is located in the project tree, and if it has access to see the interface types at all.
/// If the current code location is not a submodule of the interface definition or was not given access through the visibility indicator
/// prefix on the struct name, it cannot use the types either. Therefore, the macro expects you to import these types explicitly for it to use so
/// you handle the visibility invariants.
///
/// ---
///
/// How to handle multiple platforms which would traditionally cause trait implementation collisions is documented in the [``define_platforms``] macro
/// which automatically handles the issue of defining platform config gated implementations for you. If you wish to handle it manually that is also a possibility,
/// but it's only recommended for more advanced users.
#[proc_macro]
pub fn define_interface(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as Input);

    define_interface_inner(input).unwrap_or_else(|err| err.to_compile_error().into())
}

fn define_interface_inner(input: Input) -> syn::Result<TokenStream> {
    let platform = &input.platform;
    let interface = &input.interface;
    let impl_macro = &input.impl_macro;
    let methods = input.methods.iter().map(|method| &method.method).collect::<Vec<_>>();

    let methods_sig = methods.iter().map(|method| &method.sig).collect::<Vec<_>>();
    let methods_names = methods.iter().map(|method| &method.sig.ident).collect::<Vec<_>>();
    let methods_args = methods.iter().map(|method|
        method.sig.inputs.iter().map(|input| {
            match input {
                FnArg::Typed(pat) => Ok(&pat.pat),
                FnArg::Receiver(rec) => Err(syn::Error::new_spanned(rec, "interface functions taking self is not supported. Read the define_interface macro docs for more details")),
            }
        }).collect::<syn::Result<Vec<_>>>()
    ).collect::<syn::Result<Vec<_>>>()?;
    let static_visibility = input.methods.iter().map(|method| &method.vis).collect::<Vec<_>>();

    let platform_vis = &input.platform_vis;

    let macro_export = match platform_vis {
        Visibility::Public(_) => Some(quote!(#[macro_export])),
        _ => None
    };

    Ok(quote! {
        #platform_vis struct #platform;

        #platform_vis trait #interface {
            #(#methods)*
        }

        #macro_export
        macro_rules! #impl_macro {
            ($($body:tt)*) => {
                impl #interface for #platform {
                    $($body)*
                }
            };
        }

        #[allow(unused_imports)]
        #platform_vis use #impl_macro;

        #(#static_visibility #methods_sig {
            <#platform as #interface>::#methods_names(#(#methods_args),*)
        })*
    }.into())
}

enum Platform {
    Simple(Ident),
    Complex {
        file: LitStr,
        cfg: Meta
    }
}

impl Parse for Platform {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if input.peek(token::Brace) {
            let content;
            braced!(content in input);

            let mut file = None;
            let mut cfg = None;

            while !content.is_empty() {
                let key = content.parse::<Ident>()?;
                content.parse::<Token![:]>()?;

                let key_str = key.to_string();

                match key_str.as_str() {
                    "file" => {
                        if file.is_some() {
                            return Err(input.error("multiple file fields specified"))
                        }

                        file = Some(content.parse::<LitStr>()?);
                    },
                    "cfg" => {
                        if cfg.is_some() {
                            return Err(input.error("multiple cfg fields specified"))
                        }

                        cfg = Some(content.parse::<Meta>()?);
                    },
                    _ => return Err(input.error(format!("expected `file` or `cfg` as fields, unexpected field: {}", key_str)))
                }

                if content.is_empty() {
                    break;
                }

                content.parse::<token::Comma>()?;
            }

            Ok(Self::Complex {
                file: file.ok_or_else(|| content.error("complex platform definition missing field `file`"))?,
                cfg: cfg.ok_or_else(|| content.error("complex platform definition missing field `cfg`"))?
            })
        } else if input.peek(Ident) {
            Ok(Platform::Simple(input.parse()?))
        } else {
            Err(input.error("Expected either a simple platform ident config or complex braced platform config."))
        }
    }
}

/// ## The `define_platforms` macro
///
/// This macro depends on and complements the [`define_interface`] macro. Please read the documentation of the macro, and make sure to add it to your project before adding this one if you have not already.
///
/// This macro generates module path definitions gated by `cfg` gates for the platform they each represent,
/// where each module implements the interface trait for the struct, which is detailed in the documentation for [`define_interface`]
/// via the implementation macro which makes the whole process a little simpler.
///
/// This macro takes multiple elements representing module definitions, coming in two different formats.
/// One of these is a simpler definition which is just a single identifier for what platform you are on.
/// The only requirement for that it can be used as an identifier here is that it has to represent the
/// platform its on (in simpler words, it has to work being put inside a `#[cfg(...)]` clause).
///
/// An example of this simple usage would be:
/// ```
/// define_platforms![
///     unix,
///     windows
/// ];
/// ```
///
/// This then expands to:
/// ```
/// #[cfg(unix)]
/// #[path = "unix.rs"]
/// mod unix;
///
/// #[cfg(windows)]
/// #[path = "windows.rs"]
/// mod windows;
///
/// #[cfg(all(not(unix), not(windows)))]
/// compile_error!("This crate does not support the current target OS/environment as a compilation target.");
/// ```
///
/// For most users, this is enough. If you need more complex or involved ways to define what paths are currently enabled and implements the trait then you may also do so manually but be aware it's much easier to stumble into issues.
#[proc_macro]
pub fn define_platforms(item: TokenStream) -> TokenStream {
    let input: Vec<_> = parse_macro_input!(item with Punctuated<Platform, Token![,]>::parse_terminated ).into_iter().collect();
    let mut platform_code = vec![];
    let mut opposed_cfg = vec![];

    for platform in &input {
        match platform {
            Platform::Simple(platform) => {
                let lit = LitStr::new(&format!("{}.rs", platform), platform.span());

                platform_code.push(quote! {
                    #[cfg(#platform)]
                    #[path = #lit]
                    mod #platform;
                });

                opposed_cfg.push(quote! {
                    not(#platform)
                });
            },
            Platform::Complex { file, cfg} => {
                let lit = LitStr::new(&format!("{}.rs", file.value()), file.span());
                let file_ident = Ident::new(&file.value(), file.span());

                platform_code.push(quote! {
                    #[cfg(#cfg)]
                    #[path = #lit]
                    mod #file_ident;
                });

                opposed_cfg.push(quote! {
                    not(#cfg)
                });
            }
        }
    }

    quote! {
        #(#platform_code)*
        #[cfg(all(#(#opposed_cfg),*))]
        compile_error!("This crate does not support the current target OS/environment as a compilation target.");
    }.into()
}