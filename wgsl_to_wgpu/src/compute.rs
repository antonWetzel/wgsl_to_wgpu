use proc_macro2::{Literal, Span, TokenStream};
use quote::quote;
use syn::Ident;

use crate::TypePath;

pub fn compute_module<F>(module: &naga::Module, demangle: F) -> TokenStream
where
    F: Fn(&str) -> TypePath + Clone,
{
    let entry_points: Vec<_> = module
        .entry_points
        .iter()
        .filter_map(|e| {
            if e.stage == naga::ShaderStage::Compute {
                let workgroup_size_constant = workgroup_size(e, demangle.clone());
                let create_pipeline = create_compute_pipeline(module, e, demangle.clone());

                Some(quote! {
                    #workgroup_size_constant
                    #create_pipeline
                })
            } else {
                None
            }
        })
        .collect();

    if entry_points.is_empty() {
        // Don't include empty modules.
        quote!()
    } else {
        quote! {
            pub mod compute {
                pub fn empty_descriptor<'a>(module: &'a wgpu::ShaderModule) -> wgpu::ComputePipelineDescriptor<'a> {
                    wgpu::ComputePipelineDescriptor {
                        label: None,
                        layout: None,
                        module,
                        entry_point: None,
                        compilation_options: Default::default(),
                        cache: None,
                    }
                }

                #(#entry_points)*
            }
        }
    }
}

fn create_compute_pipeline<F>(
    module: &naga::Module,
    e: &naga::EntryPoint,
    demangle: F,
) -> TokenStream
where
    F: Fn(&str) -> TypePath,
{
    // Ignore the path, because the compute pipeline requires
    // the bind groups and layouts from the shader module.
    let name = &demangle(&e.name).name;

    // Compute pipeline creation has few parameters and can be generated.
    let function_name = Ident::new(&format!("create_{name}_pipeline"), Span::call_site());

    // TODO: Include a user supplied module name in the label?
    let label = format!("Compute Pipeline {name}");

    // The entry name string itself should remain mangled to match the WGSL code.
    let entry_point = &e.name;

    let (compilations_options, compilations_options_arg) = if !module.overrides.is_empty() {
        let options = quote! {
            wgpu::PipelineCompilationOptions {
                constants: &overrides.constants(),
                ..Default::default()
            }
        };
        (options, quote! { overrides: &super::OverrideConstants, })
    } else {
        (quote! { Default::default() }, quote! {})
    };

    quote! {
        pub fn #function_name(device: &wgpu::Device, desc: &wgpu::ComputePipelineDescriptor, #compilations_options_arg) -> wgpu::ComputePipeline {
            let layout = desc.layout.is_none().then(|| super::create_pipeline_layout(device));

            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: desc.label.or(Some(#label)),
                layout: desc.layout.or(layout.as_ref()),
                module: desc.module,
                entry_point: Some(#entry_point),
                compilation_options: #compilations_options,
                cache: desc.cache,
            })
        }
    }
}

fn workgroup_size<F>(e: &naga::EntryPoint, demangle: F) -> TokenStream
where
    F: Fn(&str) -> TypePath + Clone,
{
    // Ignore the path, see above for reason.
    let name = &demangle(&e.name).name;

    let name = Ident::new(
        &format!("{}_WORKGROUP_SIZE", name.to_uppercase()),
        Span::call_site(),
    );
    let [x, y, z] = e
        .workgroup_size
        .map(|s| Literal::usize_unsuffixed(s as usize));
    quote!(pub const #name: [u32; 3] = [#x, #y, #z];)
}
