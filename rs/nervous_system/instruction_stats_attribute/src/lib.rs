use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{parse_macro_input, ItemFn, Stmt};

/// This does almost the same thing as ic_cdk::update. There is just one
/// difference: This adds a statement to the beginning of the function. It looks
/// something like this:
///
/// let _on_drop = foo(#function_name);
///
/// For this to work, you will need to depend on
/// ic-nervous-system-instruction-stats, because foo is defined there.
///
/// More precisely, foo tracks instructions used by the call context. To expose
/// this data, ic_nervous_system_instruction_stats::encode_instruction_metrics
/// needs to be called.
#[proc_macro_attribute]
pub fn update(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut item_fn = parse_macro_input!(item as ItemFn);

    let function_name = item_fn.sig.ident.to_string();

    // Create statement that we'll insert into the function.
    let new_stmt = quote! {
        let _on_drop = ic_nervous_system_instruction_stats::UpdateInstructionStatsOnDrop::new(
            &ic_nervous_system_instruction_stats::BasicRequest::new(#function_name)
        );
    };
    let new_stmt = TokenStream::from(new_stmt);
    let new_stmt = parse_macro_input!(new_stmt as Stmt);

    item_fn.block.stmts.insert(0, new_stmt);

    let updated_item_fn = quote! {
        #[ic_cdk::update]
        #item_fn
    };

    TokenStream::from(updated_item_fn.into_token_stream())
}
