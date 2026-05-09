use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{
    Expr, FnArg, Ident, ItemFn, Lit, Meta, Pat, PatIdent, ReturnType, Type, parse_macro_input,
    spanned::Spanned,
};

#[proc_macro_attribute]
pub fn rule(args: TokenStream, input: TokenStream) -> TokenStream {
    let args = parse_macro_input!(args with syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated);
    let input = parse_macro_input!(input as ItemFn);

    match expand_rule(args.into_iter().collect(), input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

struct RuleArgs {
    id: String,
    description: String,
    severity: Ident,
}

struct ViewParam {
    ident: Ident,
    ty: Type,
    capability_method: Ident,
}

fn expand_rule(args: Vec<Meta>, input: ItemFn) -> syn::Result<proc_macro2::TokenStream> {
    let rule_args = parse_rule_args(args)?;
    let vis = input.vis.clone();
    let fn_name = input.sig.ident.clone();
    let run_name = format_ident!("__polint_run_{fn_name}");
    let struct_name = format_ident!("__PolintRule_{fn_name}");
    let block = input.block;
    let attrs = input.attrs;
    let output = input.sig.output.clone();
    validate_return_type(&output)?;

    let mut inputs = input.sig.inputs.iter();
    let Some(first) = inputs.next() else {
        return Err(syn::Error::new(
            input.sig.span(),
            "polint rules must take `ctx: &mut RuleCtx<'_>` as the first parameter",
        ));
    };
    let ctx_ident = parse_ctx_param(first)?;
    let mut view_params = Vec::new();
    for input in inputs {
        view_params.push(parse_view_param(input)?);
    }
    let capability_methods = view_params
        .iter()
        .map(|param| &param.capability_method)
        .collect::<Vec<_>>();
    let view_bindings = view_params.iter().map(|param| {
        let ident = &param.ident;
        let ty = &param.ty;
        quote! {
            let #ident: #ty = <#ty as ::polint::sdk::__private::FactView<'_>>::build(db);
        }
    });
    let view_idents = view_params
        .iter()
        .map(|param| &param.ident)
        .collect::<Vec<_>>();
    let run_inputs = std::iter::once(first.clone())
        .chain(input.sig.inputs.iter().skip(1).cloned())
        .collect::<syn::punctuated::Punctuated<FnArg, syn::Token![,]>>();
    let id = rule_args.id;
    let description = rule_args.description;
    let severity = rule_args.severity;

    Ok(quote! {
        #[allow(non_camel_case_types)]
        struct #struct_name;

        #vis fn #fn_name() -> ::std::sync::Arc<dyn ::polint::sdk::prelude::Rule> {
            ::std::sync::Arc::new(#struct_name)
        }

        impl ::polint::sdk::prelude::Rule for #struct_name {
            fn meta(&self) -> ::polint::sdk::prelude::RuleMeta {
                ::polint::sdk::prelude::RuleMeta {
                    id: #id.to_string(),
                    description: #description.to_string(),
                    severity: ::polint::sdk::prelude::Severity::#severity,
                }
            }

            fn capabilities(&self) -> ::polint::sdk::prelude::Capabilities {
                ::polint::sdk::prelude::Capabilities::new()#(.#capability_methods())*
            }

            fn run(
                &self,
                db: &::polint::sdk::__private::AnalysisDb,
                ctx: &mut ::polint::sdk::prelude::RuleCtx<'_>,
            ) -> ::polint::sdk::prelude::RuleResult {
                #(#view_bindings)*
                #run_name(#ctx_ident, #(#view_idents),*)
            }
        }

        #(#attrs)*
        fn #run_name(#run_inputs) #output #block
    })
}

fn parse_rule_args(args: Vec<Meta>) -> syn::Result<RuleArgs> {
    let mut id = None;
    let mut description = None;
    let mut severity = None;
    for meta in args {
        let Meta::NameValue(name_value) = meta else {
            return Err(syn::Error::new(
                meta.span(),
                "expected name-value rule attribute arguments",
            ));
        };
        let Some(name) = name_value.path.get_ident().map(ToString::to_string) else {
            return Err(syn::Error::new(
                name_value.path.span(),
                "expected simple argument name",
            ));
        };
        let value = string_lit(&name_value.value)?;
        match name.as_str() {
            "id" => id = Some(value),
            "description" => description = Some(value),
            "severity" => severity = Some(parse_severity(&value, name_value.value.span())?),
            _ => {
                return Err(syn::Error::new(
                    name_value.path.span(),
                    "unknown polint rule attribute argument",
                ));
            }
        }
    }
    Ok(RuleArgs {
        id: id.ok_or_else(|| syn::Error::new(proc_macro2::Span::call_site(), "missing `id`"))?,
        description: description.ok_or_else(|| {
            syn::Error::new(proc_macro2::Span::call_site(), "missing `description`")
        })?,
        severity: severity
            .ok_or_else(|| syn::Error::new(proc_macro2::Span::call_site(), "missing `severity`"))?,
    })
}

fn string_lit(expr: &Expr) -> syn::Result<String> {
    let Expr::Lit(expr_lit) = expr else {
        return Err(syn::Error::new(expr.span(), "expected string literal"));
    };
    let Lit::Str(lit) = &expr_lit.lit else {
        return Err(syn::Error::new(expr.span(), "expected string literal"));
    };
    Ok(lit.value())
}

fn parse_severity(value: &str, span: proc_macro2::Span) -> syn::Result<Ident> {
    match value {
        "error" => Ok(format_ident!("Error")),
        "warn" | "warning" => Ok(format_ident!("Warn")),
        "info" => Ok(format_ident!("Info")),
        _ => Err(syn::Error::new(
            span,
            "severity must be one of `error`, `warn`, or `info`",
        )),
    }
}

fn validate_return_type(output: &ReturnType) -> syn::Result<()> {
    if matches!(output, ReturnType::Default) {
        return Err(syn::Error::new(
            output.span(),
            "polint rule functions must return RuleResult",
        ));
    }
    Ok(())
}

fn parse_ctx_param(arg: &FnArg) -> syn::Result<Ident> {
    let FnArg::Typed(pat_type) = arg else {
        return Err(syn::Error::new(arg.span(), "methods are not supported"));
    };
    let Pat::Ident(PatIdent { ident, .. }) = pat_type.pat.as_ref() else {
        return Err(syn::Error::new(
            pat_type.pat.span(),
            "the RuleCtx parameter must be a simple identifier",
        ));
    };
    if ident != "ctx" {
        return Err(syn::Error::new(
            ident.span(),
            "the RuleCtx parameter must be named `ctx`",
        ));
    }
    Ok(ident.clone())
}

fn parse_view_param(arg: &FnArg) -> syn::Result<ViewParam> {
    let FnArg::Typed(pat_type) = arg else {
        return Err(syn::Error::new(arg.span(), "methods are not supported"));
    };
    let Pat::Ident(PatIdent {
        ident, mutability, ..
    }) = pat_type.pat.as_ref()
    else {
        return Err(syn::Error::new(
            pat_type.pat.span(),
            "fact-view parameters must use simple identifiers",
        ));
    };
    if mutability.is_some() {
        return Err(syn::Error::new(
            pat_type.pat.span(),
            "fact-view parameters cannot be `mut`",
        ));
    }
    let capability_method = capability_for_type(pat_type.ty.as_ref())?;
    Ok(ViewParam {
        ident: ident.clone(),
        ty: (*pat_type.ty).clone(),
        capability_method,
    })
}

fn capability_for_type(ty: &Type) -> syn::Result<Ident> {
    let Type::Path(path) = ty else {
        return Err(syn::Error::new(
            ty.span(),
            "unsupported polint fact view parameter",
        ));
    };
    let Some(segment) = path.path.segments.last() else {
        return Err(syn::Error::new(
            ty.span(),
            "unsupported polint fact view parameter",
        ));
    };
    let method = match segment.ident.to_string().as_str() {
        "SourceFiles" | "Packages" | "Functions" => "syntax",
        "Imports" => "imports",
        "Cfg" => "cfg",
        "CallGraph" => "call_graph",
        "GoTests" => "go_tests",
        "BranchObligations" => "branch_obligations",
        "CoverageFacts" => "coverage_facts",
        "TestSuiteMetrics" => "test_suite_metrics",
        "TsComponents" => "ts_components",
        "TsClasses" => "ts_classes",
        "StringLiterals" => "string_literals",
        "JsxAttributes" => "jsx_attributes",
        _ => {
            return Err(syn::Error::new(
                segment.ident.span(),
                "unsupported polint fact view parameter",
            ));
        }
    };
    Ok(format_ident!("{method}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capability(type_source: &str) -> String {
        let ty = syn::parse_str::<Type>(type_source).unwrap();
        capability_for_type(&ty).unwrap().to_string()
    }

    #[test]
    fn capability_for_type_maps_supported_fact_views() {
        assert_eq!(capability("SourceFiles<'_>"), "syntax");
        assert_eq!(capability("GoTests<'_>"), "go_tests");
        assert_eq!(capability("BranchObligations<'_>"), "branch_obligations");
        assert_eq!(capability("StringLiterals<'_>"), "string_literals");
        assert_eq!(
            capability("polint::sdk::facts::JsxAttributes<'_>"),
            "jsx_attributes"
        );
    }

    #[test]
    fn capability_for_type_rejects_unknown_fact_views() {
        let ty = syn::parse_str::<Type>("UserDefinedFacts<'_>").unwrap();
        let error = capability_for_type(&ty).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("unsupported polint fact view parameter")
        );
    }
}
