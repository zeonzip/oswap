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

#[proc_macro]
pub fn define_interface(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as Input);

    let platform = &input.platform;
    let interface = &input.interface;
    let impl_macro = &input.impl_macro;
    let methods = input.methods.iter().map(|method| &method.method).collect::<Vec<_>>();

    let methods_sig = methods.iter().map(|method| &method.sig).collect::<Vec<_>>();
    let methods_names = methods.iter().map(|method| &method.sig.ident).collect::<Vec<_>>();
    let methods_args = methods.iter().map(|method|
        method.sig.inputs.iter().map(|input| {
            match input {
                FnArg::Typed(pat) => Some(&pat.pat),
                FnArg::Receiver(_) => None
            }
        }).collect::<Vec<_>>()
    ).collect::<Vec<_>>();
    let static_visibility = input.methods.iter().map(|method| &method.vis).collect::<Vec<_>>();

    let platform_vis = &input.platform_vis;

    quote! {
        #platform_vis struct #platform;

        #platform_vis trait #interface {
            #(#methods)*
        }

        macro_rules! #impl_macro {
            ($($body:tt)*) => {
                impl #interface for #platform {
                    $($body)*
                }
            };
        }

        #(#static_visibility #methods_sig {
            <#platform as #interface>::#methods_names(#(#methods_args)*)
        })*
    }.into()
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
                    mod platform;
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